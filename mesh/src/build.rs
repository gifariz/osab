//! The mesh step: an object model (any flat polygon areas, openings, frames) in, an analysis model out
//! (3- and 4-node shell elements, frame pieces, mesh joints `~1…`, edge constraints).
//!
//! Every area is cut into faces at its edges, openings, the frames and other areas' edges lying in it
//! (and, for the Rectangular mesh and walls, lines along its local axes through its key points). Face
//! edges shared by several objects are divided once, into ceil(length / size) equal pieces with the
//! smallest size of the objects using them, so neighbouring meshes share their joints. A four-sided face
//! with matching opposite sides gets a mapped grid (an equal grid on rectangles), any other face a
//! quad-dominant mesh. Frames are split at every joint along them.

use std::collections::{BTreeMap, HashMap, HashSet};

use osab_solver::axes::{self, Axes, Vec3};
use osab_solver::model::{
    coords, Area, AreaLoad, AreaMesh, AreaMeshWord, EndOffsets, FloorMesh, Frame, FrameKind,
    FrameLoad, Id, LoadDir, MeshSetting, MeshSettings, Model, NodeLoad, Release, Releases,
    ShellType, SlabType, Support,
};
use osab_solver::results::{MeshElement, MeshInfo, MeshJoint};
use osab_solver::Error;

use crate::arrange;
use crate::boxes::{self, Boxes};
use crate::geom::{self, Plane, P2};
use crate::grade;
use crate::hanging;
use crate::mapped;
use crate::nulls;
use crate::panels;
use crate::points::Points;
use crate::rect;
use crate::tri;

/// The analysis model and where its parts came from.
pub struct Meshed {
    pub model: Model,
    pub info: MeshInfo,
    /// Frame objects split into pieces: object id and its pieces from end i.
    pub pieces: Vec<(Id, Vec<Id>)>,
    /// End length offsets of the split frame objects (the object's results cover the clear length
    /// between them).
    pub offsets: HashMap<Id, [f64; 2]>,
    /// Parent area of every shell element.
    pub parent: HashMap<Id, Id>,
    pub notes: Vec<String>,
    /// Joints left out of the analysis (nothing attached, *Include in mesh* = No): their loads
    /// went to the floor elements around them, and they stay put.
    pub left_out: Vec<Id>,
}

fn invalid(msg: String) -> Error {
    Error::Invalid(msg)
}

/// The default maximum mesh size, 1.25 m, in the model's length unit.
pub fn default_size(length_unit: &str) -> f64 {
    match length_unit.to_ascii_lowercase().as_str() {
        "mm" => 1250.0,
        "cm" => 125.0,
        "ft" => 4.0,
        "in" => 48.0,
        _ => 1.25,
    }
}

/// Unit normal of a polygon (Newell's method).
fn newell(p: &[Vec3]) -> Vec3 {
    let mut n = [0.0; 3];
    for k in 0..p.len() {
        let (a, b) = (p[k], p[(k + 1) % p.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    axes::normalize(n)
}

/// Local axes of an area (as the solver's `area_axes`, for any number of corners).
pub(crate) fn area_axes(p: &[Vec3], angle: f64) -> Axes {
    if p.len() <= 4 {
        return axes::area_axes(p, angle);
    }
    let e3 = newell(p);
    let mut reference = [0.0, 0.0, 1.0];
    let c = axes::cross(e3, reference);
    if axes::dot(c, c).sqrt() < 1e-3 {
        reference = [0.0, 1.0, 0.0];
    }
    let e1 = axes::normalize(axes::cross(reference, e3));
    let e2 = axes::cross(e3, e1);
    let (s, co) = angle.to_radians().sin_cos();
    [
        axes::add(axes::scale(e1, co), axes::scale(e2, s)),
        axes::add(axes::scale(e1, -s), axes::scale(e2, co)),
        e3,
    ]
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// One element (3 or 4 corners).
    None,
    General,
    Rectangular,
    /// A Membrane floor: cut at beams and walls into convex panels, one element each.
    Panels,
}

struct Host {
    index: usize,
    plane: Plane,
    corners: Vec<P2>,
    holes: Vec<Vec<P2>>,
    hole_areas: Vec<usize>,
    h: f64,
    kind: Kind,
    wall: bool,
    /// A Membrane wall (the default for walls): one element, or cut only along its openings' lines,
    /// since a meshed wall with no out-of-plane stiffness would leave its inside joints loose.
    cut_at_openings_only: bool,
    /// Who wins where areas overlap: the higher rank (Stiff 2, Drop 1, Slab 0), else the
    /// smaller area.
    rank: u8,
    size: f64,
    /// A Membrane floor: its vertical loads go to its panels' edges by tributary areas.
    membrane_floor: bool,
    /// Its section's one-way load distribution: strips along local axis 1 instead.
    one_way: bool,
    /// Where grading refines the mesh (General floors with `refine`): re-entrant and opening corners.
    hot: Vec<P2>,
}

/// Whether two polygons (in one plane, 2D) overlap: a corner of one inside the other, or edges crossing.
pub(crate) fn polygons_overlap(a: &[P2], b: &[P2], tol: f64) -> bool {
    let strictly = |p: P2, poly: &[P2]| {
        geom::inside(p, poly, 0.0)
            && (0..poly.len()).all(|k| geom::seg_dist(p, poly[k], poly[(k + 1) % poly.len()]) > tol)
    };
    if a.iter().any(|p| strictly(*p, b)) || b.iter().any(|p| strictly(*p, a)) {
        return true;
    }
    for i in 0..a.len() {
        for j in 0..b.len() {
            let (p, q) = (a[i], a[(i + 1) % a.len()]);
            let (r, s) = (b[j], b[(j + 1) % b.len()]);
            if let Some((t, u)) = geom::seg_cross(p, q, r, s, 0.0) {
                let (lp, lr) = (geom::dist(p, q), geom::dist(r, s));
                if t * lp > tol && (1.0 - t) * lp > tol && u * lr > tol && (1.0 - u) * lr > tol {
                    return true;
                }
            }
        }
    }
    // The same polygon (or one exactly on the other) overlaps too.
    let centroid = |p: &[P2]| {
        geom::scale(
            p.iter().fold([0.0, 0.0], |s, q| geom::add(s, *q)),
            1.0 / p.len() as f64,
        )
    };
    strictly(centroid(a), b) || strictly(centroid(b), a)
}

/// Merge tolerance of a model: 1e-6 of the size of the box round its joints.
pub(crate) fn model_tol(xyz: &[Vec3]) -> f64 {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in xyz {
        for d in 0..3 {
            lo[d] = lo[d].min(p[d]);
            hi[d] = hi[d].max(p[d]);
        }
    }
    let span = if xyz.is_empty() {
        1.0
    } else {
        geom::dist3(lo, hi).max(1e-9)
    };
    1e-6 * span
}

/// Runs the mesh step. A model with `"mesh": "off"` is read as every area set to no mesh (one element
/// each). Areas with a stored mesh keep it (see [`crate::stored`]).
pub fn mesh(model: &Model) -> Result<Meshed, Error> {
    model.check_format()?;
    let model = &*crate::section::resolve(model)?;
    // Groups name the model's objects; the analysis model has others (pieces) and needs none.
    let model = &*if model.groups.is_empty() {
        std::borrow::Cow::Borrowed(model)
    } else {
        model.check_groups()?;
        let mut m = model.clone();
        m.groups.clear();
        std::borrow::Cow::Owned(m)
    };
    let model = &*model.with_patterns()?;
    let mut m = if model.areas.iter().any(|a| a.user_mesh.is_some()) {
        let mut clock = osab_solver::timing::Clock::new();
        let (work, stored) = crate::stored::expand(model)?;
        clock.lap("mesh: stored meshes expanded");
        let mut m = mesh_objects(&work)?;
        clock = osab_solver::timing::Clock::new();
        stored.restore(&mut m);
        clock.lap("mesh: stored meshes restored");
        m
    } else {
        mesh_objects(model)?
    };
    wall_sections(model, &mut m);
    Ok(m)
}

/// Piers and spandrels come from the objects (their corners, stories and order); the solver sums
/// the forces of their elements: an area's shell elements, a frame's pieces.
fn wall_sections(model: &Model, m: &mut Meshed) {
    if model.pier_labels.is_empty() && model.spandrel_labels.is_empty() {
        return;
    }
    let mut notes = Vec::new();
    let mut found = osab_solver::piers::sections(model, &mut notes);
    let mut elements: HashMap<&Id, Vec<Id>> = HashMap::new();
    for a in &m.model.areas {
        if let Some(p) = m.parent.get(&a.id) {
            elements.entry(p).or_default().push(a.id.clone());
        }
    }
    let pieces: HashMap<&Id, &Vec<Id>> = m.pieces.iter().map(|(f, p)| (f, p)).collect();
    for w in &mut found {
        w.areas = w
            .areas
            .iter()
            .flat_map(|a| elements.get(a).cloned().unwrap_or_default())
            .collect();
        w.frames = w
            .frames
            .iter()
            .flat_map(|f| match pieces.get(f) {
                Some(p) => (*p).clone(),
                None => vec![f.clone()],
            })
            .collect();
    }
    m.model.wall_sections = Some(found);
    m.notes.extend(notes);
}

fn mesh_objects(model: &Model) -> Result<Meshed, Error> {
    let mut clock = osab_solver::timing::Clock::new();
    // A model whose mesh setting is "off" (older files): every area is set to no mesh, one element
    // each; frames are still split at the joints, frames and area edges they meet.
    let unmeshed;
    let model = if matches!(model.mesh, Some(MeshSetting::Word(_))) {
        let mut m = model.clone();
        m.mesh = None;
        for a in &mut m.areas {
            a.mesh = Some(AreaMesh::Word(AreaMeshWord::None));
        }
        unmeshed = m;
        &unmeshed
    } else {
        model
    };
    let settings = match &model.mesh {
        Some(MeshSetting::Auto(s)) => s.clone(),
        _ => MeshSettings::default(),
    };
    let size0 = default_size(&model.units.length);
    let floor_size = settings.floor_size.unwrap_or(size0);
    let wall_size = settings.wall_size.unwrap_or(size0);
    for (what, v) in [("floor_size", floor_size), ("wall_size", wall_size)] {
        if !(v > 0.0 && v.is_finite()) {
            return Err(invalid(format!("mesh: {what} must be > 0")));
        }
    }
    let (obj, reversed) = model.oriented();
    let mut notes = Vec::new();
    if !reversed.is_empty() {
        let ids: Vec<String> = reversed.iter().map(|id| id.to_string()).collect();
        notes.push(format!(
            "Reversed so that end i is the lower end (then the lower X, then the lower Y): frames {}",
            ids.join(", ")
        ));
    }
    let node_index: HashMap<&Id, usize> = obj
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (&n.id, i))
        .collect();
    for a in &obj.areas {
        if a.nodes.len() < 3 {
            return Err(invalid(format!(
                "area {}: give at least 3 corner joints",
                a.id
            )));
        }
        for id in &a.nodes {
            if !node_index.contains_key(id) {
                return Err(invalid(format!("area {}: unknown node {id}", a.id)));
            }
        }
    }
    for f in &obj.frames {
        for id in &f.nodes {
            if !node_index.contains_key(id) {
                return Err(invalid(format!("frame {}: unknown node {id}", f.id)));
            }
        }
    }
    let xyz: Vec<Vec3> = obj.nodes.iter().map(coords).collect();
    let tol = model_tol(&xyz);
    let mut points = Points::new(tol);
    for (i, p) in xyz.iter().enumerate() {
        points.add_joint(*p, i);
    }

    clock.lap("mesh: setup");
    // Floors are meshed along frames with *Include in mesh* (the default; null frames too).
    let meshing = |f: &Frame| f.in_mesh != Some(false);
    // Joints the floor mesh must pass through: ends of those frames, links, supports, springs, joints with
    // *Include in mesh* on, and joints lying on a frame (frames are split there). A joint with nothing
    // attached (one that only carries a joint load) is left out; its load goes to the joints
    // of the element it lies in (`nulls`).
    let mut active: HashSet<usize> = HashSet::new();
    for f in obj.frames.iter().filter(|f| meshing(f)) {
        active.extend(f.nodes.iter().map(|id| node_index[id]));
    }
    for l in &obj.links {
        active.extend(l.nodes.iter().filter_map(|id| node_index.get(id).copied()));
    }
    for s in &obj.supports {
        active.extend(node_index.get(&s.node).copied());
    }
    for s in &obj.springs {
        active.extend(node_index.get(&s.node).copied());
    }
    // A joint in a rigid diaphragm counts as attached (it is held by the constraint); a semi-rigid one
    // ties nothing.
    let rigid =
        |d: &osab_solver::model::Id| obj.diaphragms.iter().any(|x| &x.id == d && !x.semi_rigid);
    for (j, n) in obj.nodes.iter().enumerate() {
        if n.in_mesh == Some(true) || n.diaphragm.as_ref().is_some_and(rigid) {
            active.insert(j);
        }
    }
    let joint_boxes = Boxes::new(xyz.iter().map(|p| [*p; 2]).collect());
    for f in obj.frames.iter().filter(|f| meshing(f) && !f.is_null()) {
        let (a, b) = (xyz[node_index[&f.nodes[0]]], xyz[node_index[&f.nodes[1]]]);
        let len = geom::dist3(a, b);
        for j in joint_boxes.meeting(&boxes::around([a, b], 2.0 * tol)) {
            if geom::on_segment3(xyz[j], a, b, tol)
                .is_some_and(|t| t * len > tol && (1.0 - t) * len > tol)
            {
                active.insert(j);
            }
        }
    }
    // For the Rectangular floor mesh: joints held by something other than frames, the other ends of each
    // joint's frames, and the areas each joint is a corner of. A null frame's end j (in end order)
    // counts as held: its lines are kept like a column's (probe findings 2h).
    let on_frame: HashSet<usize> = obj
        .frames
        .iter()
        .flat_map(|f| f.nodes.iter().map(|id| node_index[id]))
        .collect();
    let held: HashSet<usize> = active
        .iter()
        .copied()
        .filter(|j| !on_frame.contains(j))
        .chain(
            obj.supports
                .iter()
                .map(|s| &s.node)
                .chain(obj.springs.iter().map(|s| &s.node))
                .chain(obj.links.iter().flat_map(|l| l.nodes.iter()))
                .chain(
                    obj.frames
                        .iter()
                        .filter(|f| f.is_null() && meshing(f))
                        .map(|f| &f.nodes[1]),
                )
                .filter_map(|id| node_index.get(id).copied()),
        )
        .collect();
    let mut frame_ends: Vec<Vec<usize>> = vec![Vec::new(); obj.nodes.len()];
    for f in obj.frames.iter().filter(|f| !f.is_null()) {
        let (a, b) = (node_index[&f.nodes[0]], node_index[&f.nodes[1]]);
        frame_ends[a].push(b);
        frame_ends[b].push(a);
    }
    let mut corner_of: Vec<Vec<usize>> = vec![Vec::new(); obj.nodes.len()];
    for (ai, a) in obj.areas.iter().enumerate().filter(|(_, a)| !a.is_null()) {
        for id in &a.nodes {
            corner_of[node_index[id]].push(ai);
        }
    }

    clock.lap("mesh: joints to pass through");
    // Planes, shapes and openings of the areas.
    let corner3 =
        |a: &Area| -> Vec<Vec3> { a.nodes.iter().map(|id| xyz[node_index[id]]).collect() };
    let mut hosts: Vec<Host> = Vec::new();
    let mut opening_of: Vec<Option<usize>> = vec![None; obj.areas.len()];
    let mut polygons_meshed: Vec<&Id> = Vec::new();
    let mut warped_kept: Vec<String> = Vec::new();
    const NO_MESH: AreaMesh = AreaMesh::Word(AreaMeshWord::None);
    for (ai, a) in obj.areas.iter().enumerate() {
        let p = corner3(a);
        let ax = area_axes(&p, a.angle);
        let plane = Plane {
            origin: p[0],
            axes: ax,
        };
        let size = p
            .iter()
            .flat_map(|q| p.iter().map(move |r| geom::dist3(*q, *r)))
            .fold(0.0, f64::max);
        // A quad whose corners are off one plane is not meshed: it stays one (warped) element, which
        // the solver analyses on its mean plane. Other areas must be flat.
        let warped = p.iter().any(|q| plane.offset(*q) > 1e-6 * size.max(tol));
        if warped && (p.len() != 4 || a.opening || a.is_null()) {
            let q = p
                .iter()
                .find(|q| plane.offset(**q) > 1e-6 * size.max(tol))
                .unwrap();
            return Err(invalid(format!(
                "area {}: the corners are not in one plane (a corner is {:.3e} off it); only a 4-corner floor or wall may be warped",
                a.id,
                plane.offset(*q)
            )));
        }
        let c2: Vec<P2> = p.iter().map(|q| plane.to2(*q)).collect();
        if geom::area(&c2) <= 1e-12 * size * size {
            return Err(invalid(format!(
                "area {}: its corners are in one line (no area)",
                a.id
            )));
        }
        let n = c2.len();
        for i in 0..n {
            for j in i + 1..n {
                if j == i + 1 || (i == 0 && j == n - 1) {
                    continue;
                }
                let ([a1, b1], [a2, b2]) = ([c2[i], c2[(i + 1) % n]], [c2[j], c2[(j + 1) % n]]);
                if geom::seg_cross(a1, b1, a2, b2, -tol).is_some() {
                    return Err(invalid(format!(
                        "area {}: its edges cross each other",
                        a.id
                    )));
                }
            }
        }
        // Openings cut the areas they lie in; null areas have no elements and do not cut the mesh.
        if a.opening || a.is_null() {
            continue;
        }
        let slope = axes::dot(ax[2], [0.0, 0.0, 1.0])
            .abs()
            .clamp(0.0, 1.0)
            .acos()
            .to_degrees();
        // Floor/wall rule: walls slope the model's limit (20 degrees by default) or more.
        let wall = slope >= obj.wall_slope() - 1e-9;
        let section = obj.area_sections.iter().find(|s| s.id == a.section);
        let membrane = section.is_some_and(|s| s.kind == ShellType::Membrane);
        let one_way = section.is_some_and(|s| s.one_way);
        let membrane_floor = membrane && !wall;
        if membrane_floor && matches!(a.mesh, Some(AreaMesh::Size { .. })) {
            notes.push(format!(
                "Membrane floor {}: its mesh size is not used; it is cut at beams and walls only",
                a.id
            ));
        }
        // One element needs 3 or 4 corners: a polygon set to no mesh gets the model's mesh instead.
        let mesh = match &a.mesh {
            _ if warped => {
                let (off, _) = osab_solver::area::warp(&p);
                if !matches!(a.mesh, Some(AreaMesh::Word(AreaMeshWord::None))) {
                    warped_kept.push(format!("{} ({:.3e})", a.id, off));
                }
                Some(&NO_MESH)
            }
            Some(AreaMesh::Word(AreaMeshWord::None)) if n > 4 => {
                polygons_meshed.push(&a.id);
                None
            }
            m => m.as_ref(),
        };
        let (kind, h) = match mesh {
            Some(AreaMesh::Word(AreaMeshWord::None)) => (Kind::None, f64::INFINITY),
            _ if membrane_floor => (Kind::Panels, f64::INFINITY),
            Some(AreaMesh::Size { size }) => {
                if !(*size > 0.0 && size.is_finite()) {
                    return Err(invalid(format!("area {}: mesh size must be > 0", a.id)));
                }
                (
                    if wall || settings.floor == FloorMesh::Rectangular {
                        Kind::Rectangular
                    } else {
                        Kind::General
                    },
                    *size,
                )
            }
            None if wall && membrane => (Kind::Rectangular, f64::INFINITY),
            None if wall => (Kind::Rectangular, wall_size),
            None if settings.floor == FloorMesh::Rectangular => (Kind::Rectangular, floor_size),
            None => (Kind::General, floor_size),
        };
        hosts.push(Host {
            index: ai,
            plane,
            corners: c2.clone(),
            holes: Vec::new(),
            hole_areas: Vec::new(),
            h,
            kind,
            wall,
            cut_at_openings_only: wall && membrane && mesh.is_none(),
            rank: obj
                .area_sections
                .iter()
                .find(|s| s.id == a.section)
                .map_or(0, |s| match s.slab_kind() {
                    SlabType::Slab => 0,
                    SlabType::Drop => 1,
                    SlabType::Stiff => 2,
                }),
            size: geom::area(&c2).abs(),
            membrane_floor,
            one_way,
            hot: Vec::new(),
        });
    }
    if !polygons_meshed.is_empty() {
        let ids: Vec<String> = polygons_meshed.iter().map(|id| id.to_string()).collect();
        notes.push(format!(
            "Warning: areas {} are set to no auto mesh but have more than 4 corners, so they are meshed with the model's mesh settings; check their mesh options",
            ids.join(", ")
        ));
    }
    if !warped_kept.is_empty() {
        notes.push(format!(
            "Areas whose corners are not in one plane are not meshed: each stays one element, analysed on its mean plane (largest distance of a corner from it): {}",
            warped_kept.join(", ")
        ));
    }
    clock.lap("mesh: planes, shapes, openings");
    // An opening cuts every floor or wall it overlaps (an opening wins over everything).
    for (ai, a) in obj.areas.iter().enumerate().filter(|(_, a)| a.opening) {
        let p = corner3(a);
        let mut found = false;
        for hi in 0..hosts.len() {
            let h = &hosts[hi];
            if !p.iter().all(|q| h.plane.offset(*q) <= tol) {
                continue;
            }
            let p2: Vec<P2> = p.iter().map(|q| h.plane.to2(*q)).collect();
            if !polygons_overlap(&p2, &h.corners, tol) {
                continue;
            }
            if h.kind == Kind::None {
                return Err(invalid(format!(
                    "opening {}: area {} is not meshed, so it cannot have an opening",
                    a.id, obj.areas[h.index].id
                )));
            }
            hosts[hi].holes.push(p2);
            hosts[hi].hole_areas.push(ai);
            found = true;
        }
        if !found {
            return Err(invalid(format!(
                "opening {}: it does not lie in a floor or wall",
                a.id
            )));
        }
        opening_of[ai] = Some(0);
    }
    // A Membrane wall with no opening stays one element when it has 3 or 4 corners.
    for h in hosts.iter_mut() {
        if h.cut_at_openings_only && h.holes.is_empty() && h.corners.len() <= 4 {
            h.kind = Kind::None;
        }
        if h.kind == Kind::General && settings.refine {
            h.hot = grade::hot_spots(&h.corners, &h.holes, tol);
        }
    }
    clock.lap("mesh: openings cut areas");
    // Overlapping areas in one plane: only the winner of each overlap gets elements there.
    let coplanar = |a: &Host, b: &Host| {
        let c = axes::cross(a.plane.axes[2], b.plane.axes[2]);
        axes::dot(c, c).sqrt() < 1e-6 && a.plane.offset(b.plane.origin) <= tol
    };
    // Each area's box in space, grown by more than the distance at which the checks below still count two
    // coplanar areas as overlapping (the tolerance, and the slight tilt `coplanar` allows): only areas
    // whose boxes meet are compared.
    let host_boxes = Boxes::new(
        hosts
            .iter()
            .map(|h| {
                let b = boxes::around(h.corners.iter().map(|c| h.plane.to3(*c)), 0.0);
                boxes::around(b, 2.0 * tol + 1e-5 * geom::dist3(b[0], b[1]))
            })
            .collect(),
    );
    // An area that is not meshed cannot overlap a meshed one (it would have no part to give up). Two
    // areas that are both not meshed (each one element) are both analysed in full where they overlap:
    // stiffness, self-weight and the loads of an object over them, on each.
    let mut both_in_full: Vec<String> = Vec::new();
    for i in (0..hosts.len()).filter(|&i| hosts[i].kind == Kind::None) {
        for j in host_boxes.near(i) {
            if i == j || !coplanar(&hosts[i], &hosts[j]) {
                continue;
            }
            let bj: Vec<P2> = hosts[j]
                .corners
                .iter()
                .map(|q| hosts[i].plane.to2(hosts[j].plane.to3(*q)))
                .collect();
            if polygons_overlap(&hosts[i].corners, &bj, tol) {
                if hosts[j].kind == Kind::None {
                    if i < j {
                        both_in_full.push(format!(
                            "{}/{}",
                            obj.areas[hosts[i].index].id, obj.areas[hosts[j].index].id
                        ));
                    }
                    continue;
                }
                return Err(invalid(format!(
                    "area {}: it is not meshed, so it cannot overlap area {}",
                    obj.areas[hosts[i].index].id, obj.areas[hosts[j].index].id
                )));
            }
        }
    }
    if !both_in_full.is_empty() {
        notes.push(format!(
            "Areas that are not meshed overlap; both are analysed in full there (stiffness, self-weight and loads twice): {}",
            both_in_full.join(", ")
        ));
    }
    // Areas covering a point (coplanar, inside, not in one of their openings), and the winner there.
    let covering = |p3: Vec3, near: &Host| -> Vec<usize> {
        host_boxes
            .at(p3)
            .into_iter()
            .filter(|&k| {
                let h = &hosts[k];
                let c = axes::cross(h.plane.axes[2], near.plane.axes[2]);
                if axes::dot(c, c).sqrt() >= 1e-6 || h.plane.offset(p3) > tol {
                    return false;
                }
                let q = h.plane.to2(p3);
                geom::inside(q, &h.corners, 0.0)
                    && h.holes.iter().all(|hole| !geom::inside(q, hole, 0.0))
            })
            .collect()
    };
    // Whether area a lies inside area b (coplanar; its corners and edge midpoints inside b or on its
    // boundary, and smaller). An object inside another wins whatever the slab types.
    let within = |a: usize, b: usize| -> bool {
        if a == b || !coplanar(&hosts[a], &hosts[b]) || hosts[a].size >= hosts[b].size - tol * tol {
            return false;
        }
        let c = &hosts[a].corners;
        let to_b = |q: P2| hosts[b].plane.to2(hosts[a].plane.to3(q));
        (0..c.len()).all(|k| {
            let m = [
                0.5 * (c[k][0] + c[(k + 1) % c.len()][0]),
                0.5 * (c[k][1] + c[(k + 1) % c.len()][1]),
            ];
            geom::inside(to_b(c[k]), &hosts[b].corners, tol)
                && geom::inside(to_b(m), &hosts[b].corners, tol)
        })
    };
    // Who wins between two areas over a point (Phase 3b probe 2c and docs/elements/STIFF_AREA.md): the one
    // inside the other, else the higher slab type (Stiff, Drop, Slab), else the smaller area.
    let beats = |a: usize, b: usize| -> bool {
        let (ab, ba) = (within(a, b), within(b, a));
        if ab || ba {
            return ab;
        }
        hosts[a]
            .rank
            .cmp(&hosts[b].rank)
            .then(hosts[b].size.total_cmp(&hosts[a].size))
            .then(a.cmp(&b))
            .is_gt()
    };
    clock.lap("mesh: overlaps and winners");
    // Rectangular floors that overlap share one grid (`rect`): the grid of the largest of them.
    let mut host_of_area_index: Vec<Option<usize>> = vec![None; obj.areas.len()];
    for (hi, h) in hosts.iter().enumerate().rev() {
        host_of_area_index[h.index] = Some(hi);
    }
    let rect_floor = |h: &Host| h.kind == Kind::Rectangular && !h.wall;
    let mut group: Vec<usize> = (0..hosts.len()).collect();
    fn root(group: &mut [usize], mut i: usize) -> usize {
        while group[i] != i {
            group[i] = group[group[i]];
            i = group[i];
        }
        i
    }
    for i in (0..hosts.len()).filter(|&i| rect_floor(&hosts[i])) {
        for j in host_boxes.near(i).into_iter().filter(|&j| j > i) {
            if !rect_floor(&hosts[j]) || !coplanar(&hosts[i], &hosts[j]) {
                continue;
            }
            let bj: Vec<P2> = hosts[j]
                .corners
                .iter()
                .map(|q| hosts[i].plane.to2(hosts[j].plane.to3(*q)))
                .collect();
            if polygons_overlap(&hosts[i].corners, &bj, tol) {
                let (ri, rj) = (root(&mut group, i), root(&mut group, j));
                group[ri.max(rj)] = ri.min(rj);
            }
        }
    }
    let roots: Vec<usize> = (0..hosts.len()).map(|i| root(&mut group, i)).collect();
    // The largest area of each group (the first of equals).
    let mut largest: Vec<Option<usize>> = vec![None; hosts.len()];
    for k in 0..hosts.len() {
        let best = &mut largest[roots[k]];
        if best.is_none_or(|b| hosts[k].size > hosts[b].size) {
            *best = Some(k);
        }
    }
    let base_of: Vec<usize> = roots.iter().map(|&r| largest[r].unwrap()).collect();
    let winner = |cover: &[usize]| -> Option<usize> {
        cover
            .iter()
            .copied()
            .reduce(|w, c| if beats(c, w) { c } else { w })
    };

    clock.lap("mesh: rectangular grids");
    // Segments and key points of each area, in its plane (a Membrane floor's panels: its edges,
    // openings, beams and other areas' edges only).
    let in_plane = |h: &Host, p: Vec3| h.plane.offset(p) <= tol;
    // The lines and points that can reach an area: in space, inside the box around its box in its plane
    // (grown by twice the tolerance, as `near_region` keeps them) or, for a rectangular floor, its group's
    // boxes (its grid uses the group's joints and edges). The box grows by the tolerance off the plane
    // and the slight tilt `coplanar` allows.
    let plane_box = |h: &Host| -> boxes::Box3 {
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for c in &h.corners {
            for d in 0..2 {
                lo[d] = lo[d].min(c[d] - 2.0 * tol);
                hi[d] = hi[d].max(c[d] + 2.0 * tol);
            }
        }
        let b = boxes::around(
            [
                [lo[0], lo[1]],
                [hi[0], lo[1]],
                [hi[0], hi[1]],
                [lo[0], hi[1]],
            ]
            .map(|q| h.plane.to3(q)),
            0.0,
        );
        boxes::around(b, 2.0 * tol + 1e-5 * geom::dist3(b[0], b[1]))
    };
    let mut group_members: HashMap<usize, Vec<usize>> = HashMap::new();
    for k in 0..hosts.len() {
        group_members.entry(base_of[k]).or_default().push(k);
    }
    let reach = |h: &Host| -> boxes::Box3 {
        if h.kind == Kind::Rectangular && !h.wall {
            let me = host_of_area_index[h.index].unwrap();
            let members = &group_members[&base_of[me]];
            boxes::around(members.iter().flat_map(|&k| plane_box(&hosts[k])), 0.0)
        } else {
            plane_box(h)
        }
    };
    let frame_boxes = Boxes::new(
        obj.frames
            .iter()
            .map(|f| boxes::around(f.nodes.iter().map(|id| xyz[node_index[id]]), 0.0))
            .collect(),
    );
    let area_boxes = Boxes::new(
        obj.areas
            .iter()
            .map(|a| boxes::around(corner3(a), 0.0))
            .collect(),
    );
    // Where a frame crosses an area's edge (in the area's plane or not, within the join distance), the
    // frame is split and the joint, on the frame, is tied to the edge; meshed areas pass through it.
    // Null objects and openings split nothing.
    let join = obj.units.frame_join().max(tol);
    let mut edge_cross: Vec<(usize, Vec3)> = Vec::new();
    for (fi, f) in obj.frames.iter().enumerate().filter(|(_, f)| !f.is_null()) {
        let (a, b) = (xyz[node_index[&f.nodes[0]]], xyz[node_index[&f.nodes[1]]]);
        let reach = boxes::around([a, b], 2.0 * join);
        for c in area_boxes
            .meeting(&reach)
            .into_iter()
            .map(|ai| &obj.areas[ai])
            .filter(|c| !c.opening && !c.is_null())
        {
            let p = corner3(c);
            for k in 0..p.len() {
                if let Some(q) = crossing3(a, b, p[k], p[(k + 1) % p.len()], tol, join) {
                    edge_cross.push((fi, q));
                }
            }
        }
    }
    let edge_cross_boxes = Boxes::new(edge_cross.iter().map(|(_, q)| [*q; 2]).collect());
    let active_list: Vec<usize> = {
        let mut v: Vec<usize> = active.iter().copied().collect();
        v.sort_unstable();
        v
    };
    let active_boxes = Boxes::new(active_list.iter().map(|&j| [xyz[j]; 2]).collect());
    let build_lines = |h: &Host, extra: &[Vec3]| -> (Vec<[P2; 2]>, Vec<P2>) {
        let mut segs: Vec<[P2; 2]> = Vec::new();
        let n = h.corners.len();
        for k in 0..n {
            segs.push([h.corners[k], h.corners[(k + 1) % n]]);
        }
        for hole in &h.holes {
            for k in 0..hole.len() {
                segs.push([hole[k], hole[(k + 1) % hole.len()]]);
            }
        }
        if h.kind == Kind::None {
            return (segs, Vec::new());
        }
        let mut keys: Vec<P2> = Vec::new();
        let near = reach(h);
        for f in frame_boxes
            .meeting(&near)
            .into_iter()
            .map(|i| &obj.frames[i])
            .filter(|f| meshing(f))
        {
            let (a, b) = (xyz[node_index[&f.nodes[0]]], xyz[node_index[&f.nodes[1]]]);
            if in_plane(h, a) && in_plane(h, b) {
                segs.push([h.plane.to2(a), h.plane.to2(b)]);
            }
        }
        for (bi, b) in area_boxes
            .meeting(&near)
            .into_iter()
            .map(|bi| (bi, &obj.areas[bi]))
        {
            if bi == h.index || b.opening || b.is_null() || h.hole_areas.contains(&bi) {
                continue;
            }
            let p = corner3(b);
            for k in 0..p.len() {
                let (a1, b1) = (p[k], p[(k + 1) % p.len()]);
                if in_plane(h, a1) && in_plane(h, b1) {
                    segs.push([h.plane.to2(a1), h.plane.to2(b1)]);
                }
            }
        }
        if h.kind == Kind::Panels {
            return (segs, Vec::new());
        }
        let inside = |q: P2| geom::inside(q, &h.corners, tol);
        let active_near: Vec<usize> = active_boxes
            .meeting(&near)
            .into_iter()
            .map(|k| active_list[k])
            .collect();
        for &j in &active_near {
            if in_plane(h, xyz[j]) && inside(h.plane.to2(xyz[j])) {
                keys.push(h.plane.to2(xyz[j]));
            }
        }
        for p in extra.iter().chain(
            edge_cross_boxes
                .meeting(&near)
                .into_iter()
                .filter(|&k| meshing(&obj.frames[edge_cross[k].0]))
                .map(|k| &edge_cross[k].1),
        ) {
            if in_plane(h, *p) && inside(h.plane.to2(*p)) {
                keys.push(h.plane.to2(*p));
            }
        }
        if h.kind == Kind::Rectangular && !h.wall {
            // Rectangular floor mesh with localized meshing (`rect`), on the grid of the
            // largest floor of the group (in its plane coordinates).
            let me = host_of_area_index[h.index].unwrap();
            let base = &hosts[base_of[me]];
            let members = &group_members[&base_of[me]];
            let to_base = |p: Vec3| base.plane.to2(p);
            let on_group = |p: Vec3| {
                members
                    .iter()
                    .any(|&k| geom::inside(hosts[k].plane.to2(p), &hosts[k].corners, tol))
            };
            let coplanar_area = |b: usize| {
                obj.areas[b]
                    .nodes
                    .iter()
                    .all(|id| in_plane(h, xyz[node_index[id]]))
            };
            let (mut lo2, mut hi2) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for &k in members {
                for c in &hosts[k].corners {
                    let q = to_base(hosts[k].plane.to3(*c));
                    for d in 0..2 {
                        lo2[d] = lo2[d].min(q[d]);
                        hi2[d] = hi2[d].max(q[d]);
                    }
                }
            }
            // Joint objects: supports, springs, links, loaded joints, and joints where a column or a wall
            // meets the floor; last created first.
            let joints: Vec<usize> = joint_boxes
                .meeting(&near)
                .into_iter()
                .rev()
                .filter(|&j| in_plane(h, xyz[j]) && on_group(xyz[j]))
                .filter(|&j| {
                    held.contains(&j)
                        || frame_ends[j].iter().any(|&o| !in_plane(h, xyz[o]))
                        || corner_of[j]
                            .iter()
                            .any(|&b| !obj.areas[b].opening && !coplanar_area(b))
                })
                .collect();
            let joint_pts: Vec<P2> = joints.iter().map(|&j| to_base(xyz[j])).collect();
            // The other floors of the group and the openings in them.
            let mut objects: Vec<rect::Object> = members
                .iter()
                .filter(|&&k| k != base_of[me])
                .map(|&k| rect::Object {
                    order: hosts[k].index,
                    rank: hosts[k].rank,
                    opening: false,
                    corners: corner3(&obj.areas[hosts[k].index])
                        .into_iter()
                        .map(to_base)
                        .collect(),
                })
                .collect();
            let mut openings: Vec<usize> = members
                .iter()
                .flat_map(|&k| hosts[k].hole_areas.iter().copied())
                .collect();
            openings.sort_unstable();
            openings.dedup();
            objects.extend(openings.into_iter().map(|b| rect::Object {
                order: b,
                rank: 0,
                opening: true,
                corners: corner3(&obj.areas[b]).into_iter().map(to_base).collect(),
            }));
            // Beam ends: frame joints that are not joint objects.
            let mut ends: Vec<usize> = active_near
                .iter()
                .copied()
                .filter(|j| !joints.contains(j) && in_plane(h, xyz[*j]) && on_group(xyz[*j]))
                .collect();
            ends.sort_unstable();
            let points: Vec<P2> = ends.iter().map(|&j| to_base(xyz[j])).collect();
            let shared: Vec<P2> = extra
                .iter()
                .filter(|p| in_plane(h, **p) && on_group(**p))
                .map(|p| to_base(*p))
                .collect();
            let edges: Vec<[P2; 2]> = segs
                .iter()
                .map(|s| s.map(|q| to_base(h.plane.to3(q))))
                .collect();
            let lines = rect::lines(&rect::Input {
                corners: &base.corners,
                extent: [lo2, hi2],
                size: base.h,
                joints: &joint_pts,
                objects: &objects,
                points: &points,
                shared: &shared,
                edges: &edges,
                tol,
            });
            segs.extend(
                lines
                    .into_iter()
                    .map(|s| s.map(|q| h.plane.to2(base.plane.to3(q)))),
            );
        } else if h.kind == Kind::Rectangular {
            // Walls (auto rectangular mesh): lines along local 1 and 2 through every key point, across
            // the whole area.
            let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for c in &h.corners {
                for d in 0..2 {
                    lo[d] = lo[d].min(c[d]);
                    hi[d] = hi[d].max(c[d]);
                }
            }
            let pad = 0.01 * (hi[0] - lo[0] + hi[1] - lo[1]);
            let mut through: Vec<P2> = h.corners.clone();
            through.extend(h.holes.iter().flatten().copied());
            if !h.cut_at_openings_only {
                through.extend(segs.iter().flatten().copied().filter(|q| inside(*q)));
                through.extend(keys.iter().copied());
            }
            let mut us: Vec<f64> = through.iter().map(|q| q[0]).collect();
            let mut vs: Vec<f64> = through.iter().map(|q| q[1]).collect();
            for list in [&mut us, &mut vs] {
                list.sort_by(|a, b| a.total_cmp(b));
                list.dedup_by(|a, b| (*a - *b).abs() <= tol);
            }
            for u in us {
                segs.push([[u, lo[1] - pad], [u, hi[1] + pad]]);
            }
            for v in vs {
                segs.push([[lo[0] - pad, v], [hi[0] + pad, v]]);
            }
        }
        (segs, keys)
    };

    // Only the lines and points that reach an area's box (grown by twice the tolerance) can cut its faces:
    // the others (e.g. the edges of the walls stacked in the same plane on other stories) are left out of
    // its arrangement.
    let near_region = |h: &Host, segs: &mut Vec<[P2; 2]>, keys: &mut Vec<P2>| {
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for c in &h.corners {
            for d in 0..2 {
                lo[d] = lo[d].min(c[d] - 2.0 * tol);
                hi[d] = hi[d].max(c[d] + 2.0 * tol);
            }
        }
        let inside = |q: &P2| (0..2).all(|d| lo[d] <= q[d] && q[d] <= hi[d]);
        segs.retain(|[a, b]| (0..2).all(|d| a[d].min(b[d]) <= hi[d] && a[d].max(b[d]) >= lo[d]));
        keys.retain(inside);
    };

    clock.lap("mesh: segments and key points");
    // Pass 1: arrangements; their vertices are shared with the other areas in pass 2.
    let mut shared: Vec<Vec3> = Vec::new();
    for h in hosts.iter().filter(|h| h.kind != Kind::None) {
        let (mut segs, mut keys) = build_lines(h, &[]);
        near_region(h, &mut segs, &mut keys);
        let arr = arrange::arrange(&segs, &keys, &h.corners, &h.holes, tol);
        let used: HashSet<usize> = arr
            .faces
            .iter()
            .flat_map(|f| f.outer.iter().chain(f.holes.iter().flatten()))
            .copied()
            .collect();
        shared.extend(used.iter().map(|&v| h.plane.to3(arr.verts[v])));
    }
    {
        let mut table = Points::new(tol);
        shared.retain(|p| {
            let n = table.len();
            table.add(*p) == n
        });
    }

    clock.lap("mesh: pass 1 arrangements");
    // Pass 2: final faces, with every shared point on an area's lines.
    struct FaceWork {
        host: usize,
        outer: Vec<usize>,
        holes: Vec<Vec<usize>>,
        /// Areas covering the face (their loads act on its elements).
        cover: Vec<usize>,
    }
    let mut overlaps: BTreeMap<(usize, usize), bool> = BTreeMap::new();
    let mut faces: Vec<FaceWork> = Vec::new();
    // Membrane floor elements (host, corner point ids), for their load transfer, and all elements.
    let mut panel_loops: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut elems: Vec<(usize, Vec<usize>, Vec<usize>)> = Vec::new();
    let mut seg_h: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    // Edges divided by a host's size field (grading): used only by irregular faces of that host.
    let mut seg_graded: BTreeMap<(usize, usize), Option<usize>> = BTreeMap::new();
    // The shared points near each area (in its box), in their order; a rectangular floor's grid takes
    // points from its whole group, so it gets those near the group.
    let shared_boxes = Boxes::new(shared.iter().map(|p| [*p; 2]).collect());
    for (hi, h) in hosts.iter().enumerate() {
        if h.kind == Kind::None {
            continue;
        }
        let extra: Vec<Vec3> = {
            let near = if h.kind == Kind::Rectangular && !h.wall {
                reach(h)
            } else {
                let b = boxes::around(h.corners.iter().map(|c| h.plane.to3(*c)), 0.0);
                boxes::around(b, 4.0 * tol + 1e-5 * geom::dist3(b[0], b[1]))
            };
            shared_boxes
                .meeting(&near)
                .into_iter()
                .map(|k| shared[k])
                .collect()
        };
        let (mut segs, mut keys) = build_lines(h, &extra);
        if h.kind != Kind::Panels {
            keys.extend(
                extra
                    .iter()
                    .filter(|p| in_plane(h, **p))
                    .map(|p| h.plane.to2(*p)),
            );
        }
        near_region(h, &mut segs, &mut keys);
        let mut arr = arrange::arrange(&segs, &keys, &h.corners, &h.holes, tol);
        if h.kind == Kind::Panels {
            // Cut until every panel is convex (the rule at re-entrant corners).
            for _ in 0..8 {
                let cuts = panels::convex_cuts(&arr, tol);
                if cuts.is_empty() {
                    break;
                }
                segs.extend(cuts);
                arr = arrange::arrange(&segs, &keys, &h.corners, &h.holes, tol);
            }
        }
        if arr.faces.is_empty() {
            return Err(invalid(format!(
                "area {}: could not split it into faces",
                obj.areas[h.index].id
            )));
        }
        let gid: Vec<usize> = arr
            .verts
            .iter()
            .map(|v| points.add(h.plane.to3(*v)))
            .collect();
        for f in &arr.faces {
            let mut cover = covering(h.plane.to3(f.inside), h);
            if !cover.contains(&hi) {
                cover.push(hi);
            }
            let win = winner(&cover).unwrap_or(hi);
            for &c in &cover {
                if c != win {
                    overlaps.insert((c, win), hosts[win].rank > hosts[c].rank);
                }
            }
            if win != hi {
                continue;
            }
            if h.kind == Kind::Panels {
                if !panels::is_convex(&arr, &f.outer, f.holes.len(), tol) {
                    return Err(invalid(format!(
                        "area {}: could not cut this Membrane floor into convex panels",
                        obj.areas[h.index].id
                    )));
                }
                let pts: Vec<P2> = f.outer.iter().map(|&v| arr.verts[v]).collect();
                let ids: Vec<usize> = panels::corners(&pts, tol)
                    .into_iter()
                    .map(|k| gid[f.outer[k]])
                    .collect();
                // One element per panel (a panel with more corners: a fan of quads).
                let mut k = 1;
                while k + 1 < ids.len() {
                    let e: Vec<usize> = if k + 2 < ids.len() {
                        vec![ids[0], ids[k], ids[k + 1], ids[k + 2]]
                    } else {
                        vec![ids[0], ids[k], ids[k + 1]]
                    };
                    panel_loops.push((hi, e.clone()));
                    elems.push((hi, e, cover.clone()));
                    k += 2;
                }
                continue;
            }
            let walk = |c: &[usize]| c.iter().map(|&v| gid[v]).collect::<Vec<usize>>();
            let outer = walk(&f.outer);
            let holes: Vec<Vec<usize>> = f.holes.iter().map(|c| walk(c)).collect();
            // An irregular face (not four-sided and convex, or with holes) gets the quad-dominant mesh;
            // with grading its edges follow the size field.
            let graded = !h.hot.is_empty() && {
                let loop2: Vec<P2> = f.outer.iter().map(|&v| arr.verts[v]).collect();
                let c = mapped::corners(&loop2, 1e-6);
                let n = loop2.len();
                let convex = c.iter().all(|&k| {
                    let a = geom::sub(loop2[k], loop2[(k + n - 1) % n]);
                    let b = geom::sub(loop2[(k + 1) % n], loop2[k]);
                    geom::cross(a, b) > 0.0
                });
                !f.holes.is_empty() || c.len() != 4 || !convex
            };
            for c in std::iter::once(&outer).chain(holes.iter()) {
                for k in 0..c.len() {
                    let (a, b) = (c[k], c[(k + 1) % c.len()]);
                    if a != b {
                        let key = (a.min(b), a.max(b));
                        let e = seg_h.entry(key).or_insert(f64::INFINITY);
                        *e = e.min(h.h);
                        let g = seg_graded.entry(key).or_insert(graded.then_some(hi));
                        if !graded || *g != Some(hi) {
                            *g = None;
                        }
                    }
                }
            }
            faces.push(FaceWork {
                host: hi,
                outer,
                holes,
                cover,
            });
        }
    }
    clock.lap("mesh: pass 2 faces");
    // Edges shared by objects: divided once, into ceil(length / smallest size) equal pieces.
    let mut seg_pts: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (&(a, b), &h) in &seg_h {
        let (pa, pb) = (points.xyz[a], points.xyz[b]);
        let len = geom::dist3(pa, pb);
        let ts: Vec<f64> = seg_graded[&(a, b)]
            .and_then(|hi| {
                let host = &hosts[hi];
                let field = grade::Field { h, hot: &host.hot };
                field.divide(host.plane.to2(pa), host.plane.to2(pb))
            })
            .unwrap_or_else(|| {
                let n = ((len / h) - 1e-9).ceil().max(1.0) as usize;
                (1..n).map(|s| s as f64 / n as f64).collect()
            });
        let mut ids = vec![a];
        for t in ts {
            ids.push(points.add(axes::add(pa, axes::scale(axes::sub(pb, pa), t))));
        }
        ids.push(b);
        seg_pts.insert((a, b), ids);
    }
    let chain = |a: usize, b: usize| -> Vec<usize> {
        if a < b {
            seg_pts[&(a, b)].clone()
        } else {
            let mut v = seg_pts[&(b, a)].clone();
            v.reverse();
            v
        }
    };
    let expand = |c: &[usize]| -> Vec<usize> {
        let mut out = Vec::new();
        for k in 0..c.len() {
            let (a, b) = (c[k], c[(k + 1) % c.len()]);
            if a == b {
                continue;
            }
            let ch = chain(a, b);
            out.extend_from_slice(&ch[..ch.len() - 1]);
        }
        out
    };

    clock.lap("mesh: shared edges divided");
    // Elements of every area: (host, corner point ids counterclockwise in its plane).
    for (hi, h) in hosts.iter().enumerate() {
        if h.kind == Kind::None {
            let a = &obj.areas[h.index];
            let ids: Vec<usize> = a
                .nodes
                .iter()
                .map(|id| points.find(xyz[node_index[id]]).unwrap())
                .collect();
            if h.membrane_floor {
                panel_loops.push((hi, ids.clone()));
            }
            elems.push((hi, ids, vec![hi]));
        }
    }
    for f in &faces {
        let h = &hosts[f.host];
        let outer = expand(&f.outer);
        let holes: Vec<Vec<usize>> = f.holes.iter().map(|c| expand(c)).collect();
        let to2 = |i: usize| h.plane.to2(points.xyz[i]);
        let outer2: Vec<P2> = outer.iter().map(|&i| to2(i)).collect();
        let holes2: Vec<Vec<P2>> = holes
            .iter()
            .map(|c| c.iter().map(|&i| to2(i)).collect())
            .collect();
        let strictly_inside = |q: P2| {
            geom::inside(q, &outer2, -tol)
                && (0..outer2.len())
                    .all(|k| geom::seg_dist(q, outer2[k], outer2[(k + 1) % outer2.len()]) > tol)
                && holes2.iter().all(|hh| !geom::inside(q, hh, tol))
        };
        // Joints with something attached (a column, a support) inside the face must be mesh joints.
        let face_box = boxes::around(outer.iter().map(|&i| points.xyz[i]), 2.0 * tol);
        let mut inner: Vec<usize> = active_boxes
            .meeting(&face_box)
            .into_iter()
            .map(|k| active_list[k])
            .filter(|&j| in_plane(h, xyz[j]) && strictly_inside(h.plane.to2(xyz[j])))
            .collect();
        inner.sort_unstable();
        let distinct = outer.iter().collect::<HashSet<_>>().len() == outer.len();
        // Rectangular floors: every cell is one element; joints of finer cells next to it that lie on its
        // sides hang on them (edge constraints).
        if h.kind == Kind::Rectangular
            && !h.wall
            && holes.is_empty()
            && distinct
            && inner.is_empty()
        {
            let loop2: Vec<P2> = outer.iter().map(|&i| to2(i)).collect();
            let c = mapped::corners(&loop2, 1e-6);
            let n = loop2.len();
            let convex = c.iter().all(|&k| {
                let a = geom::sub(loop2[k], loop2[(k + n - 1) % n]);
                let b = geom::sub(loop2[(k + 1) % n], loop2[k]);
                geom::cross(a, b) > 0.0
            });
            if (c.len() == 3 || c.len() == 4) && convex {
                elems.push((
                    f.host,
                    c.iter().map(|&k| outer[k]).collect(),
                    f.cover.clone(),
                ));
                continue;
            }
            // A cell with a corner cut off by a skew line: two quads.
            let corners: Vec<P2> = c.iter().map(|&k| loop2[k]).collect();
            if let Some(parts) = convex
                .then(|| rect::split_cut_corner(&corners, tol))
                .flatten()
            {
                for part in parts {
                    let ids: Vec<usize> = part
                        .iter()
                        .map(
                            |q| match corners.iter().position(|r| geom::dist(*q, *r) <= tol) {
                                Some(k) => outer[c[k]],
                                None => points.add(h.plane.to3(*q)),
                            },
                        )
                        .collect();
                    elems.push((f.host, ids, f.cover.clone()));
                }
                continue;
            }
        }
        if holes.is_empty() && distinct && inner.is_empty() {
            let loop2: Vec<P2> = outer.iter().map(|&i| to2(i)).collect();
            if let Some((pts2, quads)) = mapped::mapped(&loop2) {
                let ids: Vec<usize> = (0..pts2.len())
                    .map(|k| {
                        if k < outer.len() {
                            outer[k]
                        } else {
                            points.add(h.plane.to3(pts2[k]))
                        }
                    })
                    .collect();
                for q in quads {
                    elems.push((f.host, q.iter().map(|&k| ids[k]).collect(), f.cover.clone()));
                }
                continue;
            }
        }
        // Quad-dominant mesh: the face's points, edges and the active joints strictly inside it.
        let mut local: Vec<usize> = Vec::new();
        let mut at: HashMap<usize, usize> = HashMap::new();
        let mut idx = |g: usize, local: &mut Vec<usize>| -> usize {
            *at.entry(g).or_insert_with(|| {
                local.push(g);
                local.len() - 1
            })
        };
        let mut edges: Vec<[usize; 2]> = Vec::new();
        for c in std::iter::once(&outer).chain(holes.iter()) {
            for k in 0..c.len() {
                let (a, b) = (idx(c[k], &mut local), idx(c[(k + 1) % c.len()], &mut local));
                if a != b
                    && !edges
                        .iter()
                        .any(|e| (e[0] == a && e[1] == b) || (e[0] == b && e[1] == a))
                {
                    edges.push([a, b]);
                }
            }
        }
        for j in inner {
            let g = points.find(xyz[j]).unwrap();
            idx(g, &mut local);
        }
        let pts2: Vec<P2> = local.iter().map(|&g| to2(g)).collect();
        let out = tri::mesh(&tri::Input {
            pts: pts2,
            fixed_edges: edges,
            outer: &outer2,
            holes: &holes2,
            h: h.h,
            origin: h.corners[0],
            hot: &h.hot,
        })
        .map_err(|e| invalid(format!("area {}: {e}", obj.areas[h.index].id)))?;
        let ids: Vec<usize> = (0..out.pts.len())
            .map(|k| {
                if k < local.len() {
                    local[k]
                } else {
                    points.add(h.plane.to3(out.pts[k]))
                }
            })
            .collect();
        for e in out.elems {
            elems.push((f.host, e.iter().map(|&k| ids[k]).collect(), f.cover.clone()));
        }
    }

    clock.lap("mesh: elements of every area");
    // Points where frames cross, passing within the join distance of each other: both get split there, at
    // the point on the frame that comes first in the model. Null frames have no elements.
    let mut cross_ids: Vec<usize> = Vec::new();
    // Points a frame is split at although they may lie up to the join distance off its line.
    let mut forced: HashMap<&Id, Vec<usize>> = HashMap::new();
    let solid: Vec<&Frame> = obj.frames.iter().filter(|f| !f.is_null()).collect();
    // Only frames whose boxes, grown by the join distance, meet can cross.
    let solid_boxes = Boxes::new(
        solid
            .iter()
            .map(|f| boxes::around(f.nodes.iter().map(|id| xyz[node_index[id]]), join))
            .collect(),
    );
    for (i, f) in solid.iter().enumerate() {
        for g in solid_boxes
            .near(i)
            .into_iter()
            .filter(|&j| j > i)
            .map(|j| solid[j])
        {
            let (a, b) = (xyz[node_index[&f.nodes[0]]], xyz[node_index[&f.nodes[1]]]);
            let (c, d) = (xyz[node_index[&g.nodes[0]]], xyz[node_index[&g.nodes[1]]]);
            if let Some(p) = crossing3(a, b, c, d, tol, join) {
                let k = points.add(p);
                cross_ids.push(k);
                forced.entry(&g.id).or_default().push(k);
            }
        }
    }
    // A frame left out of the floor mesh (*Include in mesh* off) is split where it crosses element edges;
    // those joints hang on the edges (edge constraints; probe findings 2h).
    for f in solid.iter().filter(|f| !meshing(f)) {
        let (a, b) = (xyz[node_index[&f.nodes[0]]], xyz[node_index[&f.nodes[1]]]);
        let len = geom::dist3(a, b);
        for (hi, e, _) in &elems {
            let h = &hosts[*hi];
            if !(in_plane(h, a) && in_plane(h, b)) {
                continue;
            }
            let (a2, b2) = (h.plane.to2(a), h.plane.to2(b));
            for k in 0..e.len() {
                let (p, q) = (points.xyz[e[k]], points.xyz[e[(k + 1) % e.len()]]);
                if let Some((t, _)) = geom::seg_cross(a2, b2, h.plane.to2(p), h.plane.to2(q), tol) {
                    if t * len > tol && (1.0 - t) * len > tol {
                        cross_ids.push(points.add(axes::add(a, axes::scale(axes::sub(b, a), t))));
                    }
                }
            }
        }
    }

    for (fi, q) in &edge_cross {
        let k = points.add(*q);
        cross_ids.push(k);
        forced.entry(&obj.frames[*fi].id).or_default().push(k);
    }
    clock.lap("mesh: frame crossings");
    // Analysis joints: object joints keep their ids, mesh joints become ~1, ~2…
    let mut name: Vec<Option<Id>> = points
        .joint
        .iter()
        .map(|j| j.map(|j| obj.nodes[j].id.clone()))
        .collect();
    let mut mesh_joints: Vec<MeshJoint> = Vec::new();
    let mut am = obj.clone();
    am.mesh = None;
    let taken: HashSet<Id> = obj.nodes.iter().map(|n| n.id.clone()).collect();
    let mut next_joint = 1usize;
    let mut name_of = |i: usize,
                       name: &mut Vec<Option<Id>>,
                       am: &mut Model,
                       mesh_joints: &mut Vec<MeshJoint>|
     -> Id {
        if let Some(id) = &name[i] {
            return id.clone();
        }
        let id = loop {
            let id = Id(format!("~{next_joint}"));
            next_joint += 1;
            if !taken.contains(&id) {
                break id;
            }
        };
        let p = points.xyz[i];
        am.nodes.push(osab_solver::model::Node {
            id: id.clone(),
            x: p[0],
            y: p[1],
            z: p[2],
            in_mesh: None,
            diaphragm: None,
        });
        mesh_joints.push(MeshJoint {
            id: id.clone(),
            x: p[0],
            y: p[1],
            z: p[2],
        });
        name[i] = Some(id.clone());
        id
    };

    clock.lap("mesh: analysis joints");
    // Shell elements.
    let mut parent: HashMap<Id, Id> = HashMap::new();
    let mut by_host: Vec<Vec<(Vec<usize>, Vec<usize>)>> = vec![Vec::new(); hosts.len()];
    for (hi, e, cover) in elems {
        let h = &hosts[hi];
        let mut e = e;
        let p2: Vec<P2> = e.iter().map(|&i| h.plane.to2(points.xyz[i])).collect();
        if geom::area(&p2) < 0.0 {
            e.reverse();
        }
        by_host[hi].push((e, cover));
    }
    am.areas.clear();
    let object_ids: HashSet<Id> = obj
        .areas
        .iter()
        .map(|a| a.id.clone())
        .chain(obj.frames.iter().map(|f| f.id.clone()))
        .chain(obj.links.iter().map(|l| l.id.clone()))
        .collect();
    let mut shell_info: Vec<MeshElement> = Vec::new();
    let mut element_of_area: HashMap<Id, Vec<Id>> = HashMap::new();
    let mut used_ids: HashSet<Id> = HashSet::new();
    let fresh = |base: &Id, k: usize, used: &mut HashSet<Id>| -> Id {
        let mut s = 0;
        loop {
            let id = if s == 0 {
                Id(format!("{base}-{k}"))
            } else {
                Id(format!("{base}-{k}.{s}"))
            };
            if !object_ids.contains(&id) && used.insert(id.clone()) {
                return id;
            }
            s += 1;
        }
    };
    for (hi, list) in by_host.iter().enumerate() {
        let a = &obj.areas[hosts[hi].index];
        let single = list.len() == 1;
        for (k, (e, cover)) in list.iter().enumerate() {
            let id = if single {
                used_ids.insert(a.id.clone());
                a.id.clone()
            } else {
                fresh(&a.id, k + 1, &mut used_ids)
            };
            let nodes: Vec<Id> = e
                .iter()
                .map(|&i| name_of(i, &mut name, &mut am, &mut mesh_joints))
                .collect();
            am.areas.push(Area {
                id: id.clone(),
                nodes: nodes.clone(),
                section: a.section.clone(),
                angle: a.angle,
                modifiers: a.modifiers,
                insertion: a.insertion,
                area_spring: a.area_spring.clone(),
                pier: a.pier.clone(),
                spandrel: a.spandrel.clone(),
                diaphragm: a.diaphragm.clone(),
                opening: false,
                mesh: None,
                edge_restraints: None,
                user_mesh: None,
            });
            parent.insert(id.clone(), a.id.clone());
            // Every area covering the element loads it (overlapping areas' loads add up).
            for &c in cover {
                element_of_area
                    .entry(obj.areas[hosts[c].index].id.clone())
                    .or_default()
                    .push(id.clone());
            }
            if !single {
                shell_info.push(MeshElement {
                    id,
                    parent: a.id.clone(),
                    nodes,
                });
            }
        }
    }

    clock.lap("mesh: shell elements");
    // Frames split at every connected joint along them (mesh joints, other frames' joints) and where
    // frames cross.
    let connected: Vec<usize> = (0..points.len())
        .filter(|&i| name[i].is_some() || points.joint[i].is_some_and(|j| active.contains(&j)))
        .collect();
    am.frames.clear();
    let mut pieces: Vec<(Id, Vec<Id>)> = Vec::new();
    let mut frame_info: Vec<MeshElement> = Vec::new();
    let mut piece_of: HashMap<Id, Vec<(Id, f64, f64)>> = HashMap::new();
    // End length offsets belong to the object (automatic ones come from the objects meeting at
    // its ends): every piece gets the part of them that lies on it, as given lengths.
    let object_offsets: HashMap<&Id, [f64; 2]> = obj
        .frames
        .iter()
        .zip(obj.end_offsets())
        .map(|(f, o)| (&f.id, o))
        .collect();
    // A bending release stays on the object's first or last piece (`hinged` per end): the offset on
    // that piece reaches at most 99 % of it, so the hinge sits inside the piece and the piece keeps a
    // flexible part past it.
    let piece_offsets = |f: &Frame, s0: f64, s1: f64, len: f64, hinged: [bool; 2]| {
        let [oi, oj] = object_offsets[&f.id];
        let cap = |o: f64, h: bool| if h { o.min(0.99 * (s1 - s0)) } else { o };
        let i = cap((oi - s0).clamp(0.0, s1 - s0), hinged[0]);
        let j = cap((s1 - (len - oj)).clamp(0.0, s1 - s0), hinged[1]);
        (i > 0.0 || j > 0.0).then_some(EndOffsets::Lengths { i, j })
    };
    let bending = |r: &[Release]| {
        r.iter()
            .any(|x| matches!(x, Release::V2 | Release::V3 | Release::M2 | Release::M3))
    };
    let mut split_offsets: HashMap<Id, [f64; 2]> = HashMap::new();
    // The points a frame may pass through, and their boxes for finding the ones near a frame.
    let stops_at: Vec<usize> = connected.iter().chain(cross_ids.iter()).copied().collect();
    let stop_boxes = Boxes::new(stops_at.iter().map(|&i| [points.xyz[i]; 2]).collect());
    for f in solid.iter().copied() {
        let (ia, ib) = (
            points.find(xyz[node_index[&f.nodes[0]]]).unwrap(),
            points.find(xyz[node_index[&f.nodes[1]]]).unwrap(),
        );
        let (a, b) = (points.xyz[ia], points.xyz[ib]);
        let len = geom::dist3(a, b);
        let mut on: Vec<(f64, usize)> = stop_boxes
            .meeting(&boxes::around([a, b], 2.0 * tol + 1e-9 * len))
            .into_iter()
            .map(|k| stops_at[k])
            .filter(|&i| i != ia && i != ib)
            .filter_map(|i| geom::on_segment3(points.xyz[i], a, b, tol).map(|t| (t, i)))
            .chain(forced.get(&f.id).into_iter().flatten().map(|&i| {
                let d = axes::sub(b, a);
                (axes::dot(axes::sub(points.xyz[i], a), d) / (len * len), i)
            }))
            .filter(|&(t, _)| t * len > tol && (1.0 - t) * len > tol)
            .collect();
        on.sort_by(|x, y| x.0.total_cmp(&y.0));
        on.dedup_by(|x, y| x.1 == y.1);
        if on.is_empty() {
            am.frames.push(Frame {
                end_offsets: piece_offsets(f, 0.0, len, len, [false; 2]),
                ..f.clone()
            });
            piece_of.insert(f.id.clone(), vec![(f.id.clone(), 0.0, len)]);
            continue;
        }
        let mut stops = vec![(0.0, ia)];
        stops.extend(on);
        stops.push((1.0, ib));
        let mut ids = Vec::new();
        let mut list = Vec::new();
        for (k, w) in stops.windows(2).enumerate() {
            let id = fresh(&f.id, k + 1, &mut used_ids);
            let nodes = [
                name_of(w[0].1, &mut name, &mut am, &mut mesh_joints),
                name_of(w[1].1, &mut name, &mut am, &mut mesh_joints),
            ];
            // End releases stay at the object's own ends: end i on the first piece, end j on the last.
            let last = stops.len() - 2;
            let releases = Releases {
                i: if k == 0 {
                    f.releases.i.clone()
                } else {
                    Vec::new()
                },
                j: if k == last {
                    f.releases.j.clone()
                } else {
                    Vec::new()
                },
            };
            am.frames.push(Frame {
                id: id.clone(),
                nodes: nodes.clone(),
                releases,
                end_offsets: piece_offsets(
                    f,
                    w[0].0 * len,
                    w[1].0 * len,
                    len,
                    [
                        k == 0 && bending(&f.releases.i),
                        k == last && bending(&f.releases.j),
                    ],
                ),
                object: Some(f.id.clone()),
                // Joint offsets along the frame follow a line from end i to end j.
                insertion: f.insertion.map(|ins| ins.piece(w[0].0, w[1].0)),
                ..f.clone()
            });
            frame_info.push(MeshElement {
                id: id.clone(),
                parent: f.id.clone(),
                nodes: nodes.to_vec(),
            });
            list.push((id.clone(), w[0].0 * len, w[1].0 * len));
            ids.push(id);
        }
        piece_of.insert(f.id.clone(), list);
        pieces.push((f.id.clone(), ids));
        split_offsets.insert(f.id.clone(), object_offsets[&f.id]);
    }

    clock.lap("mesh: frames split, offsets");
    // Membrane floors: where a unit vertical load on each goes (load transfer, `panels::plan`).
    let named: Vec<(usize, Id)> = (0..points.len())
        .filter_map(|i| name[i].clone().map(|n| (i, n)))
        .collect();
    let host_of_area: HashMap<&Id, usize> = hosts
        .iter()
        .enumerate()
        .map(|(hi, h)| (&obj.areas[h.index].id, hi))
        .collect();
    let mut plans: HashMap<usize, panels::Plan> = HashMap::new();
    let load_frames: Vec<&Frame> = obj
        .frames
        .iter()
        .filter(|f| f.kind == FrameKind::Frame && !f.is_null())
        .collect();
    if hosts.iter().any(|h| h.membrane_floor) {
        // Joints held up by something other than Membrane floors.
        let mut held: HashSet<&Id> = HashSet::new();
        held.extend(am.frames.iter().flat_map(|f| f.nodes.iter()));
        held.extend(am.supports.iter().map(|s| &s.node));
        held.extend(am.springs.iter().map(|s| &s.node));
        held.extend(am.links.iter().flat_map(|l| l.nodes.iter()));
        for a in &am.areas {
            let host = parent.get(&a.id).and_then(|p| host_of_area.get(p));
            if host.is_some_and(|&hi| !hosts[hi].membrane_floor) {
                held.extend(a.nodes.iter());
            }
        }
        let joints: Vec<(Vec3, bool)> = named
            .iter()
            .map(|(i, n)| (points.xyz[*i], held.contains(n)))
            .collect();
        let frames: Vec<(Vec3, Vec3)> = load_frames
            .iter()
            .map(|f| (xyz[node_index[&f.nodes[0]]], xyz[node_index[&f.nodes[1]]]))
            .collect();
        let mut support_edges: Vec<(Vec3, Vec3)> = Vec::new();
        for h in hosts.iter().filter(|h| !h.membrane_floor) {
            let p = corner3(&obj.areas[h.index]);
            for k in 0..p.len() {
                support_edges.push((p[k], p[(k + 1) % p.len()]));
            }
        }
        for (hi, h) in hosts.iter().enumerate().filter(|(_, h)| h.membrane_floor) {
            let elements: Vec<Vec<Vec3>> = panel_loops
                .iter()
                .filter(|(p, _)| *p == hi)
                .map(|(_, ids)| ids.iter().map(|&i| points.xyz[i]).collect())
                .collect();
            let site = panels::Site {
                plane: &h.plane,
                frames: &frames,
                joints: &joints,
                support_edges: &support_edges,
                one_way: h.one_way,
                tol,
            };
            let plan = panels::plan(&site, &elements)
                .map_err(|e| invalid(format!("area {}: {e}", obj.areas[h.index].id)))?;
            plans.insert(hi, plan);
        }
    }
    let mut tributary_used: Vec<Id> = Vec::new();
    clock.lap("mesh: membrane load transfer");
    // Loads: area loads on every element of the area, frame loads cut at the pieces.
    // A Membrane floor's self-weight goes the way of its area loads (load transfer to the beams),
    // so its elements get none of their own in the solver.
    let membrane_floor_objects: Vec<&Area> = hosts
        .iter()
        .filter(|h| h.membrane_floor)
        .map(|h| &obj.areas[h.index])
        .collect();
    for a in &membrane_floor_objects {
        for e in element_of_area.get(&a.id).into_iter().flatten() {
            if let Some(el) = am.areas.iter_mut().find(|x| &x.id == e) {
                el.modifiers.weight = 0.0;
            }
        }
    }
    for (c, oc) in am.load_sets_mut().zip(obj.load_sets()) {
        c.area_loads.clear();
        let own_node_loads = c.node_loads.len();
        let mut floor_frame_loads: Vec<FrameLoad> = Vec::new();
        // A Membrane floor's normal loads on its elements, for the mass source (`LoadCase::mass_loads`).
        let mut floor_mass_loads: Vec<AreaLoad> = Vec::new();
        let floor_weight: Vec<AreaLoad> = membrane_floor_objects
            .iter()
            .map(|a| AreaLoad {
                area: a.id.clone(),
                dir: LoadDir::Z,
                q: -oc.self_weight * obj.area_weight(a),
            })
            .filter(|l| l.q != 0.0)
            .collect();
        for l in oc.area_loads.iter().chain(&floor_weight) {
            if let Some(&hi) = host_of_area
                .get(&l.area)
                .filter(|&&hi| hosts[hi].membrane_floor)
            {
                let h = &hosts[hi];
                let ax = h.plane.axes;
                let qv = match l.dir {
                    LoadDir::X => [l.q, 0.0, 0.0],
                    LoadDir::Y => [0.0, l.q, 0.0],
                    LoadDir::Z => [0.0, 0.0, l.q],
                    LoadDir::L1 => axes::scale(ax[0], l.q),
                    LoadDir::L2 => axes::scale(ax[1], l.q),
                    LoadDir::L3 => axes::scale(ax[2], l.q),
                };
                // Normal part: by tributary areas to the panels' edges; in-plane part: on the elements.
                let nv = axes::scale(ax[2], axes::dot(qv, ax[2]));
                let inplane = axes::sub(qv, nv);
                let big = l.q.abs();
                for (d, dir) in [LoadDir::X, LoadDir::Y, LoadDir::Z].into_iter().enumerate() {
                    if inplane[d].abs() <= 1e-12 * big {
                        continue;
                    }
                    for e in element_of_area.get(&l.area).into_iter().flatten() {
                        c.area_loads.push(AreaLoad {
                            area: e.clone(),
                            dir,
                            q: inplane[d],
                        });
                    }
                }
                if axes::dot(nv, nv).sqrt() <= 1e-12 * big {
                    continue;
                }
                for (d, dir) in [LoadDir::X, LoadDir::Y, LoadDir::Z].into_iter().enumerate() {
                    if nv[d].abs() > 1e-12 * big {
                        for e in element_of_area.get(&l.area).into_iter().flatten() {
                            floor_mass_loads.push(AreaLoad {
                                area: e.clone(),
                                dir,
                                q: nv[d],
                            });
                        }
                    }
                }
                if !tributary_used.contains(&l.area) {
                    tributary_used.push(l.area.clone());
                }
                let plan = &plans[&hi];
                for &(fi, x0, x1, d0, d1) in &plan.frames {
                    for (d, dir) in [LoadDir::X, LoadDir::Y, LoadDir::Z].into_iter().enumerate() {
                        if nv[d].abs() > 1e-12 * big {
                            floor_frame_loads.push(FrameLoad::Trapezoidal {
                                frame: load_frames[fi].id.clone(),
                                dir,
                                w1: nv[d] * d0,
                                w2: nv[d] * d1,
                                from: Some(x0),
                                to: Some(x1),
                            });
                        }
                    }
                }
                for &(j, share) in &plan.joints {
                    if share != 0.0 {
                        c.node_loads.push(NodeLoad {
                            node: named[j].1.clone(),
                            fx: nv[0] * share,
                            fy: nv[1] * share,
                            fz: nv[2] * share,
                            ..Default::default()
                        });
                    }
                }
                continue;
            }
            let opening = obj
                .areas
                .iter()
                .position(|a| a.id == l.area)
                .is_some_and(|i| opening_of[i].is_some());
            if let Some(els) = element_of_area.get(&l.area) {
                for e in els {
                    c.area_loads.push(osab_solver::model::AreaLoad {
                        area: e.clone(),
                        ..l.clone()
                    });
                }
            } else if opening {
                return Err(invalid(format!(
                    "load case {}: opening {} cannot carry a load",
                    c.id, l.area
                )));
            } else {
                c.area_loads.push(l.clone());
            }
        }
        c.frame_loads.clear();
        let mut own_frame_loads = None;
        for (k, l) in oc.frame_loads.iter().chain(&floor_frame_loads).enumerate() {
            if k == oc.frame_loads.len() {
                own_frame_loads = Some(c.frame_loads.len());
            }
            let Some(list) = piece_of.get(l.frame()) else {
                c.frame_loads.push(l.clone());
                continue;
            };
            if list.len() == 1 {
                c.frame_loads.push(l.clone());
                continue;
            }
            let len = list.last().unwrap().2;
            match l {
                FrameLoad::Uniform { .. } | FrameLoad::Trapezoidal { .. } => {
                    for (id, s, e) in list {
                        c.frame_loads.extend(l.cut(id, len, *s, *e, tol));
                    }
                }
                FrameLoad::Point { dir, p, at, .. } => {
                    let (id, s, e) = list
                        .iter()
                        .find(|(_, s, e)| *at >= s - tol && *at <= e + tol)
                        .unwrap_or(list.last().unwrap());
                    c.frame_loads.push(FrameLoad::Point {
                        frame: id.clone(),
                        dir: *dir,
                        p: *p,
                        at: (at - s).clamp(0.0, e - s),
                    });
                }
            }
        }
        // The mass source counts a Membrane floor's normal loads at its elements' corners (probe
        // 8): the case without the transferred loads, with those loads on the elements instead.
        if !floor_mass_loads.is_empty() {
            let mut m = c.clone();
            m.node_loads.truncate(own_node_loads);
            m.frame_loads
                .truncate(own_frame_loads.unwrap_or(c.frame_loads.len()));
            m.area_loads.extend(floor_mass_loads);
            c.mass_loads = Some(Box::new(m));
        }
    }

    clock.lap("mesh: loads");
    // Restraints on edges (an option, by default on for walls and off for floors): a mesh joint on
    // an area edge whose two corners have the same restraints gets them too.
    let support_of: HashMap<&Id, &Support> = obj.supports.iter().map(|s| (&s.node, s)).collect();
    let mut added_supports = 0;
    let wants = |h: &Host| obj.areas[h.index].edge_restraints.unwrap_or(h.wall);
    for h in hosts.iter().filter(|h| wants(h) && h.kind != Kind::None) {
        let a = &obj.areas[h.index];
        let n = a.nodes.len();
        for k in 0..n {
            let (ca, cb) = (&a.nodes[k], &a.nodes[(k + 1) % n]);
            let (Some(sa), Some(sb)) = (support_of.get(ca), support_of.get(cb)) else {
                continue;
            };
            let (ra, rb) = (sa.restraints()?, sb.restraints()?);
            if ra != rb || !ra.iter().any(|r| *r) {
                continue;
            }
            let (pa, pb) = (xyz[node_index[ca]], xyz[node_index[cb]]);
            for i in 0..points.len() {
                if points.joint[i].is_some() || name[i].is_none() {
                    continue;
                }
                if let Some(t) = geom::on_segment3(points.xyz[i], pa, pb, tol) {
                    if t > 0.0 && t < 1.0 {
                        let node = name[i].clone().unwrap();
                        if !am.supports.iter().any(|s| s.node == node) {
                            am.supports.push(Support {
                                node,
                                fix: sa.fix.clone(),
                            });
                            added_supports += 1;
                        }
                    }
                }
            }
        }
    }

    let (one_way, two_way): (Vec<&Id>, Vec<&Id>) = tributary_used
        .iter()
        .partition(|id| host_of_area.get(id).is_some_and(|&hi| hosts[hi].one_way));
    let names = |ids: &[&Id]| {
        ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !two_way.is_empty() {
        notes.push(format!(
            "Membrane floors {}: vertical loads go to the beams, walls and joints at their panel edges by 45-degree tributary areas",
            names(&two_way)
        ));
    }
    if !one_way.is_empty() {
        notes.push(format!(
            "Membrane floors {} (one-way): vertical loads go along local axis 1 to the beams, walls and joints at their panel edges",
            names(&one_way)
        ));
    }
    clock.lap("mesh: edge restraints");
    am.edge_constraints = hanging::find(&am);
    clock.lap("mesh: joints tied to area edges");
    let (left_out, null_notes) = nulls::transfer(&obj, &mut am, tol);
    clock.lap("mesh: null objects");
    notes.extend(null_notes);
    let n_shell = shell_info.len();
    let meshed_areas = shell_info
        .iter()
        .map(|s| &s.parent)
        .collect::<HashSet<_>>()
        .len();
    if n_shell > 0 || !pieces.is_empty() || !am.edge_constraints.is_empty() {
        let mut parts = Vec::new();
        if n_shell > 0 {
            parts.push(format!(
                "{meshed_areas} areas into {n_shell} shell elements"
            ));
        }
        if !pieces.is_empty() {
            parts.push(format!(
                "{} frames into {} pieces",
                pieces.len(),
                frame_info.len()
            ));
        }
        if !am.edge_constraints.is_empty() {
            parts.push(format!(
                "{} joints tied to area edges",
                am.edge_constraints.len()
            ));
        }
        if added_supports > 0 {
            parts.push(format!("edge restraints added at {added_supports} joints"));
        }
        notes.push(format!("Meshed: {}", parts.join(", ")));
    }
    clock.lap("mesh: notes");
    Ok(Meshed {
        info: MeshInfo {
            joints: mesh_joints,
            shells: shell_info,
            frames: frame_info,
            edge_constraints: am.edge_constraints.clone(),
        },
        model: am,
        pieces,
        offsets: split_offsets,
        parent,
        notes,
        left_out,
    })
}

/// Point on the first of two 3D segments where they cross (both strictly inside, more than `tol` from their
/// ends), if they pass within `reach` of each other.
fn crossing3(a: Vec3, b: Vec3, c: Vec3, d: Vec3, tol: f64, reach: f64) -> Option<Vec3> {
    let u = axes::sub(b, a);
    let v = axes::sub(d, c);
    let w = axes::sub(a, c);
    let (uu, uv, vv, uw, vw) = (
        axes::dot(u, u),
        axes::dot(u, v),
        axes::dot(v, v),
        axes::dot(u, w),
        axes::dot(v, w),
    );
    let den = uu * vv - uv * uv;
    if den <= 1e-12 * uu * vv {
        return None;
    }
    let s = (uv * vw - vv * uw) / den;
    let t = (uu * vw - uv * uw) / den;
    let (lu, lv) = (uu.sqrt(), vv.sqrt());
    if s * lu <= tol || (1.0 - s) * lu <= tol || t * lv <= tol || (1.0 - t) * lv <= tol {
        return None;
    }
    let p = axes::add(a, axes::scale(u, s));
    let q = axes::add(c, axes::scale(v, t));
    (geom::dist3(p, q) <= reach).then_some(p)
}
