//! Linear static analysis: assemble, solve every load case, recover forces.

use std::collections::HashMap;

use crate::area;
use crate::axes::{self, Axes};
use crate::combine;
use crate::dof::{DofMap, NDF};
use crate::eigen::{self, SymSparse};
use crate::elements::shell::Shell;
use crate::elements::{self, edge, frame, wall_joint, LocalLoad, Props, Side};
use crate::mass;
use crate::model::{
    coords, length, ComboType, FrameLoad, Id, LoadCase, LoadDir, ModalCase, ModalType, Model,
    PDeltaMethod, Release, ShellType, DOF_NAMES,
};
use crate::results::{
    CaseResults, ElementResults, Equilibrium, Instability, ModalResults, Mode, NodeValues, Results,
    ShellCorner, ShellResults,
};
use crate::rigid::{Bodies, Ties};
use crate::sparse::{Factor, SymMatrix};
use crate::spectrum;
use crate::timing::{self, Clock};
use crate::Error;

/// Number of equally spaced output stations along each element (ends included).
const STATIONS: usize = combine::STATIONS;

/// A free DOF gets no stiffness from any element (e.g. rotations at truss-only nodes) and is restrained
/// automatically when its stiffness diagonal is below this fraction of the largest stiffness a single
/// element gives its joint in the same kind of direction (the three translations, or the three
/// rotations). The measure is the joint's own elements, never the model's largest diagonal: a very
/// stiff member elsewhere must not make a connected joint count as free.
const ZERO_STIFFNESS: f64 = 1e-12;

/// Pivot ratio below which results get an accuracy warning (about 8 digits lost).
const ILL_CONDITIONED: f64 = 1e-8;

/// Load cases solved together in one pass over the factor.
const SOLVE_BATCH: usize = 32;

/// A load case's loads, kept from building the right-hand side to recovering the forces.
struct Loaded {
    id: Id,
    combination: Option<ComboType>,
    /// Joint loads per DOF (node * 6 + d).
    nodal: Vec<f64>,
    elem_loads: Vec<Vec<LocalLoad>>,
    /// Resultant and moment about the origin of the member loads.
    applied_member: [f64; 6],
    /// Fixed-end forces of each member.
    fixed: Vec<[f64; 12]>,
    /// Each area's loads at its joints (6 per joint; empty without loads), for the story cuts.
    shell_loads: Vec<Vec<f64>>,
}

struct Elem {
    nodes: [usize; 2],
    /// The axes the element is analysed in: the frame's local axes, or for a section with a product of
    /// inertia (I23) its principal axes, turned by `turn` (radians) about local 1.
    axes: Axes,
    /// The frame's local axes (member loads in local 2 and 3, results).
    geo: Axes,
    turn: f64,
    props: Props,
    /// Local stiffness without end releases (for recovering the released end displacements).
    k_full: [[f64; 12]; 12],
    /// Released local DOFs (end releases), all false for most frames.
    rel: [bool; 12],
    /// Global stiffness at the joints, with the released DOFs condensed out.
    k_global: [[f64; 12]; 12],
    /// Rigid arms from the joints to the element's ends (an insertion point that changes the stiffness).
    arms: Option<[[f64; 3]; 2]>,
    /// Where the element starts (end i: its joint plus the arm).
    start: [f64; 3],
}

impl Elem {
    /// Sets `k_full` and `k_global` from the element's properties (its P-Delta axial force included).
    fn stiffen(&mut self) {
        self.k_full = elements::local_stiffness(&self.props);
        self.k_global = self.global_stiffness(&self.props);
    }

    /// The stiffness at the joints (global axes, released DOFs condensed) for the properties `props`.
    fn global_stiffness(&self, props: &Props) -> [[f64; 12]; 12] {
        let rel = &self.rel;
        // Releases act at the clear-length ends: inside the member when it has end offsets.
        let k_local = if frame::hinged_inside(props, rel) {
            frame::hinged_stiffness(props, rel)
        } else if rel.contains(&true) {
            frame::condense(&elements::local_stiffness(props), &[0.0; 12], rel).0
        } else {
            elements::local_stiffness(props)
        };
        let mut k_global = axes::mat12_to_global(&self.axes, &k_local);
        if let Some(arms) = &self.arms {
            let flat: Vec<f64> = k_global.iter().flatten().copied().collect();
            let k = crate::arms::stiffness(&flat, arms);
            k_global = std::array::from_fn(|r| std::array::from_fn(|c| k[r * 12 + c]));
        }
        k_global
    }

    /// Local end forces (nodes on the element) and local end displacements (released DOFs filled) from
    /// the joint displacements `u`, the fixed-end forces `fef` and the member loads; and the hinges
    /// inside the member.
    fn end_forces(
        &self,
        u: &[f64],
        fef: &[f64; 12],
        loads: &[LocalLoad],
    ) -> ([f64; 12], [f64; 12], frame::Hinges) {
        let mut ug = [0.0; 12];
        for a in 0..12 {
            ug[a] = u[self.nodes[a / NDF] * NDF + a % NDF];
        }
        if let Some(arms) = &self.arms {
            crate::arms::to_ends(&mut ug, arms);
        }
        let mut ul = axes::vec12_to_local(&self.axes, &ug);
        let mut hinges = frame::Hinges::default();
        let mut end = *fef;
        if frame::hinged_inside(&self.props, &self.rel) {
            (end, hinges) = frame::hinged_end_forces(&self.props, &self.rel, loads, &ul);
        } else {
            if self.rel.contains(&true) {
                if self.props.pd != 0.0 && frame::bending_release(&self.rel) {
                    let plain = frame::local_stiffness(&Props {
                        pd: 0.0,
                        as2: 0.0,
                        as3: 0.0,
                        ..self.props
                    });
                    let mut shape = ul;
                    frame::released_displacements(&plain, &[0.0; 12], &self.rel, &mut shape);
                    hinges.shape_ends = Some(shape);
                }
                frame::released_displacements(&self.k_full, fef, &self.rel, &mut ul);
            }
            for a in 0..12 {
                end[a] += (0..12).map(|b| self.k_full[a][b] * ul[b]).sum::<f64>();
            }
            for (e, r) in end.iter_mut().zip(&self.rel) {
                if *r {
                    *e = 0.0; // zero by construction; drop the round-off
                }
            }
        }
        (end, ul, hinges)
    }
}

/// A shell element (one per area in Phase 3a, 4 or 3 corners) and its global stiffness (6n x 6n,
/// row-major).
struct ShellEl {
    nodes: Vec<usize>,
    shell: Shell,
    /// Global stiffness at the joints (with P-Delta from loads, its geometric stiffness included).
    k_global: Vec<f64>,
    /// Rigid arm from each joint to the element's corner (an insertion point that changes the
    /// stiffness, plus a warped quad's arm to its mean plane), one per node.
    arm: Option<Vec<[f64; 3]>>,
    /// The insertion point's part of the arms (the same at every corner): area loads act at the
    /// mid-plane and turn the joints through it. A warped quad's arms do not move its loads
    /// (docs/SOURCES.md).
    load_arm: Option<[f64; 3]>,
}

impl ShellEl {
    /// The geometric stiffness at the joints (global) of the membrane forces the joint displacements
    /// `u` (all joints) give (see `Shell::geometric`).
    fn geometric(&self, u: &[f64]) -> Vec<f64> {
        let mut ug: Vec<f64> = (0..NDF * self.nodes.len())
            .map(|a| u[self.nodes[a / NDF] * NDF + a % NDF])
            .collect();
        if let Some(e) = &self.arm {
            crate::arms::to_ends(&mut ug, e);
        }
        let kg = self.shell.geometric(&ug);
        match &self.arm {
            Some(e) => crate::arms::stiffness(&kg, e),
            None => kg,
        }
    }
}

/// A stiffness matrix over whole nodes (6 DOFs each), already carried over to the nodes' carriers
/// (rigid-body and diaphragm masters), so it assembles straight into the equations.
struct Block {
    /// Carrier node of each group of 6 rows.
    nodes: Vec<usize>,
    /// Row-major, 6n x 6n.
    k: Vec<f64>,
}

impl Block {
    fn carried(nodes: &[usize], k: Vec<f64>, ties: &Ties) -> Block {
        let (nodes, k) = ties.carry(nodes, k);
        Block { nodes, k }
    }

    fn size(&self) -> usize {
        NDF * self.nodes.len()
    }

    /// DOF index (carrier node * 6 + d) of row or column `a`.
    fn dof(&self, a: usize) -> usize {
        self.nodes[a / NDF] * NDF + a % NDF
    }
}

/// Scale on rigid lengths so that at least a thousandth of a member stays flexible.
fn rigid_cap(off: &[f64; 2], l: f64, rz: [f64; 2]) -> f64 {
    let rigid = rz[0] * off[0] + rz[1] * off[1];
    if rigid > 0.999 * l {
        0.999 * l / rigid
    } else {
        1.0
    }
}

pub fn analyze(model: &Model) -> Result<Results, Error> {
    let mut cases = Vec::new();
    let mut r = analyze_each(model, &mut |c| {
        cases.push(c);
        Ok(())
    })?;
    r.cases = cases;
    Ok(r)
}

/// [`analyze`] handing each case's results to `out` as soon as they are known, in the same order, and
/// returning the rest (`cases` empty). A combination with a maximum and a minimum takes its parts as
/// they are solved (two running results, see [`combine::Order`]), so a model with many load cases
/// and combinations needs the memory of a few results.
///
/// A modal case that fails (no convergence, modes proven missing) does not take the other cases with
/// it: the analysis is made again without it, and without the response spectrum cases and the
/// combinations that need it; the results name it in `failed` and in a message.
pub fn analyze_each(
    model: &Model,
    out: &mut dyn FnMut(CaseResults) -> Result<(), Error>,
) -> Result<Results, Error> {
    let mut modal_failed = None;
    let first = analyze_run(model, out, &mut modal_failed);
    // The modal case comes before any case is handed on, so nothing has reached `out` yet.
    let (Err(error), Some(case)) = (&first, modal_failed) else {
        return first;
    };
    let reason = match error {
        Error::Invalid(msg) => msg.clone(),
        other => other.to_string(),
    };
    let mut rest = model.clone();
    let spectrum: Vec<Id> = rest.spectrum_cases.iter().map(|c| c.id.clone()).collect();
    for id in std::iter::once(case.clone()).chain(spectrum) {
        if !rest.not_run.contains(&id) {
            rest.not_run.push(id);
        }
    }
    // Nothing else to analyse (or the rest fails too): the modal case's own error stands.
    let Ok(mut results) = analyze_run(&rest, out, &mut None) else {
        return first;
    };
    results.notes.insert(
        0,
        format!(
            "Warning: {reason}. It has no results; the other cases were analysed without it (response \
             spectrum cases and the combinations that need it are left out, see below)"
        ),
    );
    results
        .failed
        .push(crate::results::FailedCase { case, reason });
    Ok(results)
}

/// [`analyze_each`] proper. `modal_failed` is set to the modal case's name when the error returned is
/// that case's failure.
fn analyze_run(
    model: &Model,
    out: &mut dyn FnMut(CaseResults) -> Result<(), Error>,
    modal_failed: &mut Option<Id>,
) -> Result<Results, Error> {
    let mut clock = Clock::new();
    model.validate()?;
    let (run, run_note) = model.for_run()?;
    let model = &*run.with_patterns()?;
    // Members follow the end-order rule (end i at the lower Z, then X, then Y), so end i/j results are
    // independent of the drawing direction.
    let (oriented, reversed) = model.oriented();
    let model = &oriented;
    let n_nodes = model.nodes.len();
    let node_index: HashMap<&Id, usize> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (&n.id, i))
        .collect();
    let mut notes: Vec<String> = run_note.into_iter().collect();
    if !reversed.is_empty() {
        let ids: Vec<String> = reversed.iter().map(|id| id.to_string()).collect();
        notes.push(format!(
            "Reversed so that end i is the lower end (then the lower X, then the lower Y): frames {}",
            ids.join(", ")
        ));
    }

    // End length offsets (given or automatic ones); their rigid part acts in bending and shear.
    let offsets = model.end_offsets();
    let mut elems: Vec<Elem> = model
        .frames
        .iter()
        .zip(&offsets)
        .map(|(e, off)| {
            let ni = node_index[&e.nodes[0]];
            let nj = node_index[&e.nodes[1]];
            let (a, b) = (&model.nodes[ni], &model.nodes[nj]);
            let mat = model.material(&e.material).unwrap();
            let sec = model.section(&e.section).unwrap();
            // Property modifiers: the section's times the frame's own.
            let m = model.frame_modifiers(e);
            // Rigid-zone factor of each end in each bending plane: a release makes the whole offset at
            // its end rigid in its own plane (V2 or M3: the 1-2 plane; V3 or M2: the 1-3 plane), P and T
            // change nothing (the hinge sits at the clear-length end, the offset behind it does
            // not bend; B48 D, B54, B55).
            let zone = |plane: [Release; 2]| {
                [&e.releases.i, &e.releases.j].map(|r| {
                    if r.iter().any(|x| plane.contains(x)) {
                        1.0
                    } else {
                        e.rigid_zone
                    }
                })
            };
            let (z12, z13) = (
                zone([Release::V2, Release::M3]),
                zone([Release::V3, Release::M2]),
            );
            // An insertion point that changes the stiffness: the element runs between the section's
            // centroids, tied to the joints by rigid arms.
            let arms = model.frame_arms(e, coords(a), coords(b));
            let ends = match arms {
                Some([ai, aj]) => [axes::add(coords(a), ai), axes::add(coords(b), aj)],
                None => [coords(a), coords(b)],
            };
            let d = axes::sub(ends[1], ends[0]);
            let l = axes::dot(d, d).sqrt();
            let bend = Principal::of(
                sec.I33 * m.I33,
                sec.I22 * m.I22,
                sec.I23 * (m.I22 * m.I33).sqrt(),
                sec.As2 * m.As2,
                sec.As3 * m.As3,
            );
            let props = Props {
                kind: e.kind,
                e: mat.e,
                g: mat.shear_modulus(),
                a: sec.A * m.A,
                i22: bend.i22,
                i33: bend.i33,
                j: sec.J * m.J,
                as2: bend.as2,
                as3: bend.as3,
                l,
                // A piece the mesh step cut wholly inside a rigid zone keeps a sliver of flexible
                // length, so its stiffness stays finite.
                ri: z12[0] * off[0] * rigid_cap(off, l, z12),
                rj: z12[1] * off[1] * rigid_cap(off, l, z12),
                ri3: z13[0] * off[0] * rigid_cap(off, l, z13),
                rj3: z13[1] * off[1] * rigid_cap(off, l, z13),
                oi: off[0],
                oj: off[1],
                pd: 0.0,
            };
            let geo = axes::local_axes(ends[0], ends[1], e.angle);
            let mut el = Elem {
                nodes: [ni, nj],
                axes: turned(&geo, bend.turn),
                geo,
                turn: bend.turn,
                props,
                k_full: [[0.0; 12]; 12],
                rel: e.releases.mask(),
                k_global: [[0.0; 12]; 12],
                arms,
                start: ends[0],
            };
            el.stiffen();
            el
        })
        .collect();

    // Areas: one shell element each (Phase 3a has no hidden mesh).
    let mut shells: Vec<ShellEl> = model
        .areas
        .iter()
        .map(|a| {
            let nodes: Vec<usize> = a.nodes.iter().map(|id| node_index[id]).collect();
            let pts: Vec<[f64; 3]> = nodes.iter().map(|&n| coords(&model.nodes[n])).collect();
            let sec = model.area_section(&a.section).expect("validated");
            let (mat_id, t) = sec.analysed();
            let mat = model.material(mat_id).expect("validated");
            let ax = axes::area_axes(&pts, a.angle);
            // An insertion point that changes the stiffness: the element lies on the shell's mid-plane,
            // tied to the joints by rigid arms along local 3. A warped quad: the flat element on its
            // mean plane, tied to the joints by rigid arms along the normal.
            let ins = model.area_arm(a, ax[2]);
            let warp = crate::area::warp_arms(&pts, &ax);
            let arm: Option<Vec<[f64; 3]>> = match (ins, warp) {
                (None, None) => None,
                (i, w) => Some(
                    (0..pts.len())
                        .map(|k| {
                            let wk = w.as_ref().map_or([0.0; 3], |w| w[k]);
                            axes::add(wk, i.unwrap_or([0.0; 3]))
                        })
                        .collect(),
                ),
            };
            let at: Vec<[f64; 3]> = pts
                .iter()
                .enumerate()
                .map(|(k, p)| arm.as_ref().map_or(*p, |e| axes::add(*p, e[k])))
                .collect();
            let shell = Shell::with_modifiers(
                sec.kind,
                &at,
                ax,
                mat.e,
                mat.poisson(),
                t,
                model.area_modifiers(a).stiffness(),
            );
            let k_global = match &arm {
                Some(e) => crate::arms::stiffness(&shell.k_global(), e),
                None => shell.k_global(),
            };
            ShellEl {
                nodes,
                k_global,
                shell,
                arm,
                load_arm: ins,
            }
        })
        .collect();
    let warnings: Vec<String> = model
        .areas
        .iter()
        .filter_map(|a| {
            let pts: Vec<[f64; 3]> = a
                .nodes
                .iter()
                .map(|id| coords(&model.nodes[node_index[id]]))
                .collect();
            area::shape_warning(&pts).map(|w| format!("{} ({w})", a.id))
        })
        .collect();
    if !warnings.is_empty() {
        notes.push(format!(
            "Badly shaped areas, results less accurate: {}",
            warnings.join(", ")
        ));
    }

    // Edge constraints: joints tied to an area's edge by hidden beams (see `elements::edge`).
    let areas_by_id: HashMap<&Id, &crate::model::Area> =
        model.areas.iter().rev().map(|a| (&a.id, a)).collect();
    let edges: Vec<([usize; 3], Vec<f64>)> = model
        .edge_constraints
        .iter()
        .map(|ec| {
            let a = areas_by_id[&ec.area];
            let pts: Vec<[f64; 3]> = a
                .nodes
                .iter()
                .map(|id| coords(&model.nodes[node_index[id]]))
                .collect();
            let ids = [
                node_index[&ec.edge[0]],
                node_index[&ec.edge[1]],
                node_index[&ec.node],
            ];
            let [pa, pb, pm] = ids.map(|n| coords(&model.nodes[n]));
            let sec = model.area_section(&a.section).expect("validated");
            let (mat_id, t) = sec.analysed();
            let mat = model.material(mat_id).expect("validated");
            let normal = axes::area_axes(&pts, 0.0)[2];
            let w = edge::width(&pts, pa, pb);
            (
                ids,
                edge::stiffness(
                    pa,
                    pb,
                    pm,
                    normal,
                    mat.e,
                    t,
                    w,
                    sec.kind != ShellType::Membrane,
                ),
            )
        })
        .collect();
    if !edges.is_empty() {
        notes.push(format!(
            "Edge constraints tie {} joints to area edges",
            edges.len()
        ));
    }

    // Frame-to-wall joints: a rotational spring at every wall-quad corner a frame meets (see
    // `elements::wall_joint`). Edge constraints act as hidden frames, so their joints count too.
    let mut framed = vec![false; n_nodes];
    for el in &elems {
        framed[el.nodes[0]] = true;
        framed[el.nodes[1]] = true;
    }
    for (ids, _) in &edges {
        for &n in ids {
            framed[n] = true;
        }
    }
    let mut wall_springs: Vec<([usize; 2], Vec<f64>)> = Vec::new();
    // The area of each spring (its forces count with the area's in pier and spandrel forces).
    let mut spring_area: Vec<usize> = Vec::new();
    for (ai, (sh, a)) in shells.iter().zip(&model.areas).enumerate() {
        if sh.nodes.len() != 4 || !sh.nodes.iter().any(|&n| framed[n]) {
            continue;
        }
        let p: [[f64; 3]; 4] = std::array::from_fn(|k| coords(&model.nodes[sh.nodes[k]]));
        let normal = axes::area_axes(&p, 0.0)[2];
        let slope = normal[2].abs().clamp(0.0, 1.0).acos().to_degrees();
        // Walls only (the floor/wall angle rule, the model's limit: 20 degrees by default).
        if slope < model.wall_slope() - 1e-9 {
            continue;
        }
        let sec = model.area_section(&a.section).expect("validated");
        let (mat_id, t) = sec.analysed();
        let mat = model.material(mat_id).expect("validated");
        let flags = std::array::from_fn(|k| framed[sh.nodes[k]]);
        // The spring follows the wall's membrane modifiers: the smallest of f11, f22, f12
        // (measured, checks/wall_joint_probe.py group M).
        let fm = model.area_modifiers(a);
        let e = mat.e * fm.f11.min(fm.f22).min(fm.f12);
        for (j, k, kk) in wall_joint::springs(&p, e, t, flags) {
            wall_springs.push(([sh.nodes[j], sh.nodes[k]], kk));
            spring_area.push(ai);
        }
    }
    if !wall_springs.is_empty() {
        notes.push(format!(
            "Frame-to-wall joints: {} rotational springs",
            wall_springs.len()
        ));
    }

    // Rigid links: slave joints follow their body's master (see `rigid`). Rigid diaphragms: per story,
    // joints follow a master in UX, UY and RZ (see `diaphragm`); a plane grid has none of those DOFs.
    let bodies = Bodies::new(model)?;
    let mut levels = crate::diaphragm::levels(model)?;
    if !levels.is_empty() {
        if model.dimension == crate::model::Dimension::Grid {
            notes.push("Diaphragms do nothing in a plane grid (UX, UY and RZ are held)".into());
            levels.clear();
        } else {
            notes.push(format!(
                "Rigid diaphragms: {} constraints ({} joints)",
                levels.len(),
                levels.iter().map(|l| l.joints.len()).sum::<usize>()
            ));
        }
    }
    let ties = Ties::new(n_nodes, &bodies, &levels, |n| coords(&model.nodes[n]));

    // Restraints: supports, plane-frame DOFs, then DOFs nothing stiffens. Slaves have no equations.
    let mut restrained = vec![false; n_nodes * NDF];
    let mut user_restrained = vec![false; n_nodes * NDF];
    let mut supported = vec![false; n_nodes];
    for s in &model.supports {
        let n = node_index[&s.node];
        supported[n] = true;
        for (d, r) in s.restraints()?.iter().enumerate() {
            if *r {
                restrained[n * NDF + d] = true;
                user_restrained[n * NDF + d] = true;
            }
        }
    }
    // A plane frame (2D) or plane grid holds the DOFs it leaves out at every node; tied DOFs follow
    // their master's zero motion there.
    let inactive = model.dimension.inactive();
    for n in 0..n_nodes {
        for &d in inactive {
            restrained[n * NDF + d] |= ties.own[n * NDF + d];
        }
    }
    let mut spring = vec![[0.0f64; NDF]; n_nodes];
    for s in &model.springs {
        spring[node_index[&s.node]] = s.as_array();
    }
    let mut idle = Vec::new();
    for s in &model.springs {
        let n = node_index[&s.node];
        for d in 0..NDF {
            let held = user_restrained[n * NDF + d] || inactive.contains(&d);
            if spring[n][d] > 0.0 && held {
                idle.push(format!("{}:{}", s.node, DOF_NAMES[d]));
            }
        }
    }
    if !idle.is_empty() {
        notes.push(format!(
            "Springs on restrained DOFs have no effect: {}",
            idle.join(", ")
        ));
    }

    // Line and area springs, lumped at the joints of each element in its local axes: a frame end gets the
    // stiffness times half the element's length, a shell corner times its share of the element (the
    // integral of its shape function, as a uniform area load is shared). Translations, and R1 about a
    // frame's axis; no coupling between joints.
    let mut spread = vec![[[0.0f64; NDF]; NDF]; n_nodes];
    let mut add_spread = |n: usize, k: f64, e: [f64; 3], rot: bool| {
        let o = if rot { 3 } else { 0 };
        for r in 0..3 {
            for c in 0..3 {
                spread[n][o + r][o + c] += k * e[r] * e[c];
            }
        }
    };
    for (f, el) in model.frames.iter().zip(&elems) {
        let Some(p) = f
            .line_spring
            .as_ref()
            .and_then(|id| model.line_spring_props.iter().find(|x| &x.id == id))
        else {
            continue;
        };
        let (a, b) = (
            coords(&model.nodes[el.nodes[0]]),
            coords(&model.nodes[el.nodes[1]]),
        );
        let d = axes::sub(b, a);
        let half = 0.5 * axes::dot(d, d).sqrt();
        let ax = axes::local_axes(a, b, f.angle);
        for &n in &el.nodes {
            for (k, e) in [p.u1, p.u2, p.u3].into_iter().zip(ax) {
                add_spread(n, k * half, e, false);
            }
            add_spread(n, p.r1 * half, ax[0], true);
        }
    }
    for (a, sh) in model.areas.iter().zip(&shells) {
        let Some(p) = a
            .area_spring
            .as_ref()
            .and_then(|id| model.area_spring_props.iter().find(|x| &x.id == id))
        else {
            continue;
        };
        let shares = sh.shell.area_load([0.0, 0.0, 1.0]);
        for (&n, w) in sh.nodes.iter().zip(&shares) {
            for (k, e) in [p.u1, p.u2, p.u3].into_iter().zip(sh.shell.axes) {
                add_spread(n, k * w[2], e, false);
            }
        }
    }
    let has_spread: Vec<bool> = spread
        .iter()
        .map(|m| m.iter().flatten().any(|v| *v != 0.0))
        .collect();

    // Stiffness blocks on carrier (master) DOFs: members, then springs.
    let mut blocks: Vec<Block> = elems
        .iter()
        .map(|el| {
            let flat: Vec<f64> = el.k_global.iter().flatten().copied().collect();
            Block::carried(&el.nodes, flat, &ties)
        })
        .collect();
    for sh in &shells {
        blocks.push(Block::carried(&sh.nodes, sh.k_global.clone(), &ties));
    }
    for (ids, kc) in &edges {
        blocks.push(Block::carried(ids, kc.clone(), &ties));
    }
    for (ids, kc) in &wall_springs {
        blocks.push(Block::carried(ids, kc.clone(), &ties));
    }
    for n in (0..n_nodes).filter(|&n| spring[n].iter().any(|k| *k > 0.0) || has_spread[n]) {
        let mut flat = vec![0.0; NDF * NDF];
        for r in 0..NDF {
            for c in 0..NDF {
                flat[r * NDF + c] = spread[n][r][c];
            }
            flat[r * NDF + r] += spring[n][r];
        }
        blocks.push(Block::carried(&[n], flat, &ties));
    }
    clock.lap("analysis: element stiffness, ties");

    // Each DOF's stiffness diagonal, and the largest part of it that one element gives (several rows of
    // an element can share a carrier DOF, so an element's part is summed first).
    let mut diag = vec![0.0f64; n_nodes * NDF];
    let mut largest_part = vec![0.0f64; n_nodes * NDF];
    let mut part: Vec<(usize, f64)> = Vec::new();
    for blk in &blocks {
        let size = blk.size();
        part.clear();
        for a in 0..size {
            let dof = blk.dof(a);
            let sum: f64 = (0..size)
                .filter(|&b| blk.dof(b) == dof)
                .map(|b| blk.k[a * size + b])
                .sum();
            match part.iter_mut().find(|(d, _)| *d == dof) {
                Some((_, v)) => *v += sum,
                None => part.push((dof, sum)),
            }
        }
        for &(dof, v) in &part {
            diag[dof] += v;
            largest_part[dof] = largest_part[dof].max(v.abs());
        }
    }
    // What a DOF's diagonal is judged against: the largest part among its joint's three translations,
    // or three rotations.
    let own_scale = |k: usize| -> f64 {
        let first = k - k % 3;
        largest_part[first..first + 3]
            .iter()
            .cloned()
            .fold(0.0, f64::max)
    };
    timing::note(|| {
        let max_diag = diag.iter().cloned().fold(0.0, f64::max);
        let Some(at) = diag.iter().position(|v| *v == max_diag) else {
            return "no stiffness diagonals".into();
        };
        format!(
            "largest stiffness diagonal {:.6e} at {}:{}; free DOFs: below {:.0e} of the joint's own",
            max_diag,
            model.nodes[at / NDF].id,
            DOF_NAMES[at % NDF],
            ZERO_STIFFNESS
        )
    });
    let mut auto_restrained = vec![false; n_nodes * NDF];
    let mut auto_list = Vec::new();
    for k in 0..n_nodes * NDF {
        if !restrained[k] && ties.own[k] && diag[k] <= ZERO_STIFFNESS * own_scale(k) {
            restrained[k] = true;
            auto_restrained[k] = true;
            auto_list.push(format!(
                "{}:{}",
                model.nodes[k / NDF].id,
                DOF_NAMES[k % NDF]
            ));
        }
    }
    if !auto_list.is_empty() {
        notes.push(format!(
            "Restrained automatically (no element gives these DOFs stiffness): {}",
            auto_list.join(", ")
        ));
    }
    let mut followers = Followers::new(&shells, &auto_restrained);
    clock.lap("analysis: restraints");

    let pd_method = model
        .p_delta
        .as_ref()
        .map_or(PDeltaMethod::None, |p| p.method);
    // Lumped joint masses (the mass source): for the modal case, the diaphragms' centres of mass, user
    // wind loads at them and P-Delta from the mass.
    let groups = crate::diaphragm::groups(model)?;
    let skipped = crate::diaphragm::skipped_wind_rows(model, &groups);
    if !skipped.is_empty() {
        notes.push(format!(
            "Warning: user wind loads skipped, the diaphragm has no joints at that story: {}",
            skipped.join("; ")
        ));
    }
    let masses = if model.modal.is_some() || !groups.is_empty() || pd_method == PDeltaMethod::Mass {
        let corners = |ai: usize, qz: f64| -> Vec<f64> {
            shells[ai]
                .shell
                .area_load([0.0, 0.0, qz])
                .iter()
                .map(|f| f[2])
                .collect()
        };
        mass::lumped(model, &corners)
    } else {
        Vec::new()
    };
    // Joints with something attached (user wind loads tied to the nearest joint use only those).
    let mut attached = vec![false; n_nodes];
    for id in model
        .frames
        .iter()
        .flat_map(|f| f.nodes.iter())
        .chain(model.areas.iter().flat_map(|a| a.nodes.iter()))
        .chain(model.links.iter().flat_map(|l| l.nodes.iter()))
        .chain(model.supports.iter().map(|s| &s.node))
        .chain(model.springs.iter().map(|s| &s.node))
    {
        attached[node_index[id]] = true;
    }
    // The floor element of a semi-rigid level under a point (in plan), with its shape function values.
    let shell_nodes: Vec<Vec<usize>> = shells.iter().map(|sh| sh.nodes.clone()).collect();
    let floor = |g: &crate::diaphragm::Level, p: (f64, f64)| -> Option<Vec<(usize, f64)>> {
        let mine: std::collections::HashSet<usize> = g.joints.iter().copied().collect();
        shell_nodes.iter().find_map(|nodes| {
            if !nodes.iter().all(|n| mine.contains(n)) {
                return None;
            }
            let pts: Vec<[f64; 2]> = nodes
                .iter()
                .map(|&n| [model.nodes[n].x, model.nodes[n].y])
                .collect();
            let s = crate::area::plan_shape(&pts, [p.0, p.1])?;
            Some(nodes.iter().copied().zip(s).collect())
        })
    };
    let element_index: HashMap<&Id, usize> = model
        .frames
        .iter()
        .enumerate()
        .map(|(i, e)| (&e.id, i))
        .collect();
    let area_index: HashMap<&Id, usize> = model
        .areas
        .iter()
        .enumerate()
        .map(|(i, a)| (&a.id, i))
        .collect();
    clock.lap("analysis: masses");

    // Number equations in a fill-reducing order: joints that share an element, and the carriers of a
    // tied joint (its mass couples them), are neighbours.
    let no_equation: Vec<bool> = (0..n_nodes * NDF)
        .map(|k| restrained[k] || !ties.own[k])
        .collect();
    let mut connections: Vec<[usize; 2]> = blocks
        .iter()
        .flat_map(|b| {
            let n = &b.nodes;
            (0..n.len()).flat_map(move |i| (i + 1..n.len()).map(move |j| [n[i], n[j]]))
        })
        .filter(|[a, b]| a != b)
        .collect();
    for parts in ties.parts.iter().flatten() {
        for (i, (a, _)) in parts.iter().enumerate() {
            for (b, _) in &parts[i + 1..] {
                connections.push([*a, *b]);
            }
        }
    }
    let map = DofMap::new(n_nodes, &no_equation, &connections);
    timing::note(|| {
        let (count, largest) = map.pattern.supernodes();
        let entries = map.pattern.factor_entries();
        format!(
            "{} joints, {} equations; factor {} entries ({:.0} MB), {} supernodes, largest front {}",
            n_nodes,
            map.n_eq,
            entries,
            entries as f64 * 8e-6,
            count,
            largest
        )
    });
    clock.lap("analysis: equation order and factor structure");
    // Will it fit in memory? The factor's size is known now, before anything large is made (see
    // `crate::memory`).
    {
        let single = |c: &&crate::model::LoadCombination| combine::is_single(model, c);
        // Running maximum and minimum of a combination with two values: one pair per level of such
        // combinations inside it.
        fn levels(model: &Model, c: &crate::model::LoadCombination) -> usize {
            1 + c
                .items
                .iter()
                .filter_map(|it| it.combo.as_ref())
                .filter_map(|id| model.load_combinations.iter().find(|x| &x.id == id))
                .filter(|sub| !combine::is_single(model, sub))
                .map(|sub| levels(model, sub))
                .max()
                .unwrap_or(0)
        }
        let need = crate::memory::Need::of(&crate::memory::Sizes {
            equations: map.n_eq,
            joints: n_nodes,
            frames: elems.len(),
            shells: shells.len(),
            corners: shells.iter().map(|sh| sh.nodes.len()).sum(),
            stations: STATIONS,
            factor_entries: map.pattern.factor_entries(),
            work: map.pattern.work_bytes(crate::parallel::threads()),
            batch: SOLVE_BATCH.min(
                model.load_cases.len() + model.load_combinations.iter().filter(single).count(),
            ),
            modes: model.modal.as_ref().map_or(0, |mc| mc.modes),
            running_levels: model
                .load_combinations
                .iter()
                .filter(|c| !single(c))
                .map(|c| levels(model, c))
                .sum(),
        });
        timing::note(|| need.describe(crate::memory::available()));
        need.check(map.n_eq)?;
    }
    let mut stiffness = SymMatrix::new(map.pattern.clone());
    assemble(&blocks, &map, &mut stiffness);
    clock.lap("analysis: assembly");
    // An unstable structure (a direction without stiffness) stops the analysis, or goes on with a tiny
    // stiffness in each such direction (`mended`: the equations, also of the factorizations with
    // P-Delta), as the model asks.
    let mend = model.on_instability == crate::model::OnInstability::Continue;
    let mut mended: Vec<crate::sparse::Mended> = Vec::new();
    let factorized = if mend {
        let (k, cond, found) = stiffness.factorize_mended();
        mended = found;
        Ok((k, cond))
    } else {
        stiffness.factorize()
    };
    // The equations mended in the factor now in use: the modal case holds them.
    let mut held: Vec<usize> = mended.iter().map(|m| m.equation).collect();
    clock.lap("analysis: factorization");
    let mut k = match factorized {
        Err(eq) => {
            let (node, d) = map.locate(eq);
            return Err(Error::Unstable {
                node: model.nodes[node].id.clone(),
                dof: DOF_NAMES[d],
            });
        }
        Ok((k, cond)) => {
            if cond.worst_ratio < ILL_CONDITIONED {
                let (node, d) = map.locate(cond.worst_equation);
                notes.push(format!(
                    "Ill-conditioned stiffness: about {:.0} of 16 digits lost at node {} {} \
                     (e.g. a very stiff member next to a very soft one); check the results",
                    -cond.worst_ratio.log10(),
                    model.nodes[node].id,
                    DOF_NAMES[d]
                ));
            }
            k
        }
    };
    // P-Delta (see `pdelta`): from here on the stiffness carries the geometric stiffness. From the mass:
    // negative springs at the joints with mass. From loads: the P-Delta load's iterations on the
    // stiffness just factorized give each frame its axial force, so its geometric stiffness.
    let mut mass_springs = vec![[0.0f64; NDF]; n_nodes];
    // Centres of rigidity of rigid levels (3D only): unit Fx, Fy and Mz at the master, the other levels
    // free; the point where a force moves the level without turning it. All the unit loads are solved at
    // once on a factorized stiffness: the one without the geometric stiffness of P-Delta from loads
    // (worked out before that P-Delta, so that factor need not be kept), else the final one (the mass
    // method's springs are in it).
    let centres_of_rigidity = |factor: &Factor| -> HashMap<usize, (f64, f64)> {
        let mut rigidity: HashMap<usize, (f64, f64)> = HashMap::new();
        if model.dimension != crate::model::Dimension::ThreeD {
            return rigidity;
        }
        let rigid: Vec<usize> = (0..groups.len())
            .filter(|&gi| !groups[gi].semi_rigid)
            .collect();
        let units: Vec<(usize, usize)> = rigid
            .iter()
            .flat_map(|&gi| [0, 1, 5].map(|d| (groups[gi].joints[0], d)))
            .collect();
        let nrhs = units.len();
        let mut rhs = vec![0.0f64; map.n_eq * nrhs];
        let mut any = vec![false; nrhs];
        for (r, &(n, d)) in units.iter().enumerate() {
            let mut f = [0.0; NDF];
            f[d] = 1.0;
            for (c, fc) in ties.carried(n, &f) {
                for (dd, v) in fc.iter().enumerate() {
                    if let Some(e) = map.eq[c * NDF + dd] {
                        rhs[e * nrhs + r] += v;
                        any[r] |= *v != 0.0;
                    }
                }
            }
        }
        if nrhs > 0 {
            factor.solve_many(&mut rhs, nrhs);
        }
        // The rotation about Z of the master under unit load r (None when it reaches no equation).
        let mut u = vec![0.0f64; n_nodes * NDF];
        let mut turn = |r: usize| -> Option<f64> {
            if !any[r] {
                return None;
            }
            for (kk, e) in map.eq.iter().enumerate() {
                u[kk] = e.map_or(0.0, |e| rhs[e * nrhs + r]);
            }
            ties.place(&mut u);
            let m = units[r].0;
            Some(u[m * NDF + 5])
        };
        for (i, &gi) in rigid.iter().enumerate() {
            let g = &groups[gi];
            let m = g.joints[0];
            if let (Some(tx), Some(ty), Some(tm)) = (turn(3 * i), turn(3 * i + 1), turn(3 * i + 2))
            {
                if tm != 0.0 {
                    let (x0, y0) = (model.nodes[m].x, model.nodes[m].y);
                    rigidity.insert(gi, (x0 - ty / tm, y0 + tx / tm));
                }
            }
        }
        rigidity
    };
    let mut rigidity = (pd_method == PDeltaMethod::Loads).then(|| centres_of_rigidity(&k));
    if pd_method != PDeltaMethod::None {
        if pd_method == PDeltaMethod::Mass {
            mass_springs = crate::pdelta::mass_springs(model, &masses);
            for n in (0..n_nodes).filter(|&n| mass_springs[n].iter().any(|k| *k != 0.0)) {
                let mut flat = vec![0.0; NDF * NDF];
                for d in 0..NDF {
                    flat[d * NDF + d] = mass_springs[n][d];
                }
                blocks.push(Block::carried(&[n], flat, &ties));
            }
        } else {
            let load = model.p_delta_load().expect("the loads method has a load");
            let cx = LoadContext {
                model,
                elems: &elems,
                shells: &shells,
                node_index: &node_index,
                element_index: &element_index,
                area_index: &area_index,
                inactive,
                ties: &ties,
                map: &map,
                auto_restrained: &auto_restrained,
                groups: &groups,
                masses: &masses,
                attached: &attached,
                floor: &floor,
            };
            let (loaded, _) = case_loads(&cx, &load, None, &mut notes)?;
            let (axial, u) = p_delta_axial(
                &elems,
                &shells,
                &blocks,
                &ties,
                &map,
                &mut followers,
                &k,
                &loaded,
                mend,
                &mut notes,
            )?;
            for (i, (el, p)) in elems.iter_mut().zip(axial).enumerate() {
                el.props.pd = p;
                el.stiffen();
                let flat: Vec<f64> = el.k_global.iter().flatten().copied().collect();
                blocks[i] = Block::carried(&el.nodes, flat, &ties);
            }
            for (i, sh) in shells.iter_mut().enumerate() {
                let kg = sh.geometric(&u);
                for (a, b) in sh.k_global.iter_mut().zip(kg) {
                    *a += b;
                }
                blocks[elems.len() + i] = Block::carried(&sh.nodes, sh.k_global.clone(), &ties);
            }
        }
        stiffness = SymMatrix::new(map.pattern.clone());
        assemble(&blocks, &map, &mut stiffness);
        // The first factor is not needed again: freed before the next is made, so only one is in
        // memory at a time.
        k.free();
        let (f, _, negative, first) = if mend {
            let (f, found) = stiffness.factorize_indefinite_mended();
            held = found.iter().map(|m| m.equation).collect();
            mended.extend(found);
            f
        } else {
            stiffness.factorize_indefinite().map_err(|eq| {
                let (node, d) = map.locate(eq);
                Error::Unstable {
                    node: model.nodes[node].id.clone(),
                    dof: DOF_NAMES[d],
                }
            })?
        };
        if let (true, Some(eq)) = (negative > 0, first) {
            let (node, _) = map.locate(eq);
            notes.push(format!(
                "P-Delta: the stiffness has {negative} negative pivot(s) (the first near node {}): \
                 the structure buckles under the P-Delta load; results given anyway",
                model.nodes[node].id
            ));
        }
        k = f;
        clock.lap("analysis: P-Delta");
    }
    // The directions without stiffness, in the model's order (each once, with the most digits lost).
    let instabilities: Vec<Instability> = if mended.is_empty() {
        Vec::new()
    } else {
        let mut dof_of = vec![0usize; map.n_eq];
        for (kk, e) in map.eq.iter().enumerate() {
            if let Some(e) = e {
                dof_of[*e] = kk;
            }
        }
        let mut lost: std::collections::BTreeMap<usize, f64> = std::collections::BTreeMap::new();
        for m in &mended {
            let digits = lost.entry(dof_of[m.equation]).or_insert(0.0);
            *digits = digits.max(m.digits_lost());
        }
        lost.into_iter()
            .map(|(kk, digits_lost)| Instability {
                node: model.nodes[kk / NDF].id.clone(),
                dof: DOF_NAMES[kk % NDF],
                digits_lost,
            })
            .collect()
    };
    if !instabilities.is_empty() {
        const LISTED: usize = 20;
        let mut list: Vec<String> = instabilities
            .iter()
            .take(LISTED)
            .map(|i| format!("{}:{} ({:.0} digits lost)", i.node, i.dof, i.digits_lost))
            .collect();
        if instabilities.len() > LISTED {
            list.push(format!(
                "and {} more (all are listed with the results)",
                instabilities.len() - LISTED
            ));
        }
        notes.push(format!(
            "Warning: the structure is unstable. It has no stiffness in {} direction(s) (a part that \
             is not held, or a mechanism); a tiny stiffness was put in each to continue. Results at \
             and near these joints are not reliable: {}",
            instabilities.len(),
            list.join(", ")
        ));
    }
    let rigidity = rigidity.take().unwrap_or_else(|| centres_of_rigidity(&k));
    let diaphragm_centers = crate::diaphragm::centres(model, &groups, &masses, &rigidity);
    clock.lap("analysis: diaphragm centres");
    // The modal case, on the factorized stiffness. A Ritz case's starting loads on the equations: an
    // acceleration's is each joint's mass in that direction, a load pattern's its loads as in a load case.
    let starts = match &model.modal {
        Some(mc) if mc.kind == ModalType::Ritz => {
            let cx = LoadContext {
                model,
                elems: &elems,
                shells: &shells,
                node_index: &node_index,
                element_index: &element_index,
                area_index: &area_index,
                inactive,
                ties: &ties,
                map: &map,
                auto_restrained: &auto_restrained,
                groups: &groups,
                masses: &masses,
                attached: &attached,
                floor: &floor,
            };
            let mut out = Vec::with_capacity(mc.starts.len());
            for st in &mc.starts {
                let load = match (st.accel, st.loads.as_deref()) {
                    (Some(a), _) => accel_load(
                        model,
                        &masses,
                        &ties,
                        &map,
                        a.dir(),
                        mass_centre(model, &masses),
                    ),
                    (None, Some(loads)) => case_loads(&cx, loads, None, &mut notes)?.1,
                    (None, None) => {
                        return Err(Error::Invalid(format!(
                            "modal case {}: starting load {} has no loads",
                            mc.id,
                            st.name()
                        )))
                    }
                };
                out.push(crate::ritz::Start {
                    load,
                    cycles: st.cycles,
                });
            }
            out
        }
        _ => Vec::new(),
    };
    let modal = match &model.modal {
        Some(mc) => {
            // Directions without stiffness do not move in the modal case (in the static cases they
            // have a tiny stiffness): with one they would be modes of their own, with huge periods.
            let pivots = k.hold(&held);
            let made = modal_case(
                model,
                mc,
                &starts,
                &k,
                &stiffness,
                &held,
                &map,
                &ties,
                &restrained,
                &masses,
                &mut notes,
            );
            k.release(pivots);
            if made.is_err() {
                *modal_failed = Some(mc.id.clone());
            }
            made?
        }
        None => None,
    };
    clock.lap("modal (total)");

    // Load cases, then one-valued combinations as cases with the scaled loads.
    let mut linear: Vec<(LoadCase, Option<ComboType>)> =
        model.load_cases.iter().map(|c| (c.clone(), None)).collect();
    for c in &model.load_combinations {
        if let Some(lc) = combine::as_linear_case(model, c) {
            linear.push((lc, Some(c.kind)));
        }
    }
    // A batch of cases at a time: their loads, one solve for all of them, then each case's forces,
    // handed on in order (combinations with a maximum and a minimum once their parts are known).
    // One case's results from its joint displacements `u` (ties and followers placed) and its loads:
    // element forces (frames at `layout` when given, else at equally spaced stations and both sides of
    // their point loads), reactions, the equilibrium check, the base reactions and the story results.
    let stories = crate::story::Stories::new(model);
    let pd_frames = pd_method == PDeltaMethod::Loads;
    // Piers and spandrels: their cuts, and the area each edge constraint belongs to.
    let walls = match &model.wall_sections {
        Some(w) => std::borrow::Cow::Borrowed(w),
        None => std::borrow::Cow::Owned(crate::piers::sections(model, &mut notes)),
    };
    let piers = crate::piers::Cuts::new(model, &walls);
    let edge_area: Vec<usize> = model
        .edge_constraints
        .iter()
        .map(|ec| area_index[&ec.area])
        .collect();
    let recover = |id: Id,
                   combination: Option<ComboType>,
                   u: Vec<f64>,
                   nodal: Vec<f64>,
                   fixed: Vec<[f64; 12]>,
                   elem_loads: Vec<Vec<LocalLoad>>,
                   shell_loads: Vec<Vec<f64>>,
                   applied_member: [f64; 6],
                   layout: Option<&[Vec<(f64, Side)>]>|
     -> CaseResults {
        let _recovery_time = timing::Span::new("recovery (total)");
        let frame_time = timing::Span::new("recovery: frames (incl. cuts)");
        // Element end forces, and per DOF what frames and springs need minus the applied load.
        let mut need = vec![0.0f64; n_nodes * NDF];
        // With P-Delta from loads, the frames' end forces with their geometric stiffness do not balance
        // their loads in moment: the sum of the frames' end forces (and so of the reactions) is off by the
        // P-Delta moments, gathered here (about the origin, with the member loads' moment) for the
        // equilibrium check.
        let mut geometric = [0.0f64; 6];
        // Story cuts: each element's joints, their positions and K u at them; its own loads given at its
        // joints (`own`, empty for none).
        let mut cuts = stories.as_ref().map(|s| s.zero()).unwrap_or_default();
        let mut wall_sums = piers.as_ref().map(|p| p.zero()).unwrap_or_default();
        let cut = |cuts: &mut Vec<[f64; 6]>, nodes: &[usize], f: &[f64], own: &[f64]| {
            let _cut_time = timing::Span::new("derived: story cuts");
            if let Some(st) = &stories {
                let place = |v: &dyn Fn(usize) -> f64| -> Vec<([f64; 3], [f64; 6])> {
                    nodes
                        .iter()
                        .enumerate()
                        .map(|(k, &n)| {
                            let g: [f64; 6] = std::array::from_fn(|d| v(k * NDF + d));
                            (coords(&model.nodes[n]), g)
                        })
                        .collect()
                };
                let load = |a: usize| own.get(a).copied().unwrap_or(0.0);
                let loads = place(&load);
                // The joints' forces on the element: K u less its own loads there. With P-Delta from loads,
                // their moment (about the origin): K u carries the P-Delta moment.
                let couple = pd_frames.then(|| {
                    let mut c = [0.0f64; 6];
                    for (k, &n) in nodes.iter().enumerate() {
                        add_about_origin(
                            &mut c,
                            coords(&model.nodes[n]),
                            &f[k * NDF..k * NDF + NDF],
                        );
                    }
                    [c[3], c[4], c[5]]
                });
                let zs: Vec<f64> = nodes.iter().map(|&n| model.nodes[n].z).collect();
                st.add(
                    cuts,
                    &place(&|a| f[a] - load(a)),
                    &zs,
                    &|zc, z| crate::story::joint_loads_above(&loads, zc, z),
                    couple,
                );
            }
        };
        let mut elem_results = Vec::with_capacity(elems.len());
        for ((el, fef), (src, loads)) in elems
            .iter()
            .zip(&fixed)
            .zip(model.frames.iter().zip(elem_loads))
        {
            let (end, ul, hinges) = el.end_forces(&u, fef, &loads);
            let mut end_global = axes::vec12_to_global(&el.axes, &end);
            if let Some(arms) = &el.arms {
                crate::arms::to_joints(&mut end_global, arms);
            }
            for a in 0..12 {
                need[el.nodes[a / NDF] * NDF + a % NDF] += end_global[a];
            }
            if pd_frames {
                for s in 0..2 {
                    let p = coords(&model.nodes[el.nodes[s]]);
                    add_about_origin(&mut geometric, p, &end_global[s * NDF..s * NDF + NDF]);
                }
            }
            if let Some(p) = &piers {
                let _pier_time = timing::Span::new("derived: pier cuts");
                let ends = &p.frame_ends[elem_results.len()];
                p.add(&mut wall_sums, ends, &el.nodes, &|k| {
                    std::array::from_fn(|d| end_global[k * NDF + d])
                });
            }
            let mut er = ElementResults {
                element: src.id.clone(),
                nodes: src.nodes.clone(),
                length: el.props.l,
                offsets: offsets[elem_results.len()],
                axes: el.axes,
                stations: Vec::new(),
                end_forces: end,
                loads,
                end_displacements: ul,
                hinges,
                enveloped: false,
                pieces: Vec::new(),
                pdelta: (el.props.pd != 0.0).then(|| {
                    Box::new(crate::results::PDeltaShape {
                        props: el.props,
                        ul,
                        hinges,
                        turn: 0.0,
                    })
                }),
            };
            match layout {
                Some(at) => er.set_stations(&at[elem_results.len()]),
                None => er.make_stations(STATIONS),
            }
            let xs: Vec<f64> = er.stations.iter().map(|s| s.x).collect();
            let disp = elements::local_displacements(
                &el.props,
                &ul,
                &er.end_forces,
                &er.loads,
                &hinges,
                &xs,
            );
            for (s, d) in er.stations.iter_mut().zip(disp) {
                [s.ux, s.uy, s.uz] = axes::to_global(&el.axes, d);
            }
            if let Some(st) = &stories {
                let _story_time = timing::Span::new("derived: story cuts");
                let object = st.object_ends(elem_results.len());
                let at = [0, 1].map(|s| {
                    let g: [f64; 6] = std::array::from_fn(|d| end_global[s * NDF + d]);
                    (coords(&model.nodes[el.nodes[s]]), g)
                });
                if pd_frames && el.props.kind == crate::model::FrameKind::Frame {
                    // The member's internal forces at the cut (the P-Delta moment along it included):
                    // on the positive face, from the part above; the story takes the opposite.
                    let section = |zc: f64, z: f64| {
                        let x = (zc - el.start[2]) / el.axes[0][2];
                        let q = er.forces_at_side(x, Side::Left);
                        let f = axes::to_global(&el.axes, [q[0], q[1], q[2]]);
                        let m = axes::to_global(&el.axes, [q[3], -q[4], q[5]]);
                        let r = axes::sub(
                            axes::add(el.start, axes::scale(el.axes[0], x)),
                            [0.0, 0.0, z],
                        );
                        (
                            axes::scale(f, -1.0),
                            axes::scale(axes::add(m, axes::cross(r, f)), -1.0),
                        )
                    };
                    let (lo, hi) = (at[0].0[2].min(at[1].0[2]), at[0].0[2].max(at[1].0[2]));
                    st.add_section(&mut cuts, lo, hi, &object, &section);
                } else {
                    let loads_above = |_: f64, z: f64| {
                        crate::story::frame_loads_above(
                            el.start, &el.axes, el.props.l, &er.loads, z,
                        )
                    };
                    st.add(&mut cuts, &at, &object, &loads_above, None);
                }
            }
            if el.turn != 0.0 {
                to_frame_axes(&mut er, &el.geo, el.turn);
            }
            elem_results.push(er);
        }
        drop(frame_time);
        let shell_time = timing::Span::new("recovery: shells (incl. cuts)");
        // Shell corner forces (K u) and the reported per-length forces at the corners.
        let mut shell_results = Vec::with_capacity(shells.len());
        for (sh, area) in shells.iter().zip(&model.areas) {
            let m = NDF * sh.nodes.len();
            let mut ug: Vec<f64> = (0..m)
                .map(|a| u[sh.nodes[a / NDF] * NDF + a % NDF])
                .collect();
            let fe: Vec<f64> = (0..m)
                .map(|a| (0..m).map(|b| sh.k_global[a * m + b] * ug[b]).sum::<f64>())
                .collect();
            for a in 0..m {
                need[sh.nodes[a / NDF] * NDF + a % NDF] += fe[a];
            }
            if pd_frames {
                for (c, &n) in sh.nodes.iter().enumerate() {
                    let p = coords(&model.nodes[n]);
                    add_about_origin(&mut geometric, p, &fe[c * NDF..c * NDF + NDF]);
                }
            }
            let own = &shell_loads[shell_results.len()];
            cut(&mut cuts, &sh.nodes, &fe, own);
            if let Some(p) = &piers {
                let _pier_time = timing::Span::new("derived: pier cuts");
                let ends = &p.area_ends[shell_results.len()];
                let own = |a: usize| own.get(a).copied().unwrap_or(0.0);
                p.add(&mut wall_sums, ends, &sh.nodes, &|k| {
                    std::array::from_fn(|d| fe[k * NDF + d] - own(k * NDF + d))
                });
            }
            if let Some(e) = &sh.arm {
                crate::arms::to_ends(&mut ug, e);
            }
            let f = sh.shell.corner_forces(&ug);
            shell_results.push(ShellResults {
                element: area.id.clone(),
                area: area.id.clone(),
                nodes: area.nodes.clone(),
                axes: sh.shell.axes,
                corners: area
                    .nodes
                    .iter()
                    .zip(f)
                    .map(|(n, v)| ShellCorner::new(n.clone(), v))
                    .collect(),
            });
        }
        drop(shell_time);
        let hidden_time = timing::Span::new("recovery: hidden elements (incl. cuts)");
        for ((ids, kc), &ai) in wall_springs.iter().zip(&spring_area) {
            let ug: Vec<f64> = (0..2 * NDF)
                .map(|a| u[ids[a / NDF] * NDF + a % NDF])
                .collect();
            let fe: Vec<f64> = (0..2 * NDF)
                .map(|a| {
                    (0..2 * NDF)
                        .map(|b| kc[a * 2 * NDF + b] * ug[b])
                        .sum::<f64>()
                })
                .collect();
            for a in 0..2 * NDF {
                need[ids[a / NDF] * NDF + a % NDF] += fe[a];
            }
            cut(&mut cuts, ids, &fe, &[]);
            if let Some(p) = &piers {
                let _pier_time = timing::Span::new("derived: pier cuts");
                p.add_hidden(&mut wall_sums, &p.area_ends[ai], ids, &|k| {
                    std::array::from_fn(|d| fe[k * NDF + d])
                });
            }
        }
        for ((ids, kc), &ai) in edges.iter().zip(&edge_area) {
            let ug: Vec<f64> = (0..3 * NDF)
                .map(|a| u[ids[a / NDF] * NDF + a % NDF])
                .collect();
            let fe: Vec<f64> = (0..3 * NDF)
                .map(|a| {
                    (0..3 * NDF)
                        .map(|b| kc[a * 3 * NDF + b] * ug[b])
                        .sum::<f64>()
                })
                .collect();
            for a in 0..3 * NDF {
                need[ids[a / NDF] * NDF + a % NDF] += fe[a];
            }
            cut(&mut cuts, ids, &fe, &[]);
            if let Some(p) = &piers {
                let _pier_time = timing::Span::new("derived: pier cuts");
                p.add_hidden(&mut wall_sums, &p.area_ends[ai], ids, &|k| {
                    std::array::from_fn(|d| fe[k * NDF + d])
                });
            }
        }
        drop(hidden_time);
        let reaction_time = timing::Span::new("recovery: reactions, equilibrium, displacements");
        for kk in 0..n_nodes * NDF {
            need[kk] +=
                (spring[kk / NDF][kk % NDF] + mass_springs[kk / NDF][kk % NDF]) * u[kk] - nodal[kk];
        }
        for n in (0..n_nodes).filter(|&n| has_spread[n]) {
            for r in 0..NDF {
                need[n * NDF + r] += (0..NDF)
                    .map(|c| spread[n][r][c] * u[n * NDF + c])
                    .sum::<f64>();
            }
        }
        // `need` at a slave is the force its rigid links or diaphragm supply. Carried to the masters,
        // it must be zero at free DOFs (K u = F; what is left is the solver residual) and is the
        // support reaction at restrained DOFs.
        let mut at_carrier = vec![0.0f64; n_nodes * NDF];
        for n in 0..n_nodes {
            for (c, fc) in ties.carried(n, &need[n * NDF..n * NDF + NDF]) {
                for (d, v) in fc.iter().enumerate() {
                    at_carrier[c * NDF + d] += v;
                }
            }
        }
        // Force each spring applies to the structure.
        let spring_force = |n: usize| -> [f64; 6] {
            let mut f = [0.0; 6];
            for d in 0..NDF {
                f[d] = -spring[n][d] * u[n * NDF + d]
                    - (0..NDF)
                        .map(|c| spread[n][d][c] * u[n * NDF + c])
                        .sum::<f64>();
            }
            f
        };

        // Equilibrium uses support reactions and spring forces only, so it fails if any load is
        // lost or the solve is wrong; the residual is reported separately. The P-Delta mass method's
        // springs count like springs (and so in the base reaction), though not as joint reactions.
        if pd_frames {
            for d in 0..6 {
                geometric[d] += applied_member[d];
            }
        }
        let mut applied = applied_member;
        let mut react_sum = [0.0f64; 6];
        let mut residual_max = 0.0f64;
        for n in 0..n_nodes {
            let p = coords(&model.nodes[n]);
            add_about_origin(&mut applied, p, &nodal[n * NDF..n * NDF + NDF]);
            let mut r = spring_force(n);
            for d in 0..NDF {
                r[d] -= mass_springs[n][d] * u[n * NDF + d];
            }
            for d in 0..NDF {
                let k = n * NDF + d;
                if !ties.own[k] {
                    continue;
                }
                if restrained[k] {
                    r[d] += at_carrier[k];
                } else {
                    residual_max = residual_max.max(at_carrier[k].abs());
                }
            }
            add_about_origin(&mut react_sum, p, &r);
        }
        let scale = applied.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let rel = |v: f64| if scale > 0.0 { v / scale } else { v };
        let worst = (0..6).fold(0.0f64, |m, d| {
            m.max((applied[d] + react_sum[d] - geometric[d]).abs())
        });
        let relative_error = rel(worst);
        let residual = rel(residual_max);

        let displacements = (0..n_nodes)
            .map(|n| {
                let mut v = [0.0; 6];
                v.copy_from_slice(&u[n * NDF..n * NDF + NDF]);
                NodeValues {
                    node: model.nodes[n].id.clone(),
                    values: v.into(),
                }
            })
            .collect();
        let reactions = (0..n_nodes)
            .filter(|&n| supported[n] || spring[n].iter().any(|k| *k > 0.0) || has_spread[n])
            .map(|n| {
                // Spring forces, plus support reactions at the DOFs the user restrained.
                let mut v = spring_force(n);
                for d in 0..NDF {
                    if user_restrained[n * NDF + d] {
                        v[d] += at_carrier[n * NDF + d];
                    }
                }
                NodeValues {
                    node: model.nodes[n].id.clone(),
                    values: v.into(),
                }
            })
            .collect();

        drop(reaction_time);
        let derived_time = timing::Span::new("derived: drifts, story and pier tables");
        let joint_drifts = stories
            .as_ref()
            .map(|s| s.drifts(model, &u))
            .unwrap_or_default();
        let (pier_forces, spandrel_forces) = piers
            .as_ref()
            .map(|p| p.forces(&wall_sums))
            .unwrap_or_default();
        let story_drifts = crate::story::story_drifts(&joint_drifts);
        let story_forces = stories
            .as_ref()
            .map(|s| s.forces(&cuts))
            .unwrap_or_default();
        drop(derived_time);
        let links = {
            let _link_time = timing::Span::new("recovery: links");
            link_results(model, &bodies, &need, &u)
        };
        CaseResults {
            id,
            combination,
            step: None,
            displacements,
            reactions,
            frames: elem_results,
            shells: shell_results,
            links,
            equilibrium: Some(Equilibrium {
                applied,
                reactions: react_sum,
                relative_error,
                residual,
            }),
            spectrum: false,
            base_reaction: Some(react_sum),
            joint_drifts: joint_drifts.clone(),
            story_drifts,
            story_forces,
            pier_forces,
            spandrel_forces,
        }
    };

    let props: Vec<Props> = elems.iter().map(|e| e.props).collect();
    // Where each case has point loads inside frames: a combination with a maximum and a minimum has
    // stations at both sides of its parts' point loads, and takes its parts as they are solved.
    let mut points: HashMap<Id, Vec<(usize, f64)>> = HashMap::new();
    for (c, _) in &linear {
        let mut at = Vec::new();
        for ml in &c.frame_loads {
            if !matches!(ml, FrameLoad::Point { .. }) {
                continue;
            }
            let ei = element_index[ml.frame()];
            let el = &elems[ei];
            let l = el.props.l;
            let tol = 1e-9 * l;
            for ld in resolve(ml, &el.geo, &el.axes, l).1 {
                match ld {
                    LocalLoad::Point { at: x, .. } if x > tol && x < l - tol => at.push((ei, x)),
                    _ => {}
                }
            }
        }
        if !at.is_empty() {
            points.insert(c.id.clone(), at);
        }
    }
    let mut order = combine::Order::new(model, &props, &points);

    // Response spectrum cases: every mode's results at unit amplitude, then each case's rules value by
    // value. Their frames have stations at both sides of every load case's point loads too, so each
    // combination finds its own stations among them.
    if !model.spectrum_cases.is_empty() {
        // Response spectrum cases without modes to work on: the modal case's failure (see
        // `analyze_each`; nothing has been handed on yet).
        let Some(modal) = &modal else {
            *modal_failed = model.modal.as_ref().map(|mc| mc.id.clone());
            return Err(Error::Invalid(
                "spectrum cases: the modal case has no modes (the model has no mass where it can move)"
                    .into(),
            ));
        };
        // Static modes of a Ritz case carry no mass: the spectrum uses the finite modes only.
        let finite;
        let modal = if modal.modes.iter().any(|md| md.static_mode) {
            finite = ModalResults {
                id: modal.id.clone(),
                modes: modal
                    .modes
                    .iter()
                    .filter(|md| !md.static_mode)
                    .cloned()
                    .collect(),
                masses: Vec::new(),
                free_mass: modal.free_mass,
                centre_of_mass: modal.centre_of_mass,
                load_participation: Vec::new(),
            };
            &finite
        } else {
            modal
        };
        if modal.modes.is_empty() {
            *modal_failed = model.modal.as_ref().map(|mc| mc.id.clone());
            return Err(Error::Invalid(
                "spectrum cases: the modal case has only static modes".into(),
            ));
        }
        let mut points: Vec<Vec<f64>> = vec![Vec::new(); elems.len()];
        for c in &model.load_cases {
            for ml in &c.frame_loads {
                let ei = element_index[ml.frame()];
                let el = &elems[ei];
                for ld in resolve(ml, &el.geo, &el.axes, el.props.l).1 {
                    if let LocalLoad::Point { at, .. } = ld {
                        points[ei].push(at);
                    }
                }
            }
        }
        let layouts: Vec<Vec<(f64, Side)>> = elems
            .iter()
            .zip(&offsets)
            .zip(points)
            .map(|((el, off), pts)| {
                crate::results::station_layout(off[0], el.props.l - off[1], pts, STATIONS)
            })
            .collect();
        let mut template: Option<CaseResults> = None;
        let mut unit: Vec<Vec<f64>> = Vec::with_capacity(modal.modes.len());
        for mode in &modal.modes {
            let mut u = vec![0.0f64; n_nodes * NDF];
            for (n, d) in mode.shape.iter().enumerate() {
                u[n * NDF..n * NDF + NDF].copy_from_slice(&d.values.as_array());
            }
            followers.place(&mut u);
            let r = recover(
                modal.id.clone(),
                None,
                u,
                vec![0.0; n_nodes * NDF],
                vec![[0.0; 12]; elems.len()],
                vec![Vec::new(); elems.len()],
                vec![Vec::new(); shells.len()],
                [0.0; 6],
                Some(&layouts),
            );
            unit.push(combine::values(&r));
            template.get_or_insert_with(|| combine::envelope_template(&r));
        }
        let template = template.expect("the modal case has a mode");
        // Value by value, the modes side by side.
        let m = unit.len();
        let mut by_value = vec![0.0f64; unit[0].len() * m];
        for (i, v) in unit.iter().enumerate() {
            for (p, x) in v.iter().enumerate() {
                by_value[p * m + i] = *x;
            }
        }
        drop(unit);
        let data: Vec<spectrum::ModeData> = modal
            .modes
            .iter()
            .map(|md| spectrum::ModeData {
                period: md.period,
                participation: [
                    md.participation[0],
                    md.participation[1],
                    md.participation[2],
                ],
            })
            .collect();
        // Diaphragm eccentricity, per story: each mode's story shear in X and Y at unit amplitude (with
        // story levels, the story forces VX, VY at the story's bottom cut: the mass above it, outside the
        // diaphragms too, and nothing through a rigid link; without them, the inertial forces of the
        // diaphragm joints at and above the story); per diaphragm at the story, its extent in X and Y, its
        // share of the story's mass and its joints' shares of its own.
        struct Floor {
            extent: [f64; 2],
            share: f64,
            joints: Vec<(usize, f64)>,
        }
        struct Story {
            shear: [Vec<f64>; 2],
            floors: Vec<Floor>,
        }
        let mut stories: Vec<Story> = Vec::new();
        if model.spectrum_cases.iter().any(|c| c.eccentricity > 0.0) {
            // A level's height: its highest joint (a joint between two levels belongs to the one above).
            let height = |g: &crate::diaphragm::Level| {
                g.joints
                    .iter()
                    .map(|&n| model.nodes[n].z)
                    .fold(f64::NEG_INFINITY, f64::max)
            };
            let mut names: Vec<(&str, f64)> = Vec::new();
            for g in &groups {
                match names.iter_mut().find(|(n, _)| *n == g.story) {
                    Some((_, z)) => *z = z.max(height(g)),
                    None => names.push((&g.story, height(g))),
                }
            }
            names.sort_by(|a, b| a.1.total_cmp(&b.1));
            let at = |name: &str| names.iter().position(|(n, _)| *n == name).unwrap();
            let force = |g: &crate::diaphragm::Level, d: usize| -> Vec<f64> {
                modal
                    .modes
                    .iter()
                    .map(|md| {
                        g.joints
                            .iter()
                            .map(|&n| {
                                md.eigenvalue * masses[n][d] * md.shape[n].values.as_array()[d]
                            })
                            .sum()
                    })
                    .collect()
            };
            let mass = |g: &crate::diaphragm::Level| -> f64 {
                g.joints.iter().map(|&n| masses[n][0]).sum()
            };
            // With story levels: every story, bottom first, its name, its share of the values (the
            // story forces come last, 12 per story: top, then bottom P VX VY T MX MY) and its mass.
            let levels = &model.stories;
            let cut = by_value.len() / m - 12 * template.story_forces.len();
            let tol = 1e-6 * levels.iter().fold(1.0f64, |a, l| a.max(l.z.abs()));
            let story_mass = |s: usize| -> f64 {
                (0..n_nodes)
                    .filter(|&n| {
                        let z = model.nodes[n].z;
                        z > levels[s].z + tol && z <= levels[s + 1].z + tol
                    })
                    .map(|n| masses[n][0])
                    .sum()
            };
            let order: Vec<String> = if template.story_forces.is_empty() {
                names.iter().map(|(n, _)| n.to_string()).collect()
            } else {
                template
                    .story_forces
                    .iter()
                    .map(|f| f.story.clone())
                    .collect()
            };
            for (i, name) in order.iter().enumerate() {
                let mut shear = [vec![0.0; m], vec![0.0; m]];
                if template.story_forces.is_empty() {
                    for g in groups.iter().filter(|g| at(&g.story) >= i) {
                        for (d, sd) in shear.iter_mut().enumerate() {
                            for (s, f) in sd.iter_mut().zip(force(g, d)) {
                                *s += f;
                            }
                        }
                    }
                } else {
                    for (d, sd) in shear.iter_mut().enumerate() {
                        let at = (cut + 12 * i + 7 + d) * m;
                        sd.copy_from_slice(&by_value[at..at + m]);
                    }
                }
                let here: Vec<&crate::diaphragm::Level> =
                    groups.iter().filter(|g| g.story == *name).collect();
                let total: f64 = if template.story_forces.is_empty() {
                    here.iter().map(|g| mass(g)).sum()
                } else {
                    story_mass(i)
                };
                let floors = here
                    .iter()
                    .filter(|g| mass(g) > 0.0)
                    .map(|g| {
                        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
                        for &n in &g.joints {
                            let p = [model.nodes[n].x, model.nodes[n].y];
                            for a in 0..2 {
                                lo[a] = lo[a].min(p[a]);
                                hi[a] = hi[a].max(p[a]);
                            }
                        }
                        let own = mass(g);
                        Floor {
                            extent: [hi[0] - lo[0], hi[1] - lo[1]],
                            share: own / total,
                            joints: g
                                .joints
                                .iter()
                                .filter(|&&n| masses[n][0] > 0.0)
                                .map(|&n| (n, masses[n][0] / own))
                                .collect(),
                        }
                    })
                    .collect();
                stories.push(Story { shear, floors });
            }
        }
        clock.lap("spectrum: modes' results");
        for sc in &model.spectrum_cases {
            let rule = spectrum::Rule::new(sc, &model.spectrum_functions, &data);
            let mut scratch = Vec::with_capacity(m);
            let mut v: Vec<f64> = by_value
                .chunks(m)
                .map(|col| rule.combine(col, &mut scratch))
                .collect();
            // The torques: the story's forces in X and Y, the sizes of the differences of the case's
            // combined story shears; on each diaphragm the larger of ratio x By x FX and ratio x Bx x FY
            // times its share of the story's mass, as moments at its joints by their mass; their static
            // response added as a size.
            if sc.eccentricity > 0.0 && !stories.is_empty() {
                let shears: Vec<[f64; 2]> = stories
                    .iter()
                    .map(|st| st.shear.clone().map(|sd| rule.combine(&sd, &mut scratch)))
                    .collect();
                let mut nodal = vec![0.0f64; n_nodes * NDF];
                let mut rhs = vec![0.0f64; map.n_eq];
                for (i, st) in stories.iter().enumerate() {
                    let up = shears.get(i + 1).copied().unwrap_or([0.0; 2]);
                    let (fx, fy) = ((shears[i][0] - up[0]).abs(), (shears[i][1] - up[1]).abs());
                    for f in &st.floors {
                        let t =
                            sc.eccentricity * f.share * (f.extent[1] * fx).max(f.extent[0] * fy);
                        for &(n, part) in &f.joints {
                            nodal[n * NDF + 5] += t * part;
                            let mut load = [0.0; NDF];
                            load[5] = t * part;
                            for (c, fc) in ties.carried(n, &load) {
                                for (d, x) in fc.iter().enumerate() {
                                    if let Some(e) = map.eq[c * NDF + d] {
                                        rhs[e] += x;
                                    }
                                }
                            }
                        }
                    }
                }
                k.solve(&mut rhs);
                let mut u = vec![0.0f64; n_nodes * NDF];
                for (kk, e) in map.eq.iter().enumerate() {
                    if let Some(e) = e {
                        u[kk] = rhs[*e];
                    }
                }
                ties.place(&mut u);
                followers.place(&mut u);
                let torque = recover(
                    sc.id.clone(),
                    None,
                    u,
                    nodal,
                    vec![[0.0; 12]; elems.len()],
                    vec![Vec::new(); elems.len()],
                    vec![Vec::new(); shells.len()],
                    [0.0; 6],
                    Some(&layouts),
                );
                for (x, t) in v.iter_mut().zip(combine::values(&torque)) {
                    *x += t.abs();
                }
            }
            let mut r = combine::with_values(&template, &v);
            r.id = sc.id.clone();
            r.spectrum = true;
            clock.lap("spectrum: cases");
            order.push(r, out)?;
            clock.lap("spectrum: combinations and output");
        }
    }
    let cx = LoadContext {
        model,
        elems: &elems,
        shells: &shells,
        node_index: &node_index,
        element_index: &element_index,
        area_index: &area_index,
        inactive,
        ties: &ties,
        map: &map,
        auto_restrained: &auto_restrained,
        groups: &groups,
        masses: &masses,
        attached: &attached,
        floor: &floor,
    };
    // The order to solve in: every case at its turn in the results' order; a one-valued combination
    // that is part of a combination with a maximum and a minimum listed before it also ahead of that
    // one (`ahead`: only its values are used then, and its messages are left for its turn).
    let linear_index: HashMap<&Id, usize> = linear
        .iter()
        .enumerate()
        .map(|(i, (c, _))| (&c.id, i))
        .collect();
    let plan = order.plan();
    for batch in plan.chunks(SOLVE_BATCH) {
        let mut loaded: Vec<Loaded> = Vec::with_capacity(batch.len());
        let mut rhs_all = vec![0.0f64; map.n_eq * batch.len()];
        for (id, ahead) in batch {
            let (case, combination) = &linear[linear_index[id]];
            clock.lap("cases: other");
            let mut unused = Vec::new();
            let notes = if *ahead { &mut unused } else { &mut notes };
            let (l, rhs) = case_loads(&cx, case, *combination, notes)?;
            let r = loaded.len();
            for (e, v) in rhs.iter().enumerate() {
                rhs_all[e * batch.len() + r] = *v;
            }
            loaded.push(l);
            clock.lap("cases: loads");
        }
        k.solve_many(&mut rhs_all, batch.len());
        clock.lap("cases: solve");

        for (r, (l, (_, ahead))) in loaded.into_iter().zip(batch).enumerate() {
            let Loaded {
                id,
                combination,
                nodal,
                elem_loads,
                applied_member,
                fixed,
                shell_loads,
            } = l;
            let mut u = vec![0.0f64; n_nodes * NDF];
            for (kk, e) in map.eq.iter().enumerate() {
                if let Some(e) = e {
                    u[kk] = rhs_all[e * batch.len() + r];
                }
            }
            ties.place(&mut u);
            followers.place(&mut u);

            let case = recover(
                id,
                combination,
                u,
                nodal,
                fixed,
                elem_loads,
                shell_loads,
                applied_member,
                None,
            );
            clock.lap("cases: forces, reactions, checks");
            if *ahead {
                order.ahead(&case);
            } else {
                order.push(case, out)?;
            }
            clock.lap("cases: combinations, results handed on");
        }
    }

    assert!(order.done(), "every case and combination handed on");
    let (pier_sections, spandrel_sections) =
        piers.as_ref().map(|p| p.sections()).unwrap_or_default();
    Ok(Results {
        title: model.title.clone(),
        units: model.units.clone(),
        notes,
        instabilities,
        failed: Vec::new(),
        cases: Vec::new(),
        mesh: None,
        modal,
        diaphragm_centers,
        pier_sections,
        spandrel_sections,
    })
}

/// What a load case's loads need from the analysis set-up.
struct LoadContext<'a> {
    model: &'a Model,
    elems: &'a [Elem],
    shells: &'a [ShellEl],
    node_index: &'a HashMap<&'a Id, usize>,
    element_index: &'a HashMap<&'a Id, usize>,
    area_index: &'a HashMap<&'a Id, usize>,
    inactive: &'static [usize],
    ties: &'a Ties,
    map: &'a DofMap,
    auto_restrained: &'a [bool],
    groups: &'a [crate::diaphragm::Level],
    masses: &'a [[f64; NDF]],
    attached: &'a [bool],
    floor: crate::diaphragm::FloorAt<'a>,
}

/// A load case's loads (joint loads, member loads and their fixed-end forces, area loads) and its
/// right-hand side on the equations.
fn case_loads(
    cx: &LoadContext,
    case: &LoadCase,
    combination: Option<ComboType>,
    notes: &mut Vec<String>,
) -> Result<(Loaded, Vec<f64>), Error> {
    let LoadContext {
        model,
        elems,
        shells,
        node_index,
        element_index,
        area_index,
        inactive,
        ties,
        map,
        auto_restrained,
        groups,
        masses,
        attached,
        floor,
    } = *cx;
    let n_nodes = model.nodes.len();
    // Self-weight (the load case's multiplier) becomes ordinary loads.
    let case = &model.with_self_weight(case);
    // Direct nodal loads.
    let mut nodal = vec![0.0f64; n_nodes * NDF];
    // User wind loads, as joint loads (no joint is added).
    for w in &case.wind_loads {
        let loads = crate::diaphragm::wind_joint_loads(
            model, groups, masses, attached, floor, &case.id, w, notes,
        )?;
        for (n, f) in loads {
            for (d, v) in f.iter().enumerate() {
                nodal[n * NDF + d] += v;
            }
        }
    }
    for l in &case.node_loads {
        let n = node_index[&l.node];
        for (d, v) in l.as_array().iter().enumerate() {
            if inactive.contains(&d) && *v != 0.0 {
                return Err(Error::Invalid(format!(
                    "load case {}: node {} has an out-of-plane load ({}) in {}",
                    case.id,
                    l.node,
                    crate::model::FORCE_NAMES[d],
                    model.dimension.name()
                )));
            }
            nodal[n * NDF + d] += v;
        }
    }
    // Uniform area loads become corner forces q * int(N dA) (consistent loads); they have the same
    // resultant and moment as the load, so the equilibrium check stays exact.
    let mut shell_loads: Vec<Vec<f64>> = vec![Vec::new(); shells.len()];
    for l in &case.area_loads {
        let ai = area_index[&l.area];
        let sh = &shells[ai].shell;
        let q = match l.dir {
            LoadDir::X => [l.q, 0.0, 0.0],
            LoadDir::Y => [0.0, l.q, 0.0],
            LoadDir::Z => [0.0, 0.0, l.q],
            LoadDir::L1 => axes::scale(sh.axes[0], l.q),
            LoadDir::L2 => axes::scale(sh.axes[1], l.q),
            LoadDir::L3 => axes::scale(sh.axes[2], l.q),
        };
        if let Some(d) = inactive
            .iter()
            .find(|&&d| d < 3 && q[d].abs() > 1e-12 * l.q.abs())
        {
            return Err(Error::Invalid(format!(
                "load case {}: area load on {} has an out-of-plane ({}) part in {}",
                case.id,
                l.area,
                ["X", "Y", "Z"][*d],
                model.dimension.name()
            )));
        }
        let own = &mut shell_loads[ai];
        own.resize(NDF * shells[ai].nodes.len(), 0.0);
        for (k, f) in sh.area_load(q).iter().enumerate() {
            let n = shells[ai].nodes[k];
            for d in 0..3 {
                nodal[n * NDF + d] += f[d];
                own[k * NDF + d] += f[d];
            }
            // On a mid-plane away from the joints, the load also turns the joint.
            if let Some(e) = shells[ai].load_arm {
                let m = axes::cross(e, *f);
                for d in 0..3 {
                    nodal[n * NDF + 3 + d] += m[d];
                    own[k * NDF + 3 + d] += m[d];
                }
            }
        }
    }
    // Member loads resolved into local directions, and their fixed-end forces.
    let mut elem_loads: Vec<Vec<LocalLoad>> = vec![Vec::new(); elems.len()];
    let mut applied_member = [0.0f64; 6];
    for ml in &case.frame_loads {
        let ei = element_index[ml.frame()];
        let el = &elems[ei];
        let xi = el.start;
        let (g, local) = resolve(ml, &el.geo, &el.axes, el.props.l);
        if let Some(d) = inactive
            .iter()
            .find(|&&d| d < 3 && g[d].abs() > 1e-12 * axes::dot(g, g).sqrt())
        {
            return Err(Error::Invalid(format!(
                "load case {}: member load on element {} has an out-of-plane ({}) part in {}",
                case.id,
                ml.frame(),
                ["X", "Y", "Z"][*d],
                model.dimension.name()
            )));
        }
        elem_loads[ei].extend(local);
        // Resultant and moment about the origin, for the equilibrium check: with the load's
        // integrals i0 = sum of w and i1 = sum of s w along the member (s from end i),
        // F = g i0 and M = xi x g i0 + e1 x g i1.
        let l = el.props.l;
        let (i0, i1) = match *ml {
            FrameLoad::Uniform { w, from, to, .. } => {
                elements::distributed_part(w, w, from.unwrap_or(0.0), to.unwrap_or(l), 0.0, l)
            }
            FrameLoad::Trapezoidal {
                w1, w2, from, to, ..
            } => elements::distributed_part(w1, w2, from.unwrap_or(0.0), to.unwrap_or(l), 0.0, l),
            FrameLoad::Point { p, at, .. } => (p, p * at),
        };
        let f = axes::scale(g, i0);
        let m = axes::add(
            axes::cross(xi, f),
            axes::scale(axes::cross(el.axes[0], g), i1),
        );
        for d in 0..3 {
            applied_member[d] += f[d];
            applied_member[d + 3] += m[d];
        }
    }
    let fixed: Vec<[f64; 12]> = elems
        .iter()
        .zip(&elem_loads)
        .map(|(el, loads)| {
            if loads.is_empty() {
                [0.0; 12]
            } else if frame::hinged_inside(&el.props, &el.rel) {
                frame::hinged_fixed_end_forces(&el.props, &el.rel, loads)
            } else {
                frame::fixed_end_forces(&el.props, loads)
            }
        })
        .collect();

    let mut rhs = vec![0.0f64; map.n_eq];
    for n in 0..n_nodes {
        let f = &nodal[n * NDF..n * NDF + NDF];
        if f.iter().all(|v| *v == 0.0) {
            continue;
        }
        for (c, fc) in ties.carried(n, f) {
            for (d, v) in fc.iter().enumerate() {
                let kk = c * NDF + d;
                match map.eq[kk] {
                    Some(e) => rhs[e] += v,
                    None if auto_restrained[kk] && *v != 0.0 => {
                        return Err(Error::LoadOnUnstiffDof {
                            case: case.id.clone(),
                            node: model.nodes[c].id.clone(),
                            dof: DOF_NAMES[d],
                        })
                    }
                    None => {}
                }
            }
        }
    }
    for (el, fef) in elems.iter().zip(&fixed) {
        // Released end forces are zero: the element passes the condensed fixed-end forces on.
        let fef = if el.rel.contains(&true) && !frame::hinged_inside(&el.props, &el.rel) {
            frame::condense(&el.k_full, fef, &el.rel).1
        } else {
            *fef
        };
        let mut g = axes::vec12_to_global(&el.axes, &fef);
        if let Some(arms) = &el.arms {
            crate::arms::to_joints(&mut g, arms);
        }
        for (s, &n) in el.nodes.iter().enumerate() {
            for (c, fc) in ties.carried(n, &g[s * NDF..s * NDF + NDF]) {
                for (d, v) in fc.iter().enumerate() {
                    if let Some(e) = map.eq[c * NDF + d] {
                        rhs[e] -= v;
                    }
                }
            }
        }
    }
    Ok((
        Loaded {
            id: case.id.clone(),
            combination,
            nodal,
            elem_loads,
            applied_member,
            fixed,
            shell_loads,
        },
        rhs,
    ))
}

/// The P-Delta load's iterations (method `loads`): constant-stiffness iterations on `k0` (the stiffness
/// without P-Delta), each solving the unbalance F - K(P) u where every frame's stiffness has the
/// geometric stiffness of its mean axial force at the current u, until the relative unbalance is below
/// [`crate::pdelta::TOLERANCE`]: the unbalance's size over that of all the external joint forces (the
/// joint loads and the support reactions; see [`crate::pdelta::relative_unbalance`]). When they run out
/// or the unbalance grows ([`crate::pdelta::leave_constant_stiffness`]) the iterations start again from
/// no displacement as Newton steps: the first on `k0`, each later one on K(P) at the last iterate.
/// Returns each frame's mean axial force at the last iterate.
#[allow(clippy::too_many_arguments)]
fn p_delta_axial(
    elems: &[Elem],
    shells: &[ShellEl],
    blocks: &[Block],
    ties: &Ties,
    map: &DofMap,
    followers: &mut Followers,
    k0: &Factor,
    load: &Loaded,
    mend: bool,
    notes: &mut Vec<String>,
) -> Result<(Vec<f64>, Vec<f64>), Error> {
    use crate::pdelta::{leave_constant_stiffness, relative_unbalance, NEWTON, TOLERANCE};
    let n_dofs = load.nodal.len();
    // The P-Delta load at the carriers' DOFs: its joint loads alone, and less the members' fixed-end
    // forces (released members pass theirs on condensed), which the equations see.
    let mut joint = vec![0.0f64; n_dofs];
    let mut ext = vec![0.0f64; n_dofs];
    let carry = |v: &mut [f64], n: usize, f: &[f64], s: f64| {
        for (c, fc) in ties.carried(n, f) {
            for (d, x) in fc.iter().enumerate() {
                v[c * NDF + d] += s * x;
            }
        }
    };
    for n in 0..n_dofs / NDF {
        let f = &load.nodal[n * NDF..n * NDF + NDF];
        if f.iter().any(|x| *x != 0.0) {
            carry(&mut joint, n, f, 1.0);
            carry(&mut ext, n, f, 1.0);
        }
    }
    for (el, fef) in elems.iter().zip(&load.fixed) {
        if fef.iter().all(|x| *x == 0.0) {
            continue;
        }
        let fef = if el.rel.contains(&true) && !frame::hinged_inside(&el.props, &el.rel) {
            frame::condense(&el.k_full, fef, &el.rel).1
        } else {
            *fef
        };
        let mut g = axes::vec12_to_global(&el.axes, &fef);
        if let Some(arms) = &el.arms {
            crate::arms::to_joints(&mut g, arms);
        }
        for s in 0..2 {
            carry(&mut ext, el.nodes[s], &g[s * NDF..s * NDF + NDF], -1.0);
        }
    }
    let mut rhs = vec![0.0f64; map.n_eq];
    let mut joint_free = vec![0.0f64; map.n_eq];
    for (kk, e) in map.eq.iter().enumerate() {
        if let Some(e) = e {
            rhs[*e] = ext[kk];
            joint_free[*e] = joint[kk];
        }
    }
    let supported: Vec<usize> = (0..n_dofs)
        .filter(|&kk| ties.own[kk] && map.eq[kk].is_none())
        .collect();
    let mut u_eq = vec![0.0f64; map.n_eq];
    let mut r = rhs.clone();
    let mut axial = vec![0.0f64; elems.len()];
    let mut newton: Option<Factor> = None;
    // The relative unbalance of each constant-stiffness iteration; then the Newton steps taken (None
    // before them).
    let mut constant: Vec<f64> = Vec::new();
    let mut steps: Option<usize> = None;
    // The relative unbalance after each Newton step.
    let mut newton_unbalance: Vec<f64> = Vec::new();
    let mut it = 0;
    loop {
        it += 1;
        let mut du = r.clone();
        newton.as_ref().unwrap_or(k0).solve(&mut du);
        for (a, b) in u_eq.iter_mut().zip(&du) {
            *a += b;
        }
        let mut u = vec![0.0f64; n_dofs];
        for (kk, e) in map.eq.iter().enumerate() {
            if let Some(e) = e {
                u[kk] = u_eq[*e];
            }
        }
        ties.place(&mut u);
        followers.place(&mut u);
        for (i, el) in elems.iter().enumerate() {
            let (end, _, _) = el.end_forces(&u, &load.fixed[i], &load.elem_loads[i]);
            axial[i] = crate::pdelta::mean_axial(&end, &load.elem_loads[i], el.props.l);
        }
        // K(P) u at the carriers' DOFs: the frames with their geometric stiffness, the other blocks as
        // they are.
        let mut frames: Vec<Block> = elems
            .iter()
            .zip(&axial)
            .map(|(el, &p)| {
                let k = el.global_stiffness(&Props { pd: p, ..el.props });
                Block::carried(&el.nodes, k.iter().flatten().copied().collect(), ties)
            })
            .collect();
        frames.extend(shells.iter().map(|sh| {
            let k: Vec<f64> = sh
                .k_global
                .iter()
                .zip(sh.geometric(&u))
                .map(|(a, b)| a + b)
                .collect();
            Block::carried(&sh.nodes, k, ties)
        }));
        let rest = &blocks[elems.len() + shells.len()..];
        let mut ku = vec![0.0f64; n_dofs];
        for blk in frames.iter().chain(rest) {
            let size = blk.size();
            let ub: Vec<f64> = (0..size).map(|b| u[blk.dof(b)]).collect();
            for a in 0..size {
                ku[blk.dof(a)] += (0..size).map(|b| blk.k[a * size + b] * ub[b]).sum::<f64>();
            }
        }
        for (kk, e) in map.eq.iter().enumerate() {
            if let Some(e) = e {
                r[*e] = ext[kk] - ku[kk];
            }
        }
        // The support reactions' forces, their moments left out (measured, docs/SOURCES.md).
        let reactions: Vec<f64> = supported
            .iter()
            .filter(|&&kk| kk % NDF < 3)
            .map(|&kk| ku[kk] - ext[kk])
            .collect();
        let last = relative_unbalance(&r, &joint_free, &reactions);
        if last <= TOLERANCE {
            notes.push(format!(
                "P-Delta from loads: {it} iteration(s), relative unbalance {last:.2e}"
            ));
            return Ok((axial, u));
        }
        match &mut steps {
            None => {
                constant.push(last);
                if leave_constant_stiffness(&constant) {
                    steps = Some(0);
                    u_eq.fill(0.0);
                    r.copy_from_slice(&rhs);
                }
            }
            Some(taken) => {
                *taken += 1;
                newton_unbalance.push(last);
                // Out of steps, or the unbalance has stopped falling: no use factorizing again.
                if *taken >= NEWTON || crate::pdelta::newton_stalled(&newton_unbalance) {
                    return Err(p_delta_failed(it, last));
                }
                let mut s = SymMatrix::new(map.pattern.clone());
                assemble(&frames, map, &mut s);
                assemble(rest, map, &mut s);
                // The factor of the step before goes first: one in memory at a time.
                drop(newton.take());
                newton = Some(if mend {
                    s.factorize_indefinite_mended().0 .0
                } else {
                    s.factorize_indefinite()
                        .map_err(|_| p_delta_failed(it, last))?
                        .0
                });
            }
        }
    }
}

fn p_delta_failed(iterations: usize, unbalance: f64) -> Error {
    Error::Invalid(format!(
        "P-Delta from loads: no convergence after {iterations} iterations (relative unbalance \
         {unbalance:.2e}); the structure may buckle under the P-Delta load"
    ))
}

/// Adds the blocks' stiffness to a matrix numbered by `map`.
fn assemble(blocks: &[Block], map: &DofMap, k: &mut SymMatrix) {
    for blk in blocks {
        let size = blk.size();
        for a in 0..size {
            let Some(ea) = map.eq[blk.dof(a)] else {
                continue;
            };
            for b in 0..size {
                let Some(eb) = map.eq[blk.dof(b)] else {
                    continue;
                };
                // The matrix stores one triangle: (ea, eb) stands for (eb, ea) too, so each pair of
                // distinct equations is added once. Entries that land on the same equation
                // (members inside a rigid body) are all added.
                let v = blk.k[a * size + b];
                if ea <= eb && v != 0.0 {
                    k.add(ea, eb, v);
                }
            }
        }
    }
}

/// Centre of all the lumped mass (by the UX masses); the origin when there is none.
fn mass_centre(model: &Model, masses: &[[f64; NDF]]) -> [f64; 3] {
    let total: f64 = masses.iter().map(|m| m[0]).sum();
    if !(total > 0.0) {
        return [0.0; 3];
    }
    let s = masses.iter().enumerate().fold([0.0; 3], |c, (n, m)| {
        axes::add(c, axes::scale(coords(&model.nodes[n]), m[0]))
    });
    axes::scale(s, 1.0 / total)
}

/// A joint's motion under a unit rigid-body motion of the ground in global direction `d` (UX UY UZ, or a
/// rotation RX RY RZ about `about`): row `d` of the rigid-body matrix at `x`.
fn rigid_motion(x: [f64; 3], about: [f64; 3], d: usize) -> [f64; 6] {
    let [px, py, pz] = axes::sub(x, about);
    [
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        [0.0, -pz, py, 1.0, 0.0, 0.0],
        [pz, 0.0, -px, 0.0, 1.0, 0.0],
        [-py, px, 0.0, 0.0, 0.0, 1.0],
    ][d]
}

/// The load of a unit ground acceleration in global direction `d` (a rotation about `about`) on the
/// equations: M r, each joint's masses times its rigid-body motion, carried through the joint ties.
fn accel_load(
    model: &Model,
    masses: &[[f64; NDF]],
    ties: &Ties,
    map: &DofMap,
    d: usize,
    about: [f64; 3],
) -> Vec<f64> {
    let mut rhs = vec![0.0f64; map.n_eq];
    for (n, mn) in masses.iter().enumerate() {
        let r = rigid_motion(coords(&model.nodes[n]), about, d);
        let f: [f64; NDF] = std::array::from_fn(|c| mn[c] * r[c]);
        if f.iter().all(|v| *v == 0.0) {
            continue;
        }
        for (c, fc) in ties.carried(n, &f) {
            for (a, v) in fc.iter().enumerate() {
                if let Some(e) = map.eq[c * NDF + a] {
                    rhs[e] += v;
                }
            }
        }
    }
    rhs
}

/// Modal load participation ratios (%) of a load `p` on the equations: static, the share of its static
/// response's strain energy pᵀ K⁻¹ p the modes reach, Σ (φᵀ p)² / (φᵀ K φ) (finite modes φᵀ K φ = ω², static
/// ones 1); dynamic, over the part of p on massive coordinates p_m (its projection on M's range),
/// Σ (φᵀ p_m)² / (p_mᵀ M⁺ p_m) over the finite modes (unit modal mass). An acceleration's dynamic ratio is
/// its cumulative mass ratio (docs/phases/RITZ_PLAN.md, *Outputs and normalization*).
fn participation_of(
    k: &Factor,
    m: &MassBlocks,
    values: &[f64],
    vectors: &[Vec<f64>],
    first_static: usize,
    p: &[f64],
) -> (f64, f64) {
    let dot = |a: &[f64], b: &[f64]| -> f64 { a.iter().zip(b).map(|(x, y)| x * y).sum() };
    let mut x = p.to_vec();
    k.solve(&mut x);
    let energy = dot(p, &x);
    let reached: f64 = vectors
        .iter()
        .enumerate()
        .map(|(j, phi)| {
            let f = dot(phi, p);
            if j < first_static {
                f * f / values[j]
            } else {
                f * f
            }
        })
        .sum();
    let (pm, inertia) = m.project(p);
    let dynamic: f64 = vectors[..first_static]
        .iter()
        .map(|phi| dot(phi, &pm).powi(2))
        .sum();
    let pct = |a: f64, b: f64| if b > 0.0 { 100.0 * a / b } else { 0.0 };
    (pct(reached, energy), pct(dynamic, inertia))
}

/// M on the equations split into its independent blocks (equations coupled by mass entries), each with its
/// eigenvectors of nonzero mass: for projecting a load on M's range and for pᵀ M⁺ p.
struct MassBlocks {
    /// Per block: its equations, and (μ, eigenvector) pairs with μ > 0.
    blocks: Vec<(Vec<usize>, Vec<MassPair>)>,
}

/// One eigenvalue μ > 0 of a mass block and its eigenvector.
type MassPair = (f64, Vec<f64>);

impl MassBlocks {
    fn new(m: &eigen::SymSparse) -> MassBlocks {
        let mut parent: Vec<usize> = (0..m.n).collect();
        fn root(parent: &mut [usize], mut a: usize) -> usize {
            while parent[a] != a {
                parent[a] = parent[parent[a]];
                a = parent[a];
            }
            a
        }
        for &(a, b, v) in &m.entries {
            if v != 0.0 {
                let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
                if ra != rb {
                    parent[ra] = rb;
                }
            }
        }
        let massive: Vec<bool> = {
            let mut f = vec![false; m.n];
            for &(a, b, v) in &m.entries {
                if v != 0.0 {
                    f[a] = true;
                    f[b] = true;
                }
            }
            f
        };
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for e in (0..m.n).filter(|&e| massive[e]) {
            let r = root(&mut parent, e);
            groups.entry(r).or_default().push(e);
        }
        let mut entries: HashMap<usize, Vec<(usize, usize, f64)>> = HashMap::new();
        for &(a, b, v) in &m.entries {
            if v != 0.0 {
                entries
                    .entry(root(&mut parent, a))
                    .or_default()
                    .push((a, b, v));
            }
        }
        let mut keys: Vec<usize> = groups.keys().copied().collect();
        keys.sort_unstable();
        let blocks = keys
            .into_iter()
            .map(|r| {
                let eqs = groups.remove(&r).unwrap();
                let n = eqs.len();
                let at: HashMap<usize, usize> =
                    eqs.iter().enumerate().map(|(i, e)| (*e, i)).collect();
                let mut a = vec![0.0; n * n];
                for &(i, j, v) in &entries[&r] {
                    a[at[&i] * n + at[&j]] += v;
                }
                let (mu, v) = eigen::jacobi(&a, n);
                let top = mu.iter().fold(0.0f64, |t, x| t.max(x.abs()));
                let pairs = (0..n)
                    .filter(|&c| mu[c] > 1e-12 * top)
                    .map(|c| (mu[c], (0..n).map(|i| v[i * n + c]).collect()))
                    .collect();
                (eqs, pairs)
            })
            .collect();
        MassBlocks { blocks }
    }

    /// p's projection on M's range, and pᵀ M⁺ p.
    fn project(&self, p: &[f64]) -> (Vec<f64>, f64) {
        let mut out = vec![0.0; p.len()];
        let mut inertia = 0.0;
        for (eqs, pairs) in &self.blocks {
            for (mu, v) in pairs {
                let c: f64 = eqs.iter().zip(v).map(|(e, x)| p[*e] * x).sum();
                inertia += c * c / mu;
                for (e, x) in eqs.iter().zip(v) {
                    out[*e] += c * x;
                }
            }
        }
        (out, inertia)
    }
}

/// The modal case: lumped masses (`mass`), carried to the equations through the joint ties;
/// the lowest eigenpairs (`eigen`); periods, participation factors and participating mass ratios over
/// the free mass (rotations about the centre of all the mass), and the mode shapes at unit modal mass
/// (docs/phases/PHASE4_PLAN.md, probe findings 4).
#[allow(clippy::too_many_arguments)]
fn modal_case(
    model: &Model,
    mc: &ModalCase,
    starts: &[crate::ritz::Start],
    k: &Factor,
    stiffness: &SymMatrix,
    held: &[usize],
    map: &DofMap,
    ties: &Ties,
    restrained: &[bool],
    masses: &[[f64; NDF]],
    notes: &mut Vec<String>,
) -> Result<Option<ModalResults>, Error> {
    let mut clock = Clock::new();
    let n_nodes = model.nodes.len();
    // Held equations (`k` gives zero there): their mass takes no part, and the count of eigenvalues
    // is taken on the stiffness without them.
    let mut is_held = vec![false; map.n_eq];
    for &e in held {
        is_held[e] = true;
    }
    if !held.is_empty() {
        notes.push(format!(
            "Modal case {}: {} direction(s) without stiffness were held (see the warning about the \
             unstable structure); modes that move the joints next to them are not reliable",
            mc.id,
            held.len()
        ));
    }
    // M on the equations: each joint's diagonal mass carried to its carriers.
    let mut at: HashMap<(usize, usize), f64> = HashMap::new();
    for (n, mn) in masses.iter().enumerate() {
        if mn.iter().all(|v| *v == 0.0) {
            continue;
        }
        let mut d = vec![0.0; NDF * NDF];
        for a in 0..NDF {
            d[a * NDF + a] = mn[a];
        }
        let (carriers, mm) = ties.carry(&[n], d);
        let size = NDF * carriers.len();
        for a in 0..size {
            let Some(ea) = map.eq[carriers[a / NDF] * NDF + a % NDF] else {
                continue;
            };
            for b in 0..size {
                let Some(eb) = map.eq[carriers[b / NDF] * NDF + b % NDF] else {
                    continue;
                };
                let v = mm[a * size + b];
                if v != 0.0 {
                    *at.entry((ea, eb)).or_insert(0.0) += v;
                }
            }
        }
    }
    let mut entries: Vec<(usize, usize, f64)> = at
        .into_iter()
        .filter(|((a, b), _)| !is_held[*a] && !is_held[*b])
        .map(|((a, b), v)| (a, b, v))
        .collect();
    entries.sort_by_key(|x| (x.0, x.1));
    let m = SymSparse {
        n: map.n_eq,
        entries,
    };
    // No mass where the structure can move (e.g. a model without weights): the static cases still run.
    if !m.diagonal().iter().any(|v| *v > 0.0) {
        notes.push(format!(
            "Modal case {}: not run, the model has no mass where it can move (give materials a weight or joints a mass)",
            mc.id
        ));
        return Ok(None);
    }
    let fail = |msg: String| Error::Invalid(format!("modal case {}: {msg}", mc.id));
    if m.entries.iter().any(|&(a, b, _)| !stiffness.contains(a, b)) {
        return Err(fail(
            "a mass couples equations the stiffness does not".into(),
        ));
    }
    clock.lap("modal: mass matrix");
    // K - σM on the stiffness's pattern (same order and factor structure).
    let mut sturm = |sigma: f64| -> Result<usize, String> {
        clock.lap("modal: subspace iteration");
        let mut s = stiffness.clone();
        for &(a, b, v) in &m.entries {
            if a <= b {
                s.add(a, b, -sigma * v);
            }
        }
        if !held.is_empty() {
            s.isolate(&is_held);
        }
        clock.lap("modal: Sturm check, assembly");
        let r = s
            .negative_pivots()
            .map_err(|eq| format!("the Sturm check hit an eigenvalue at equation {eq}"));
        clock.lap("modal: Sturm check, factorization");
        r
    };
    // Eigen vectors, or Ritz vectors (finite modes, then static ones: `statics` of the last).
    let (e, statics) = if mc.kind == ModalType::Ritz {
        let r = crate::ritz::modes(k, &m, starts, mc.modes);
        clock.lap("modal: Ritz vectors");
        let statics = r.statics.len();
        let (mut values, mut vectors): (Vec<f64>, Vec<Vec<f64>>) = r.finite.into_iter().unzip();
        values.extend(std::iter::repeat_n(0.0, statics));
        vectors.extend(r.statics);
        (
            eigen::Eigen {
                values,
                vectors,
                checked: true,
                accuracy: None,
            },
            statics,
        )
    } else {
        let e =
            eigen::subspace(k, k.original_diagonal(), &m, mc.modes, &mut sturm).map_err(fail)?;
        clock.lap("modal: subspace iteration");
        (e, 0)
    };
    let first_static = e.values.len() - statics;
    if let Some(level) = e.accuracy {
        notes.push(format!(
            "Warning: modal case {}: its eigenvalues could not be made more exact than about {level:.0e} \
             (their relative change from one iteration to the next); the stiffness has lost too many \
             digits for more (see the note about the ill-conditioned stiffness). Periods are good to \
             about that",
            mc.id
        ));
    }
    if !e.checked {
        notes.push(format!(
            "Warning: modal case {}: the check that no mode was missed could not be completed",
            mc.id
        ));
    }
    if mc.kind == ModalType::Ritz {
        if e.values.len() < mc.modes {
            notes.push(format!(
                "Modal case {}: {} Ritz vectors ({} asked for; the starting loads give no more)",
                mc.id,
                e.values.len(),
                mc.modes
            ));
        }
        if statics > 0 {
            notes.push(format!(
                "Modal case {}: {statics} static modes (starting loads on joints without mass)",
                mc.id
            ));
        }
    } else if e.values.len() < mc.modes {
        notes.push(format!(
            "Modal case {}: {} modes (the model has only {} independent mass DOFs; {} asked for)",
            mc.id,
            e.values.len(),
            e.values.len(),
            mc.modes
        ));
    }

    // Rigid-body motions about the centre of all the mass, and the free mass in each direction.
    let x = |n: usize| coords(&model.nodes[n]);
    let centre = mass_centre(model, masses);
    let rb = |n: usize| -> [[f64; 6]; 6] { std::array::from_fn(|d| rigid_motion(x(n), centre, d)) };
    let mut free_mass = [0.0; 6];
    for (n, mn) in masses.iter().enumerate() {
        let r = rb(n);
        for d in 0..6 {
            for c in 0..6 {
                if !restrained[n * NDF + c] {
                    free_mass[d] += r[d][c] * r[d][c] * mn[c];
                }
            }
        }
    }
    // A rotation with only round-off free mass (all the mass at one height, none rotational) has no
    // ratio: its free mass counts as 0.
    let reach = masses
        .iter()
        .enumerate()
        .filter(|(_, m)| m.iter().any(|v| *v != 0.0))
        .map(|(n, _)| {
            axes::sub(x(n), centre)
                .iter()
                .fold(0.0f64, |a, v| a.max(v.abs()))
        })
        .fold(0.0f64, f64::max);
    let translational = free_mass[..3].iter().fold(0.0f64, |a, v| a.max(*v));
    for f in &mut free_mass[3..] {
        if *f <= 1e-12 * translational * reach * reach {
            *f = 0.0;
        }
    }
    let mut sums = [0.0; 6];
    let mut modes = Vec::with_capacity(e.values.len());
    for (j, (lambda, phi)) in e.values.iter().zip(&e.vectors).enumerate() {
        let mut u = vec![0.0f64; n_nodes * NDF];
        for (kk, eq) in map.eq.iter().enumerate() {
            if let Some(eq) = eq {
                u[kk] = phi[*eq];
            }
        }
        ties.place(&mut u);
        // Sign: the largest translation positive.
        let big = (0..n_nodes * NDF)
            .filter(|kk| kk % NDF < 3)
            .max_by(|&a, &b| u[a].abs().total_cmp(&u[b].abs()))
            .unwrap_or(0);
        if u[big] < 0.0 {
            u.iter_mut().for_each(|v| *v = -*v);
        }
        let mut participation = [0.0; 6];
        for (n, mn) in masses.iter().enumerate() {
            let r = rb(n);
            for d in 0..6 {
                for c in 0..6 {
                    participation[d] += r[d][c] * mn[c] * u[n * NDF + c];
                }
            }
        }
        let mass_ratio: [f64; 6] = std::array::from_fn(|d| {
            if free_mass[d] > 0.0 {
                participation[d] * participation[d] / free_mass[d]
            } else {
                0.0
            }
        });
        for d in 0..6 {
            sums[d] += mass_ratio[d];
        }
        // A negative eigenvalue (a structure that buckles under its P-Delta load, results given anyway):
        // its period and frequencies are reported negative. A static mode has none (0).
        let static_mode = j >= first_static;
        let omega = lambda.abs().sqrt().copysign(*lambda);
        let period = if static_mode {
            0.0
        } else {
            2.0 * std::f64::consts::PI / omega
        };
        modes.push(Mode {
            mode: j + 1,
            static_mode,
            period,
            frequency: omega / (2.0 * std::f64::consts::PI),
            circular_frequency: omega,
            eigenvalue: *lambda,
            participation,
            mass_ratio,
            sum_mass_ratio: sums,
            shape: (0..n_nodes)
                .map(|n| {
                    let mut v = [0.0; 6];
                    v.copy_from_slice(&u[n * NDF..n * NDF + NDF]);
                    NodeValues {
                        node: model.nodes[n].id.clone(),
                        values: v.into(),
                    }
                })
                .collect(),
        });
    }
    clock.lap("modal: periods, mass ratios, shapes");
    // Load participation: the three accelerations, then each load pattern a Ritz case starts from.
    let blocks = MassBlocks::new(&m);
    let mut load_participation = Vec::new();
    let mut ratios = |kind: &str, name: String, p: &[f64]| {
        let (stat, dynamic) = participation_of(k, &blocks, &e.values, &e.vectors, first_static, p);
        load_participation.push(crate::results::LoadParticipation {
            kind: kind.into(),
            name,
            static_percent: stat,
            dynamic_percent: dynamic,
        });
    };
    for (d, name) in ["UX", "UY", "UZ"].iter().enumerate() {
        ratios(
            "acceleration",
            name.to_string(),
            &accel_load(model, masses, ties, map, d, centre),
        );
    }
    let mut seen: Vec<&Id> = Vec::new();
    for (st, start) in mc.starts.iter().zip(starts) {
        if let Some(pat) = &st.pattern {
            if !seen.contains(&pat) {
                seen.push(pat);
                ratios("pattern", pat.to_string(), &start.load);
            }
        }
    }
    clock.lap("modal: load participation");
    Ok(Some(ModalResults {
        id: mc.id.clone(),
        modes,
        load_participation,
        masses: masses
            .iter()
            .enumerate()
            .filter(|(_, m)| m.iter().any(|v| *v != 0.0))
            .map(|(n, m)| crate::model::JointMass {
                node: model.nodes[n].id.clone(),
                ux: m[0],
                uy: m[1],
                uz: m[2],
                rx: m[3],
                ry: m[4],
                rz: m[5],
            })
            .collect(),
        free_mass,
        centre_of_mass: centre,
    }))
}

/// Forces in the rigid links, as member results (P, V2, V3, T, M2, M3 at stations; no loads along a
/// link). Links form trees, so the force in a link is what the part of the body beyond it needs:
/// `need` (member and spring forces minus applied loads) summed over that part and moved to the link.
fn link_results(model: &Model, bodies: &Bodies, need: &[f64], u: &[f64]) -> Vec<ElementResults> {
    let x = |n: usize| coords(&model.nodes[n]);
    let mut acc: Vec<[f64; 6]> = need
        .chunks(NDF)
        .map(|c| c.try_into().expect("6 values"))
        .collect();
    let mut out: Vec<Option<ElementResults>> = vec![None; model.links.len()];
    // Children before parents, so `acc[n]` holds the whole part beyond node n.
    for &(n, li, p) in bodies.tree.iter().rev() {
        let f = [acc[n][0], acc[n][1], acc[n][2]];
        let m = [acc[n][3], acc[n][4], acc[n][5]];
        // The link acts on node n with (f, m). On the link: (-f, -m) at n, and at p the force f with
        // the moment that balances the link; the same pair carries the part over to p.
        let mp = axes::add(m, axes::cross(axes::sub(x(n), x(p)), f));
        for d in 0..3 {
            acc[p][d] += f[d];
            acc[p][d + 3] += mp[d];
        }
        let at_n = [-f[0], -f[1], -f[2], -m[0], -m[1], -m[2]];
        let at_p = [f[0], f[1], f[2], mp[0], mp[1], mp[2]];
        let l = &model.links[li];
        let child_is_i = model.nodes[n].id == l.nodes[0];
        let ((i, gi), (j, gj)) = if child_is_i {
            ((n, at_n), (p, at_p))
        } else {
            ((p, at_p), (n, at_n))
        };
        let mut g = [0.0; 12];
        g[..6].copy_from_slice(&gi);
        g[6..].copy_from_slice(&gj);
        let ax = axes::local_axes(x(i), x(j), 0.0);
        let length = length(&model.nodes[i], &model.nodes[j]);
        let mut er = ElementResults {
            element: l.id.clone(),
            nodes: l.nodes.clone(),
            length,
            offsets: [0.0; 2],
            axes: ax,
            stations: Vec::new(),
            end_forces: axes::vec12_to_local(&ax, &g),
            loads: Vec::new(),
            end_displacements: [0.0; 12],
            hinges: frame::Hinges::default(),
            enveloped: false,
            pieces: Vec::new(),
            pdelta: None,
        };
        er.make_stations(STATIONS);
        // A rigid link moves without deforming: translations vary linearly along it.
        for s in &mut er.stations {
            let t = s.x / length;
            let lerp = |d: usize| u[i * NDF + d] + t * (u[j * NDF + d] - u[i * NDF + d]);
            [s.ux, s.uy, s.uz] = [lerp(0), lerp(1), lerp(2)];
        }
        out[li] = Some(er);
    }
    out.into_iter()
        .map(|e| e.expect("every link is in a rigid-body tree"))
        .collect()
}

/// Adds a nodal force/moment at point `p` to a resultant about the origin.
fn add_about_origin(out: &mut [f64; 6], p: [f64; 3], v: &[f64]) {
    let f = [v[0], v[1], v[2]];
    let m = axes::cross(p, f);
    for d in 0..3 {
        out[d] += f[d];
        out[d + 3] += v[3 + d] + m[d];
    }
}

/// Joint DOFs that only Membrane elements reach out of their plane (a floor corner with nothing under
/// it, an opening's corner): Membrane elements have only a vanishing plate stiffness, so such a joint
/// follows the floor with no load on it; OSAB holds these DOFs (automatic restraints) and then places
/// them where that plate stiffness puts them: P_ff u_f = -P_fs u_s, P assembled from the Membrane
/// elements' plate stiffness.
struct Followers {
    /// Free (placed) DOFs and the other DOFs the Membrane elements reach.
    f: Vec<usize>,
    /// P restricted to f x f (row-major), and to f x others as (row, dof, value).
    pff: Vec<f64>,
    pfs: Vec<(usize, usize, f64)>,
    /// Cholesky factor of P_ff (with a tiny diagonal shift for floating parts), made on first use.
    factor: Option<Vec<f64>>,
}

impl Followers {
    fn new(shells: &[ShellEl], auto_restrained: &[bool]) -> Followers {
        let mut p: HashMap<(usize, usize), f64> = HashMap::new();
        for sh in shells {
            let Some(k) = sh.shell.membrane_plate_global() else {
                continue;
            };
            let size = 6 * sh.nodes.len();
            for a in 0..size {
                for b in 0..size {
                    let v = k[a * size + b];
                    if v != 0.0 {
                        let (da, db) =
                            (sh.nodes[a / 6] * NDF + a % 6, sh.nodes[b / 6] * NDF + b % 6);
                        *p.entry((da, db)).or_insert(0.0) += v;
                    }
                }
            }
        }
        let mut f: Vec<usize> = p
            .keys()
            .map(|k| k.0)
            .filter(|&d| auto_restrained[d])
            .collect();
        f.sort_unstable();
        f.dedup();
        let at: HashMap<usize, usize> = f.iter().enumerate().map(|(i, &d)| (d, i)).collect();
        let n = f.len();
        let mut pff = vec![0.0; n * n];
        let mut pfs = Vec::new();
        for (&(a, b), &v) in &p {
            if let Some(&i) = at.get(&a) {
                match at.get(&b) {
                    Some(&j) => pff[i * n + j] += v,
                    None => pfs.push((i, b, v)),
                }
            }
        }
        pfs.sort_by_key(|x| (x.0, x.1));
        Followers {
            f,
            pff,
            pfs,
            factor: None,
        }
    }

    fn place(&mut self, u: &mut [f64]) {
        let n = self.f.len();
        let mut rhs = vec![0.0; n];
        for &(i, d, v) in &self.pfs {
            rhs[i] -= v * u[d];
        }
        if rhs.iter().all(|v| *v == 0.0) {
            return;
        }
        let l = self.factor.get_or_insert_with(|| {
            let big = (0..n).map(|i| self.pff[i * n + i]).fold(0.0, f64::max);
            let mut a = self.pff.clone();
            for i in 0..n {
                a[i * n + i] += 1e-12 * big;
            }
            // Cholesky, lower triangle in place.
            for j in 0..n {
                let mut d = a[j * n + j];
                for k in 0..j {
                    d -= a[j * n + k] * a[j * n + k];
                }
                let d = d.max(1e-300).sqrt();
                a[j * n + j] = d;
                for i in j + 1..n {
                    let mut s = a[i * n + j];
                    for k in 0..j {
                        s -= a[i * n + k] * a[j * n + k];
                    }
                    a[i * n + j] = s / d;
                }
            }
            a
        });
        for i in 0..n {
            let mut s = rhs[i];
            for k in 0..i {
                s -= l[i * n + k] * rhs[k];
            }
            rhs[i] = s / l[i * n + i];
        }
        for i in (0..n).rev() {
            let mut s = rhs[i];
            for k in i + 1..n {
                s -= l[k * n + i] * rhs[k];
            }
            rhs[i] = s / l[i * n + i];
        }
        for (i, &d) in self.f.iter().enumerate() {
            u[d] = rhs[i];
        }
    }
}

/// Bending of a section with a product of inertia I23: its principal axes are local 2-3 turned by
/// `turn` about local 1 (tan 2 turn = 2 I23 / (I33 - I22), within 45 degrees), with the principal
/// moments of inertia. The shear areas turn with them as flexibilities (1/As), leaving out the coupling
/// term: shear deformation is small next to bending in such members.
struct Principal {
    i33: f64,
    i22: f64,
    as2: f64,
    as3: f64,
    turn: f64,
}

impl Principal {
    fn of(i33: f64, i22: f64, i23: f64, as2: f64, as3: f64) -> Principal {
        if i23 == 0.0 {
            return Principal {
                i33,
                i22,
                as2,
                as3,
                turn: 0.0,
            };
        }
        let turn = if i33 == i22 {
            std::f64::consts::FRAC_PI_4.copysign(i23)
        } else {
            0.5 * (2.0 * i23 / (i33 - i22)).atan()
        };
        let (s, c) = turn.sin_cos();
        let flex = |a: f64| if a > 0.0 { 1.0 / a } else { 0.0 };
        let area = |f: f64| if f > 0.0 { 1.0 / f } else { 0.0 };
        let (f2, f3) = (flex(as2), flex(as3));
        Principal {
            i33: i33 * c * c + i22 * s * s + 2.0 * i23 * s * c,
            i22: i33 * s * s + i22 * c * c - 2.0 * i23 * s * c,
            as2: area(c * c * f2 + s * s * f3),
            as3: area(s * s * f2 + c * c * f3),
            turn,
        }
    }
}

/// Local axes 2 and 3 turned by `turn` (radians) about local 1.
fn turned(ax: &Axes, turn: f64) -> Axes {
    if turn == 0.0 {
        return *ax;
    }
    let (s, c) = turn.sin_cos();
    let mix =
        |a: [f64; 3], b: [f64; 3], p: f64, q: f64| std::array::from_fn(|k| p * a[k] + q * b[k]);
    [ax[0], mix(ax[1], ax[2], c, s), mix(ax[1], ax[2], -s, c)]
}

/// A member analysed in its principal axes (turned by `turn` from the frame's local axes `geo`): its
/// end forces, displacements, loads and station forces in the frame's local axes.
fn to_frame_axes(er: &mut ElementResults, geo: &Axes, turn: f64) {
    let (s, c) = turn.sin_cos();
    if let Some(p) = &mut er.pdelta {
        p.turn = turn;
    }
    let back = |a: f64, b: f64| (c * a - s * b, s * a + c * b);
    for v in [&mut er.end_forces, &mut er.end_displacements] {
        for k in [1, 4, 7, 10] {
            (v[k], v[k + 1]) = back(v[k], v[k + 1]);
        }
    }
    for st in &mut er.stations {
        (st.V2, st.V3) = back(st.V2, st.V3);
        // M2 is the moment vector's local-2 part with the sign changed (it compresses the +3 face).
        (st.M2, st.M3) = (c * st.M2 + s * st.M3, -s * st.M2 + c * st.M3);
    }
    // A load along principal 2 (dir 1) or 3 (dir 2) has parts along local 2 and 3.
    let parts = |dir: usize| -> Vec<(usize, f64)> {
        match dir {
            1 => vec![(1, c), (2, s)],
            2 => vec![(1, -s), (2, c)],
            d => vec![(d, 1.0)],
        }
    };
    er.loads = er
        .loads
        .iter()
        .flat_map(|ld| {
            let dir = match ld {
                LocalLoad::Point { dir, .. } | LocalLoad::Distributed { dir, .. } => *dir,
            };
            parts(dir)
                .into_iter()
                .filter(|(_, f)| f.abs() > 1e-15)
                .map(|(d, f)| match *ld {
                    LocalLoad::Point { p, at, .. } => LocalLoad::Point {
                        dir: d,
                        p: f * p,
                        at,
                    },
                    LocalLoad::Distributed {
                        w1, w2, from, to, ..
                    } => LocalLoad::Distributed {
                        dir: d,
                        w1: f * w1,
                        w2: f * w2,
                        from,
                        to,
                    },
                })
                .collect::<Vec<_>>()
        })
        .collect();
    er.axes = *geo;
}

/// Splits a member load into local-direction loads of the axes the element is analysed in (`axes`;
/// a load along local 2 or 3 follows the frame's own axes `geo`). Also returns the load's direction as a
/// global unit vector (the load is that vector times its value).
fn resolve(ml: &FrameLoad, geo: &Axes, axes: &Axes, l: f64) -> ([f64; 3], Vec<LocalLoad>) {
    let dir = match ml {
        FrameLoad::Uniform { dir, .. }
        | FrameLoad::Trapezoidal { dir, .. }
        | FrameLoad::Point { dir, .. } => *dir,
    };
    let global = match dir {
        LoadDir::X => [1.0, 0.0, 0.0],
        LoadDir::Y => [0.0, 1.0, 0.0],
        LoadDir::Z => [0.0, 0.0, 1.0],
        LoadDir::L1 => geo[0],
        LoadDir::L2 => geo[1],
        LoadDir::L3 => geo[2],
    };
    let local = axes::to_local(axes, global);
    let mut out = Vec::new();
    for (d, &v) in local.iter().enumerate() {
        if v.abs() <= 1e-14 {
            continue;
        }
        out.push(match ml {
            FrameLoad::Uniform { w, from, to, .. } => {
                LocalLoad::uniform(d, v * w, from.unwrap_or(0.0), to.unwrap_or(l))
            }
            FrameLoad::Trapezoidal {
                w1, w2, from, to, ..
            } => LocalLoad::Distributed {
                dir: d,
                w1: v * w1,
                w2: v * w2,
                from: from.unwrap_or(0.0),
                to: to.unwrap_or(l),
            },
            FrameLoad::Point { p, at, .. } => LocalLoad::Point {
                dir: d,
                p: v * p,
                at: *at,
            },
        });
    }
    (global, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cantilever drawn in -X (reversed by the end-order rule) with a rotated section and local and
    /// partial member loads must behave exactly like the same beam drawn in +X and turned 180 degrees about Z.
    #[test]
    fn reversed_member_keeps_its_meaning() {
        let build = |reverse: bool| {
            // The -X beam is the +X beam turned 180 degrees about global Z (same local axes and loads).
            let s = if reverse { -1.0 } else { 1.0 };
            let json = format!(
                r#"{{
                "materials": [{{"id": "m", "E": 200e6, "nu": 0.3}}],
                "sections": [{{"id": "s", "A": 0.01, "I22": 5e-5, "I33": 2e-4, "J": 1e-5}}],
                "nodes": [{{"id": "1", "x": 0}}, {{"id": "2", "x": {x}}}],
                "frames": [{{"id": "1", "type": "frame", "nodes": ["1", "2"], "material": "m",
                               "section": "s", "angle": 25}}],
                "supports": [{{"node": "1", "fix": "fixed"}}],
                "load_cases": [{{"id": "LC", "frame_loads": [
                    {{"type": "uniform", "frame": "1", "dir": "2", "w": -3, "from": 0.5, "to": 2.5}},
                    {{"type": "point", "frame": "1", "dir": "3", "p": 2, "at": 3}},
                    {{"type": "point", "frame": "1", "dir": "1", "p": 5, "at": 1}}]}}]
            }}"#,
                x = 4.0 * s
            );
            analyze(&Model::from_json(&json).unwrap()).unwrap()
        };
        let plus = build(false);
        let minus = build(true);
        assert!(plus.notes.is_empty());
        assert!(minus.notes[0].contains("Reversed"));
        let dp = plus.cases[0].displacement(&"2".into()).unwrap().as_array();
        let dm = minus.cases[0].displacement(&"2".into()).unwrap().as_array();
        // A 180-degree turn about Z maps (ux, uy, uz, rx, ry, rz) to (-ux, -uy, uz, -rx, -ry, rz).
        let mirror = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0];
        for d in 0..6 {
            assert!(
                (dp[d] - mirror[d] * dm[d]).abs() < 1e-12,
                "dof {d}: {} vs {}",
                dp[d],
                dm[d]
            );
        }
    }

    const MAT_SEC: &str = r#""materials": [{"id": "m", "E": 200e6, "nu": 0.3}],
        "sections": [{"id": "s", "A": 0.01, "I22": 5e-5, "I33": 2e-4, "J": 1e-5}]"#;

    fn run(body: &str) -> Result<Results, Error> {
        analyze(&Model::from_json(&format!("{{{MAT_SEC}, {body}}}"))?)
    }

    fn invalid(body: &str) -> String {
        match run(body) {
            Err(Error::Invalid(msg)) => msg,
            other => panic!("expected an invalid-model error, got {other:?}"),
        }
    }

    const BEAM: &str = r#""nodes": [{"id": "1", "x": 0}, {"id": "2", "x": 4}],
        "frames": [{"id": "1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"}]"#;

    #[test]
    fn mechanism_is_reported_with_node_and_dof() {
        // 2D beam on two rollers: nothing stops it sliding along X.
        let r = run(&format!(
            r#""dimension": "2D", "on_instability": "stop", {BEAM}, "supports": [{{"node": "1", "fix": ["uz"]}}, {{"node": "2", "fix": ["uz"]}}],
            "load_cases": [{{"id": "LC", "node_loads": [{{"node": "2", "fz": -1}}]}}]"#
        ));
        match r {
            Err(Error::Unstable { dof, .. }) => assert_eq!(dof, "ux"),
            other => panic!("expected Unstable, got {other:?}"),
        }
    }

    #[test]
    fn moment_on_truss_only_node_is_rejected() {
        let r = run(r#""nodes": [{"id": "1", "x": 0}, {"id": "2", "x": 4}],
            "frames": [{"id": "1", "type": "truss", "nodes": ["1", "2"], "material": "m", "section": "s"}],
            "supports": [{"node": "1", "fix": "pinned"}, {"node": "2", "fix": ["uy", "uz"]}],
            "load_cases": [{"id": "LC", "node_loads": [{"node": "2", "fx": 1, "my": 2}]}]"#);
        match r {
            Err(Error::LoadOnUnstiffDof { dof, .. }) => assert_eq!(dof, "ry"),
            other => panic!("expected LoadOnUnstiffDof, got {other:?}"),
        }
    }

    #[test]
    fn plane_models_reject_out_of_plane_geometry_and_loads() {
        let fixed = r#""supports": [{"node": "1", "fix": "fixed"}]"#;
        let msg = invalid(&format!(
            r#""dimension": "2D", "nodes": [{{"id": "1"}}, {{"id": "2", "x": 4, "y": 3}}],
            "frames": [{{"id": "1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"}}],
            {fixed}, "load_cases": []"#
        ));
        assert!(msg.contains("X-Z plane"), "{msg}");
        let msg = invalid(&format!(
            r#""dimension": "2D", {BEAM}, {fixed},
            "load_cases": [{{"id": "LC", "node_loads": [{{"node": "2", "fy": 5}}]}}]"#
        ));
        assert!(msg.contains("out-of-plane"), "{msg}");
        let msg = invalid(&format!(
            r#""dimension": "2D", {BEAM}, {fixed},
            "load_cases": [{{"id": "LC", "frame_loads": [{{"type": "uniform", "frame": "1", "dir": "3", "w": 1}}]}}]"#
        ));
        assert!(msg.contains("out-of-plane"), "{msg}");
    }

    #[test]
    fn a_plane_grid_keeps_only_uz_rx_ry_and_matches_the_3d_run() {
        // Two beams in an L at z = 3, fixed at the far ends: a vertical load and moments at the corner
        // bend and twist them. With only vertical loads the 3D run is the same.
        let body = |dim: &str| {
            format!(
                r#""dimension": "{dim}", "nodes": [{{"id": "1", "z": 3}}, {{"id": "2", "x": 4, "z": 3}}, {{"id": "3", "x": 4, "y": 3, "z": 3}}],
                "frames": [{{"id": "1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"}},
                           {{"id": "2", "type": "frame", "nodes": ["2", "3"], "material": "m", "section": "s"}}],
                "supports": [{{"node": "1", "fix": "fixed"}}, {{"node": "3", "fix": "fixed"}}],
                "load_cases": [{{"id": "LC", "node_loads": [{{"node": "2", "fz": -10, "mx": 3, "my": -2}}],
                                 "frame_loads": [{{"type": "uniform", "frame": "1", "dir": "Z", "w": -2}}]}}]"#
            )
        };
        let (g, d3) = (run(&body("grid")).unwrap(), run(&body("3D")).unwrap());
        let ug = g.cases[0].displacement(&"2".into()).unwrap().as_array();
        let u3 = d3.cases[0].displacement(&"2".into()).unwrap().as_array();
        assert!(ug[2] < 0.0);
        for k in 0..6 {
            assert!(
                (ug[k] - u3[k]).abs() <= 1e-12 * ug[2].abs(),
                "dof {k}: {} vs {}",
                ug[k],
                u3[k]
            );
        }
        assert_eq!([ug[0], ug[1], ug[5]], [0.0, 0.0, 0.0]);
        // Loads and geometry out of the grid's plane are refused.
        let msg = invalid(&body("grid").replace(r#""fz": -10"#, r#""fx": 1"#));
        assert!(
            msg.contains("out-of-plane") && msg.contains("plane grid"),
            "{msg}"
        );
        let msg = invalid(&body("grid").replace(r#""dir": "Z""#, r#""dir": "X""#));
        assert!(msg.contains("out-of-plane (X)"), "{msg}");
        let msg = invalid(&body("grid").replace(r#""y": 3, "z": 3"#, r#""y": 3, "z": 4"#));
        assert!(msg.contains("one X-Y plane"), "{msg}");
    }

    #[test]
    fn bad_input_is_rejected_with_a_message() {
        let cases = [
            (
                r#""nodes": [{"id": "1"}], "frames": [{"id": "1", "type": "frame", "nodes": ["1", "9"], "material": "m", "section": "s"}]"#,
                "unknown node",
            ),
            (
                r#""nodes": [{"id": "1"}, {"id": "2"}], "frames": [{"id": "1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"}]"#,
                "zero length",
            ),
            (
                r#""nodes": [{"id": "1"}, {"id": "1", "x": 1}], "frames": []"#,
                "duplicate node",
            ),
            (
                r#""nodes": [{"id": "1"}, {"id": "2", "x": 1}], "frames": [{"id": "1", "type": "truss", "nodes": ["1", "2"], "material": "m", "section": "s"}],
                "load_cases": [{"id": "LC", "frame_loads": [{"type": "point", "frame": "1", "dir": "Z", "p": 1, "at": 0.5}]}]"#,
                "truss",
            ),
            (
                &format!(
                    r#"{BEAM}, "load_cases": [{{"id": "LC", "frame_loads": [{{"type": "point", "frame": "1", "dir": "Z", "p": 1, "at": 5}}]}}]"#
                ),
                "within",
            ),
        ];
        for (body, expect) in cases {
            let msg = invalid(body);
            assert!(msg.contains(expect), "expected '{expect}' in: {msg}");
        }
        // Missing G and nu.
        let r = Model::from_json(
            r#"{"materials": [{"id": "m", "E": 1}], "sections": [], "nodes": [], "frames": []}"#,
        );
        assert!(matches!(r, Err(Error::Invalid(m)) if m.contains("G or nu")));
    }

    #[test]
    fn point_load_inside_member_gives_stations_on_both_sides() {
        // Simply supported, P = 10 down at midspan: V2 jumps from -5 to +5 at the load (V2 is the
        // force on the positive face along +2, so the upward left reaction gives V2 = -5), and the
        // moment peak P L / 4 = 10 sits at the load.
        let r = run(&format!(
            r#""dimension": "2D", {BEAM}, "supports": [{{"node": "1", "fix": ["ux", "uz"]}}, {{"node": "2", "fix": ["uz"]}}],
            "load_cases": [{{"id": "LC", "frame_loads": [{{"type": "point", "frame": "1", "dir": "Z", "p": -10, "at": 2}}]}}]"#
        ))
        .unwrap();
        let st = &r.cases[0].frames[0].stations;
        let at: Vec<_> = st.iter().filter(|s| (s.x - 2.0).abs() < 1e-12).collect();
        assert_eq!(at.len(), 2);
        assert!((at[0].V2 + 5.0).abs() < 1e-9 && (at[1].V2 - 5.0).abs() < 1e-9);
        assert!((at[0].M3 - 10.0).abs() < 1e-9 && (at[1].M3 - 10.0).abs() < 1e-9);
        assert!(st.windows(2).all(|w| w[0].x <= w[1].x));
    }

    #[test]
    fn point_load_at_end_i_is_inside_the_member() {
        // A load right at the support goes straight into it: no shear in the span.
        let r = run(&format!(
            r#""dimension": "2D", {BEAM}, "supports": [{{"node": "1", "fix": ["ux", "uz"]}}, {{"node": "2", "fix": ["uz"]}}],
            "load_cases": [{{"id": "LC", "frame_loads": [{{"type": "point", "frame": "1", "dir": "Z", "p": -10, "at": 0}}]}}]"#
        ))
        .unwrap();
        let e = &r.cases[0].frames[0];
        assert!(e.forces_at(0.0)[1].abs() < 1e-12);
        assert!((r.cases[0].reaction(&"1".into()).unwrap().fz - 10.0).abs() < 1e-12);
    }

    #[test]
    fn equilibrium_uses_supports_only_and_reports_residual() {
        let r = run(&format!(
            r#"{BEAM}, "supports": [{{"node": "1", "fix": "fixed"}}],
            "load_cases": [{{"id": "LC", "node_loads": [{{"node": "2", "fy": 3, "fz": -10, "mx": 1}}],
                "frame_loads": [{{"type": "uniform", "frame": "1", "dir": "Z", "w": -2}}]}}]"#
        ))
        .unwrap();
        let eq = r.cases[0].equilibrium.as_ref().unwrap();
        assert!(eq.is_ok(), "{eq:?}");
        // Reactions in the check are exactly the reported support reactions here.
        let rep = r.cases[0].reaction(&"1".into()).unwrap().as_array();
        for d in 0..3 {
            assert!((eq.reactions[d] - rep[d]).abs() < 1e-9);
        }
        assert!((eq.applied[2] - (-10.0 - 8.0)).abs() < 1e-12);
    }

    #[test]
    fn stiff_member_on_soft_support_solves_with_a_warning() {
        let r = analyze(
            &Model::from_json(
                r#"{"materials": [{"id": "m", "E": 200e6, "nu": 0.3}],
                "sections": [{"id": "soft", "A": 1e-6}, {"id": "stiff", "A": 1e5}],
                "nodes": [{"id": "1"}, {"id": "2", "x": 1}, {"id": "3", "x": 2}],
                "frames": [{"id": "1", "type": "truss", "nodes": ["1", "2"], "material": "m", "section": "soft"},
                             {"id": "2", "type": "truss", "nodes": ["2", "3"], "material": "m", "section": "stiff"}],
                "supports": [{"node": "1", "fix": "pinned"}, {"node": "2", "fix": ["uy", "uz"]}, {"node": "3", "fix": ["uy", "uz"]}],
                "load_cases": [{"id": "LC", "node_loads": [{"node": "3", "fx": 1}]}]}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(r.notes.iter().any(|n| n.contains("Ill-conditioned")));
        // Soft bar EA/L = 200 carries the load.
        let ux = r.cases[0].displacement(&"3".into()).unwrap().ux;
        assert!((ux - 1.0 / 200.0).abs() < 1e-7, "{ux}");
    }

    #[test]
    fn springs_support_a_cantilever_and_report_their_forces() {
        // 2D cantilever whose base is held by springs in UZ and RY (UX restrained), tip load P down.
        let (kz, kr, p, l, ei): (f64, f64, f64, f64, f64) =
            (5000.0, 20000.0, 10.0, 4.0, 200e6 * 2e-4);
        let r = run(&format!(
            r#""dimension": "2D", {BEAM}, "supports": [{{"node": "1", "fix": ["ux"]}}],
            "springs": [{{"node": "1", "uz": {kz}, "ry": {kr}}}],
            "load_cases": [{{"id": "LC", "node_loads": [{{"node": "2", "fz": -{p}}}]}}]"#
        ))
        .unwrap();
        let c = &r.cases[0];
        let tip = c.displacement(&"2".into()).unwrap().uz;
        let expect = -(p / kz + p * l * l / kr + p * l.powi(3) / (3.0 * ei));
        assert!((tip - expect).abs() < 1e-12, "{tip} vs {expect}");
        // Spring forces are reported as reactions (forces on the structure).
        let base = c.reaction(&"1".into()).unwrap();
        assert!(
            (base.fz - p).abs() < 1e-9 && (base.my + p * l).abs() < 1e-9,
            "{base:?}"
        );
        assert!(c.equilibrium_ok(), "{:?}", c.equilibrium);
        assert!(r.notes.is_empty(), "{:?}", r.notes);
    }

    #[test]
    fn spring_on_a_restrained_dof_does_nothing_and_says_so() {
        let r = run(&format!(
            r#"{BEAM}, "supports": [{{"node": "1", "fix": "fixed"}}],
            "springs": [{{"node": "1", "uz": 100}}, {{"node": "2", "ux": 50}}],
            "load_cases": [{{"id": "LC", "node_loads": [{{"node": "2", "fx": 5, "fz": -1}}]}}]"#
        ))
        .unwrap();
        assert!(
            r.notes.iter().any(|n| n.contains("no effect: 1:uz")),
            "{:?}",
            r.notes
        );
        let c = &r.cases[0];
        assert!((c.reaction(&"1".into()).unwrap().fz - 1.0).abs() < 1e-12);
        // The axial spring at the tip shares the load with the member (EA/L = 5e5).
        let ux = c.displacement(&"2".into()).unwrap().ux;
        assert!((ux - 5.0 / (5e5 + 50.0)).abs() < 1e-15, "{ux}");
        assert!((c.reaction(&"2".into()).unwrap().fx + 50.0 * ux).abs() < 1e-15);
        assert!(c.equilibrium_ok());
    }

    #[test]
    fn rigid_bracket_on_a_column() {
        // Column 1-2 (fixed base, H = 3) with a rigid arm 2-3 of length e along +X; P down at 3.
        let (h, e, p) = (3.0, 0.8, 12.0);
        let r = run(&format!(
            r#""nodes": [{{"id": "1"}}, {{"id": "2", "z": {h}}}, {{"id": "3", "x": {e}, "z": {h}}}],
            "frames": [{{"id": "C1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"}}],
            "links": [{{"id": "L1", "type": "rigid", "nodes": ["3", "2"]}}],
            "supports": [{{"node": "1", "fix": "fixed"}}],
            "load_cases": [{{"id": "LC", "node_loads": [{{"node": "3", "fz": -{p}}}]}}]"#
        ))
        .unwrap();
        let c = &r.cases[0];
        let (ea, ei33) = (200e6 * 0.01, 200e6 * 2e-4);
        // The column top carries P and the moment e P about +Y (bending about local 3 = +Y).
        let d2 = c.displacement(&"2".into()).unwrap();
        let ry = e * p * h / ei33;
        assert!((d2.ry - ry).abs() < 1e-15, "{} vs {ry}", d2.ry);
        assert!((d2.ux - e * p * h * h / (2.0 * ei33)).abs() < 1e-15);
        assert!((d2.uz + p * h / ea).abs() < 1e-15);
        // Node 3 moves with the arm: uz3 = uz2 - ry * e.
        let d3 = c.displacement(&"3".into()).unwrap();
        assert!((d3.uz - (d2.uz - ry * e)).abs() < 1e-15);
        assert!((d3.ry - d2.ry).abs() < 1e-18 && (d3.ux - d2.ux).abs() < 1e-18);
        // The link (reversed to the end order: end i = 2) is a cantilever arm: V2 = -P and
        // M3 = -e P at the column, M3 = 0 at the tip.
        let l = c.link(&"L1".into()).unwrap();
        assert_eq!(l.nodes, ["2".into(), "3".into()]);
        let fi = l.forces_at(0.0);
        let fj = l.forces_at(e);
        assert!(
            (fi[1] + p).abs() < 1e-12 && (fi[5] + e * p).abs() < 1e-12,
            "{fi:?}"
        );
        assert!(fj[5].abs() < 1e-12 && fj[0].abs() < 1e-12, "{fj:?}");
        assert!((l.stations.last().unwrap().uz - d3.uz).abs() < 1e-18);
        let col = c.frame(&"C1".into()).unwrap().forces_at(h);
        assert!((col[0] + p).abs() < 1e-9, "{col:?}");
        assert!(c.equilibrium_ok(), "{:?}", c.equilibrium);
        let base = c.reaction(&"1".into()).unwrap();
        assert!(
            (base.fz - p).abs() < 1e-9 && (base.my + e * p).abs() < 1e-9,
            "{base:?}"
        );
    }

    #[test]
    fn rigid_body_on_springs_carries_slave_springs_and_loads() {
        // Joint 1 on springs in all six DOFs, joint 2 rigidly linked 2 m along X with its own UZ
        // spring, 10 down at joint 2. Only UZ and RY of the master move:
        //   (k1z + k2) uz - 2 k2 ry = -10,   -2 k2 uz + (k1r + 4 k2) ry = 20.
        let (k1z, k1r, k2) = (1000.0, 500.0, 2000.0);
        let r = analyze(
            &Model::from_json(&format!(
                r#"{{"materials": [], "sections": [], "frames": [],
                "nodes": [{{"id": "1"}}, {{"id": "2", "x": 2}}],
                "links": [{{"id": "L1", "type": "rigid", "nodes": ["1", "2"]}}],
                "springs": [{{"node": "1", "ux": 1000, "uy": 1000, "uz": {k1z}, "rx": 500, "ry": {k1r}, "rz": 500}},
                            {{"node": "2", "uz": {k2}}}],
                "load_cases": [{{"id": "LC", "node_loads": [{{"node": "2", "fz": -10}}]}}]}}"#
            ))
            .unwrap(),
        )
        .unwrap();
        let (a, b, d) = (k1z + k2, -2.0 * k2, k1r + 4.0 * k2);
        let det = a * d - b * b;
        let uz = (-10.0 * d - b * 20.0) / det;
        let ry = (a * 20.0 - b * -10.0) / det;
        let c = &r.cases[0];
        let d1 = c.displacement(&"1".into()).unwrap();
        assert!(
            (d1.uz - uz).abs() < 1e-15 && (d1.ry - ry).abs() < 1e-15,
            "{d1:?}"
        );
        let d2 = c.displacement(&"2".into()).unwrap();
        assert!((d2.uz - (uz - 2.0 * ry)).abs() < 1e-15);
        assert!((c.reaction(&"2".into()).unwrap().fz + k2 * d2.uz).abs() < 1e-12);
        assert!(c.equilibrium_ok(), "{:?}", c.equilibrium);
    }

    #[test]
    fn member_inside_a_rigid_body_carries_only_its_own_load() {
        // Fixed column 1-2 and beam 2-3 whose ends are also rigidly linked (through joint 4): the
        // beam cannot bend, so it only carries its own load as fixed-end forces.
        let r = run(r#""nodes": [{"id": "1"}, {"id": "2", "z": 3}, {"id": "3", "x": 2, "z": 3}, {"id": "4", "x": 1, "z": 3}],
            "frames": [{"id": "C1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"},
                         {"id": "B1", "type": "frame", "nodes": ["2", "3"], "material": "m", "section": "s"}],
            "links": [{"id": "L1", "type": "rigid", "nodes": ["2", "4"]}, {"id": "L2", "type": "rigid", "nodes": ["4", "3"]}],
            "supports": [{"node": "1", "fix": "fixed"}],
            "load_cases": [{"id": "LC", "frame_loads": [{"type": "uniform", "frame": "B1", "dir": "Z", "w": -6}]}]"#)
        .unwrap();
        let c = &r.cases[0];
        let b1 = c.frame(&"B1".into()).unwrap();
        // Fixed-end moment w L^2 / 12 = 2 at both ends (hogging).
        assert!(
            (b1.forces_at(0.0)[5] + 2.0).abs() < 1e-9,
            "{:?}",
            b1.forces_at(0.0)
        );
        assert!(c.equilibrium_ok(), "{:?}", c.equilibrium);
        let base = c.reaction(&"1".into()).unwrap();
        assert!(
            (base.fz - 12.0).abs() < 1e-9 && (base.my + 12.0).abs() < 1e-9,
            "{base:?}"
        );
    }

    #[test]
    fn bad_springs_and_links_are_rejected() {
        let col = r#""nodes": [{"id": "1"}, {"id": "2", "z": 3}, {"id": "3", "x": 1, "z": 3}, {"id": "4", "x": 1}],
            "frames": [{"id": "C1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"}]"#;
        let cases = [
            (r#""springs": [{"node": "9", "ux": 1}]"#, "unknown node"),
            (r#""springs": [{"node": "1", "ux": -1}]"#, ">= 0"),
            (
                r#""springs": [{"node": "1", "ux": 1}, {"node": "1", "uy": 1}]"#,
                "more than one spring",
            ),
            (
                r#""links": [{"id": "C1", "type": "rigid", "nodes": ["2", "3"]}]"#,
                "frame already",
            ),
            (
                r#""links": [{"id": "L1", "type": "rigid", "nodes": ["2", "8"]}]"#,
                "unknown node",
            ),
            (
                r#""links": [{"id": "L1", "type": "rigid", "nodes": ["2", "2"]}]"#,
                "same point",
            ),
            (
                r#""links": [{"id": "L1", "type": "rigid", "nodes": ["2", "3"]}, {"id": "L2", "type": "rigid", "nodes": ["3", "4"]},
                    {"id": "L3", "type": "rigid", "nodes": ["4", "2"]}]"#,
                "closes a loop",
            ),
            (
                r#""links": [{"id": "L1", "type": "rigid", "nodes": ["1", "4"]}],
                "supports": [{"node": "1", "fix": "fixed"}, {"node": "4", "fix": ["uz"]}]"#,
                "restraints at one joint only",
            ),
        ];
        for (extra, expect) in cases {
            let msg = invalid(&format!("{col}, {extra}"));
            assert!(msg.contains(expect), "expected '{expect}' in: {msg}");
        }
    }

    #[test]
    fn edge_constraint_ties_a_joint_to_a_shell_edge() {
        // A 2 x 2 slab fixed along y = 0 and a beam ending at (1, 2), the middle of the far edge: only the
        // edge constraint carries the beam.
        let body = r#""area_sections": [{"id": "S", "type": "shell-thin", "material": "m", "thickness": 0.2}],
            "nodes": [{"id": "1"}, {"id": "2", "x": 2}, {"id": "3", "x": 2, "y": 2}, {"id": "4", "y": 2},
                      {"id": "5", "x": 1, "y": 2}, {"id": "6", "x": 1, "y": 4}],
            "areas": [{"id": "F1", "nodes": ["1", "2", "3", "4"], "section": "S"}],
            "frames": [{"id": "B1", "type": "frame", "nodes": ["5", "6"], "material": "m", "section": "s"}],
            "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"}],
            "load_cases": [{"id": "P", "node_loads": [{"node": "6", "fz": -10}]}]"#;
        let stop = format!(r#""on_instability": "stop", {body}"#);
        assert!(matches!(run(&stop), Err(Error::Unstable { .. })));
        let tied = body.replace(
            r#""load_cases""#,
            r#""edge_constraints": [{"area": "F1", "edge": ["3", "4"], "node": "5"}], "load_cases""#,
        );
        let r = run(&tied).unwrap();
        let c = &r.cases[0];
        assert!(r
            .notes
            .iter()
            .any(|n| n.contains("Edge constraints tie 1 joints")));
        assert!(c.equilibrium.as_ref().unwrap().relative_error < 1e-9);
        // The joint follows the edge as a beam would (cubic interpolation; the edge 3-4 runs along -X,
        // so the slope along it is ry), up to the constraint's own flexibility.
        let d = |n: &str| *c.displacement(&n.into()).unwrap();
        let mid = (d("3").uz + d("4").uz) / 2.0 + 0.25 * (d("3").ry - d("4").ry);
        assert!(
            (d("5").uz - mid).abs() < 0.01 * mid.abs(),
            "{} vs {mid}",
            d("5").uz
        );
        for (ec, expect) in [
            (
                r#"{"area": "F9", "edge": ["3", "4"], "node": "5"}"#,
                "unknown area",
            ),
            (
                r#"{"area": "F1", "edge": ["1", "3"], "node": "5"}"#,
                "not the ends of one of its edges",
            ),
            (
                r#"{"area": "F1", "edge": ["3", "4"], "node": "9"}"#,
                "unknown node",
            ),
            (
                r#"{"area": "F1", "edge": ["3", "4"], "node": "6"}"#,
                "does not lie inside",
            ),
        ] {
            let bad = body.replace(
                r#""load_cases""#,
                &format!(r#""edge_constraints": [{ec}], "load_cases""#),
            );
            let msg = invalid(&bad);
            assert!(msg.contains(expect), "expected '{expect}' in: {msg}");
        }
    }

    #[test]
    fn empty_model_solves() {
        let r = analyze(
            &Model::from_json(r#"{"materials": [], "sections": [], "nodes": [], "frames": [], "load_cases": [{"id": "LC"}]}"#)
                .unwrap(),
        )
        .unwrap();
        assert!(r.cases[0].displacements.is_empty());
    }
}
