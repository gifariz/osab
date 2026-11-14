//! Lumped joint masses for modal analysis (docs/phases/PHASE4_PLAN.md, probe findings
//! 4):
//! - element mass: the structure's weight (material weight x A or t x mass modifiers) / g, lumped like
//!   self-weight loads: half of a frame's mass to each end of the frame object (beams over their clear
//!   length; a frame the mesh step split still gives its mass to the object's ends only), a
//!   shell's to its corners like a uniform area load; no rotational inertia from elements;
//! - added joint masses as given;
//! - mass from load patterns (in files without patterns: load cases): downward load / g where it acts (an upward load takes mass off, other
//!   directions do nothing), frame loads to the ends by simple-beam statics, the case's self-weight too
//!   (a frame's half to each end, like element mass; probe 9);
//!   a Membrane floor's loads at its elements' corners, not after the transfer to the beams (probe 8,
//!   `LoadCase::mass_loads`);
//! - lateral only (the default): UX, UY and RZ keep their mass, UZ, RX and RY none;
//! - lumped at story levels (the default): the lateral mass of a joint between two levels is split
//!   between them linearly by height, each part to the nearest joint at that level (in plan, joined to it or not; of joints
//!   equally near the first in node order; probe 10); above the top level
//!   all of it goes to the top level. Vertical mass stays where it is.

use std::collections::HashMap;

use crate::axes;
use crate::model::{coords, length, FrameLoad, Id, LoadCase, LoadDir, Model, Node};

/// Joint masses (UX UY UZ RX RY RZ) of every node of `model` (the analysis model, in node order).
/// `area_corners(area, qz)` gives the corner forces (Z) of a uniform Z load `qz` on area `area` (its
/// index), as the analysis applies area loads.
pub fn lumped(model: &Model, area_corners: &dyn Fn(usize, f64) -> Vec<f64>) -> Vec<[f64; 6]> {
    let n = model.nodes.len();
    let src = model.mass_source.clone().unwrap_or_default();
    let g = model.units.gravity();
    let index: HashMap<&Id, usize> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| (&nd.id, i))
        .collect();
    let mut m = vec![[0.0f64; 6]; n];
    // Downward forces at the joints from a set of loads, / g, as translational mass.
    let mut add_loads = |c: &LoadCase, scale: f64, halves: bool| {
        let down = downward(model, c, &index, area_corners, halves);
        for (k, f) in down.iter().enumerate() {
            for d in 0..3 {
                m[k][d] += scale * f / g;
            }
        }
    };
    // The structure's weight (`mass`: its mass times g) times `s` as loads: half of a frame's to each end.
    let gravity = |s: f64, mass: bool| {
        let mut c = LoadCase {
            id: Id("mass".into()),
            ..Default::default()
        };
        model.add_gravity(&mut c, s, mass);
        c
    };
    if src.elements {
        add_loads(&gravity(1.0, true), 1.0, true);
    }
    for l in &src.loads {
        let c = match (&l.loads, &l.case) {
            (Some(c), _) => Some(&**c),
            (None, Some(id)) => model.load_cases.iter().find(|c| &c.id == id),
            _ => None,
        };
        if let Some(c) = c {
            let c = c.mass_loads.as_deref().unwrap_or(c);
            add_loads(c, l.scale, false);
            // The case's self-weight goes like element mass, half to each end (probe 9).
            if c.self_weight != 0.0 {
                add_loads(&gravity(c.self_weight, false), l.scale, true);
            }
        }
    }
    if src.joint_masses {
        for jm in &model.masses {
            let k = index[&jm.node];
            for (d, v) in jm.as_array().iter().enumerate() {
                m[k][d] += v;
            }
        }
    }
    if src.lateral_only {
        for x in &mut m {
            x[2] = 0.0;
            x[3] = 0.0;
            x[4] = 0.0;
        }
    }
    if src.lump_at_stories && !model.stories.is_empty() {
        lump(model, &mut m);
    }
    m
}

/// Downward force (positive down) at each joint from the loads of `c`: joint loads, frame loads by
/// simple-beam statics, area loads like the analysis applies them. Only the Z parts count.
fn downward(
    model: &Model,
    c: &LoadCase,
    index: &HashMap<&Id, usize>,
    area_corners: &dyn Fn(usize, f64) -> Vec<f64>,
    halves: bool,
) -> Vec<f64> {
    let mut f = vec![0.0; model.nodes.len()];
    for l in &c.node_loads {
        f[index[&l.node]] -= l.fz;
    }
    let frames: HashMap<&Id, usize> = model
        .frames
        .iter()
        .enumerate()
        .map(|(i, e)| (&e.id, i))
        .collect();
    // Frame objects the mesh step split: their two end joints and the direction from the first.
    let mut chains: HashMap<&Id, Vec<usize>> = HashMap::new();
    for e in &model.frames {
        if let Some(o) = &e.object {
            for n in &e.nodes {
                chains.entry(o).or_default().push(index[n]);
            }
        }
    }
    let ends: HashMap<&Id, (usize, usize)> = chains
        .into_iter()
        .filter_map(|(o, list)| {
            let once: Vec<usize> = list
                .iter()
                .copied()
                .filter(|&n| list.iter().filter(|&&m| m == n).count() == 1)
                .collect();
            (once.len() == 2).then(|| (o, (once[0], once[1])))
        })
        .collect();
    for ml in &c.frame_loads {
        let e = &model.frames[frames[ml.frame()]];
        let (a, b) = (index[&e.nodes[0]], index[&e.nodes[1]]);
        let (pa, pb) = (&model.nodes[a], &model.nodes[b]);
        let l = length(pa, pb);
        let ax = axes::local_axes(coords(pa), coords(pb), e.angle);
        let (dir, (total, moment)) = match *ml {
            FrameLoad::Uniform {
                dir, w, from, to, ..
            } => (dir, part(w, w, from.unwrap_or(0.0), to.unwrap_or(l))),
            FrameLoad::Trapezoidal {
                dir,
                w1,
                w2,
                from,
                to,
                ..
            } => (dir, part(w1, w2, from.unwrap_or(0.0), to.unwrap_or(l))),
            FrameLoad::Point { dir, p, at, .. } => (dir, (p, p * at)),
        };
        let z = match dir {
            LoadDir::X | LoadDir::Y => 0.0,
            LoadDir::Z => 1.0,
            LoadDir::L1 => ax[0][2],
            LoadDir::L2 => ax[1][2],
            LoadDir::L3 => ax[2][2],
        };
        // Simple-beam reactions of the Z part over the frame object: R_far = moment / span.
        let (na, nb, first, span) = match e.object.as_ref().and_then(|o| ends.get(o)) {
            Some(&(oa, ob)) => {
                let (xa, xb) = (coords(&model.nodes[oa]), coords(&model.nodes[ob]));
                let span = axes::dot(axes::sub(xb, xa), axes::sub(xb, xa)).sqrt();
                let u = axes::scale(axes::sub(xb, xa), 1.0 / span);
                // Positions along the object: the piece's start, and its direction (+1 or -1).
                let x0 = axes::dot(axes::sub(coords(pa), xa), u);
                let sign = axes::dot(ax[0], u).signum();
                (oa, ob, total * x0 + sign * moment, span)
            }
            None => (a, b, moment, l),
        };
        // Element mass: half to each end of the object (also when an offset at one end only
        // moves the clear length off centre; B54).
        let far = if halves { total / 2.0 } else { first / span };
        f[na] -= z * (total - far);
        f[nb] -= z * far;
    }
    let areas: HashMap<&Id, usize> = model
        .areas
        .iter()
        .enumerate()
        .map(|(i, a)| (&a.id, i))
        .collect();
    for l in &c.area_loads {
        let ai = areas[&l.area];
        let a = &model.areas[ai];
        let qz = match l.dir {
            LoadDir::X | LoadDir::Y => 0.0,
            LoadDir::Z => l.q,
            // Local area axes: their Z part (a flat floor's local 3 is +Z or -Z).
            _ => {
                let p: Vec<[f64; 3]> = a
                    .nodes
                    .iter()
                    .map(|id| coords(&model.nodes[index[id]]))
                    .collect();
                let ax = axes::area_axes(&p, a.angle);
                let k = match l.dir {
                    LoadDir::L1 => 0,
                    LoadDir::L2 => 1,
                    _ => 2,
                };
                l.q * ax[k][2]
            }
        };
        if qz == 0.0 {
            continue;
        }
        for (k, v) in area_corners(ai, qz).iter().enumerate() {
            f[index[&a.nodes[k]]] -= v;
        }
    }
    f
}

/// Resultant and first moment about end i of a load varying from `w1` at `a` to `w2` at `b`.
fn part(w1: f64, w2: f64, a: f64, b: f64) -> (f64, f64) {
    crate::elements::distributed_part(w1, w2, a, b, a, b)
}

/// Lumping of lateral mass at story levels (see the module notes).
fn lump(model: &Model, m: &mut [[f64; 6]]) {
    let span = model
        .nodes
        .iter()
        .flat_map(|n| [n.x.abs(), n.y.abs(), n.z.abs()])
        .fold(1.0f64, f64::max);
    let tol = 1e-9 * span;
    let levels: Vec<f64> = model.stories.iter().map(|s| s.z).collect();
    let at_level = |z: f64| levels.iter().any(|l| (l - z).abs() <= tol);
    // The joints at each level.
    let on: Vec<Vec<usize>> = levels
        .iter()
        .map(|l| {
            (0..model.nodes.len())
                .filter(|&k| (model.nodes[k].z - l).abs() <= tol)
                .collect()
        })
        .collect();
    let grids: Vec<PlanGrid> = on.iter().map(|js| PlanGrid::new(model, js)).collect();
    let nearest =
        |level: usize, from: &Node| -> Option<usize> { grids[level].nearest(model, from) };
    for k in 0..model.nodes.len() {
        let node = &model.nodes[k];
        let lateral = [m[k][0], m[k][1], m[k][5]];
        if lateral.iter().all(|v| *v == 0.0) || at_level(node.z) {
            continue;
        }
        // Shares of the levels below and above (all to the top or base level outside them).
        let above = levels.iter().position(|l| *l > node.z);
        let shares: Vec<(usize, f64)> = match above {
            None => vec![(levels.len() - 1, 1.0)],
            Some(0) => vec![(0, 1.0)],
            Some(i) => {
                let (lo, hi) = (levels[i - 1], levels[i]);
                let up = (node.z - lo) / (hi - lo);
                vec![(i - 1, 1.0 - up), (i, up)]
            }
        };
        let mut moved = 0.0;
        for (level, share) in shares {
            let Some(t) = nearest(level, node) else {
                continue;
            };
            for (d, v) in [0, 1, 5].into_iter().zip(lateral) {
                m[t][d] += share * v;
            }
            moved += share;
        }
        for d in [0, 1, 5] {
            m[k][d] *= 1.0 - moved;
        }
    }
}

/// The joints of one level in square plan cells, for the nearest joint to a point.
struct PlanGrid {
    joints: Vec<usize>,
    origin: [f64; 2],
    cell: f64,
    cells: HashMap<[i64; 2], Vec<usize>>,
    /// The cell range holding joints.
    lo: [i64; 2],
    hi: [i64; 2],
}

impl PlanGrid {
    fn new(model: &Model, joints: &[usize]) -> PlanGrid {
        let xy = |k: usize| [model.nodes[k].x, model.nodes[k].y];
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for &k in joints {
            for d in 0..2 {
                min[d] = min[d].min(xy(k)[d]);
                max[d] = max[d].max(xy(k)[d]);
            }
        }
        // About two joints per cell.
        let extent = (max[0] - min[0]).max(max[1] - min[1]).max(0.0);
        let cell = if joints.len() > 1 && extent > 0.0 {
            (extent * extent * 2.0 / joints.len() as f64)
                .sqrt()
                .max(extent / 4096.0)
        } else {
            1.0
        };
        let origin = if joints.is_empty() { [0.0; 2] } else { min };
        let mut grid = PlanGrid {
            joints: joints.to_vec(),
            origin,
            cell,
            cells: HashMap::new(),
            lo: [i64::MAX; 2],
            hi: [i64::MIN; 2],
        };
        for &k in joints {
            let c = grid.key(xy(k));
            for d in 0..2 {
                grid.lo[d] = grid.lo[d].min(c[d]);
                grid.hi[d] = grid.hi[d].max(c[d]);
            }
            grid.cells.entry(c).or_default().push(k);
        }
        grid
    }

    fn key(&self, p: [f64; 2]) -> [i64; 2] {
        [0, 1].map(|d| ((p[d] - self.origin[d]) / self.cell).floor() as i64)
    }

    /// The joint nearest to `from` (squared distance in space; of equally near ones the first in node
    /// order), searching rings of cells outward until no farther cell can hold a joint as near.
    fn nearest(&self, model: &Model, from: &Node) -> Option<usize> {
        if self.joints.is_empty() {
            return None;
        }
        let d = |k: usize| {
            let v = axes::sub(coords(&model.nodes[k]), coords(from));
            axes::dot(v, v)
        };
        let c = self.key([from.x, from.y]);
        let mut best: Option<(f64, usize)> = None;
        let look = |cell: [i64; 2], best: &mut Option<(f64, usize)>| {
            for &k in self.cells.get(&cell).into_iter().flatten() {
                let dk = d(k);
                if best.is_none_or(|(bd, bk)| dk < bd || (dk == bd && k < bk)) {
                    *best = Some((dk, k));
                }
            }
        };
        // Rings of cells around c (Chebyshev distance `ring`), from the first one reaching the joints'
        // cells; only the cells inside the joints' range are visited.
        let first = (0..2)
            .map(|i| (self.lo[i] - c[i]).max(c[i] - self.hi[i]).max(0))
            .max()
            .unwrap_or(0);
        let last = (0..2)
            .map(|i| (self.hi[i] - c[i]).max(c[i] - self.lo[i]))
            .max()
            .unwrap_or(0);
        let (xs, ys) = (self.lo[0]..=self.hi[0], self.lo[1]..=self.hi[1]);
        for ring in first..=last {
            let (x0, x1) = (c[0] - ring, c[0] + ring);
            let (y0, y1) = (c[1] - ring, c[1] + ring);
            for y in [y0, y1] {
                if ys.contains(&y) && (ring > 0 || y == y0) {
                    for x in x0.max(self.lo[0])..=x1.min(self.hi[0]) {
                        look([x, y], &mut best);
                    }
                }
            }
            for x in [x0, x1] {
                if xs.contains(&x) && ring > 0 {
                    for y in (y0 + 1).max(self.lo[1])..=(y1 - 1).min(self.hi[1]) {
                        look([x, y], &mut best);
                    }
                }
            }
            // Joints in rings farther out are at least `ring` cells away in plan.
            let gap = ring as f64 * self.cell;
            if best.is_some_and(|(bd, _)| gap * gap > bd * (1.0 + 1e-12)) {
                break;
            }
        }
        best.map(|(_, k)| k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_grid_finds_the_nearest_joint_like_checking_every_joint() {
        // Joints on a jittered grid with repeated points (ties), at one level, and query points
        // inside, between and far outside them.
        let node = |id: String, x: f64, y: f64, z: f64| Node {
            id: Id(id),
            x,
            y,
            z,
            in_mesh: None,
            diaphragm: None,
        };
        let mut nodes = Vec::new();
        let mut seed = 7u64;
        let mut rand = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for k in 0..400 {
            let (i, j) = ((k % 20) as f64, (k / 20) as f64);
            let (x, y) = if k % 7 == 0 {
                (3.0, 4.0)
            } else {
                (i * 2.5 + rand() * 0.3, j * 1.5 + rand() * 0.3)
            };
            nodes.push(node(format!("{k}"), x, y, 10.0));
        }
        let mut model = Model::from_json(
            r#"{"materials": [], "sections": [], "nodes": [], "frames": [], "load_cases": []}"#,
        )
        .unwrap();
        model.nodes = nodes;
        let joints: Vec<usize> = (0..model.nodes.len()).filter(|k| k % 3 != 1).collect();
        let grid = PlanGrid::new(&model, &joints);
        for q in 0..300 {
            let far = if q % 50 == 0 { 1e6 } else { 0.0 };
            let (x, y, z) = (
                (rand() - 0.2) * 80.0 + far,
                (rand() - 0.2) * 40.0,
                7.0 + rand(),
            );
            let from = node("q".into(), x, y, z);
            let every = joints.iter().copied().min_by(|&a, &b| {
                let d = |k: usize| {
                    let v = axes::sub(coords(&model.nodes[k]), coords(&from));
                    axes::dot(v, v)
                };
                d(a).total_cmp(&d(b))
            });
            assert_eq!(grid.nearest(&model, &from), every, "point {q}");
        }
        assert_eq!(
            PlanGrid::new(&model, &[]).nearest(&model, &model.nodes[0]),
            None
        );
    }
}
