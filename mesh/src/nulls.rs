//! Loads that reach the floor without elements of their own (Phase 3b step 6g, docs/phases/PHASE3B_PLAN.md,
//! probe findings 2h): joint loads on joints left out of the mesh (*Include in mesh* = No, the
//! default for a joint with nothing attached), and loads on null frames and null areas (section or
//! property "None"). They go into the shell elements under them as consistent loads: every force
//! and moment component times the element's shape functions (bilinear on quads, linear on triangles),
//! integrated over the part of the line or area that lies on the element. Parts over no element are lost
//! (a null area used as cladding, a null frame between two columns).

use std::collections::{HashMap, HashSet};

use osab_solver::axes::{self, Vec3};
use osab_solver::model::{coords, Area, FrameLoad, Id, LoadDir, Model, NodeLoad};

use crate::boxes::{self, Box3, Boxes};
use crate::geom::{self, Plane, P2};

/// Joints of the analysis model with nothing attached (no element, frame piece, link, support, spring or
/// edge constraint): they are left out of the analysis mesh.
fn left_out(am: &Model) -> HashSet<Id> {
    let mut used: HashSet<&Id> = HashSet::new();
    used.extend(am.areas.iter().flat_map(|a| a.nodes.iter()));
    used.extend(am.frames.iter().flat_map(|f| f.nodes.iter()));
    used.extend(am.links.iter().flat_map(|l| l.nodes.iter()));
    used.extend(am.supports.iter().map(|s| &s.node));
    used.extend(am.springs.iter().map(|s| &s.node));
    used.extend(am.edge_constraints.iter().map(|e| &e.node));
    am.nodes
        .iter()
        .filter(|n| !used.contains(&n.id))
        .map(|n| n.id.clone())
        .collect()
}

/// Moves joint loads on joints left out of the mesh, and loads on null frames and null areas, onto the
/// shell elements of the analysis model `am` (rules in the module text). `obj` is the object
/// model in end order, with the same load cases as `am`. Returns the joints left out (sorted) and
/// notes.
pub fn transfer(obj: &Model, am: &mut Model, tol: f64) -> (Vec<Id>, Vec<String>) {
    let out = left_out(am);
    let floor = Floor::new(am, tol);
    let xyz: HashMap<&Id, Vec3> = obj.nodes.iter().map(|n| (&n.id, coords(n))).collect();
    let null_frames: HashMap<&Id, &osab_solver::model::Frame> = obj
        .frames
        .iter()
        .filter(|f| f.is_null())
        .map(|f| (&f.id, f))
        .collect();
    let null_areas: HashMap<&Id, &osab_solver::model::Area> = obj
        .areas
        .iter()
        .filter(|a| a.is_null())
        .map(|a| (&a.id, a))
        .collect();
    let mut moved: Vec<String> = Vec::new();
    let mut lost: Vec<String> = Vec::new();
    let note = |list: &mut Vec<String>, id: &Id| {
        if !list.contains(&id.0) {
            list.push(id.0.clone());
        }
    };
    let unit = |d: usize| {
        let mut v = [0.0; 3];
        v[d] = 1.0;
        v
    };
    for (c, oc) in am.load_sets_mut().zip(obj.load_sets()) {
        c.node_loads.retain(|l| !out.contains(&l.node));
        c.frame_loads
            .retain(|l| !null_frames.contains_key(l.frame()));
        c.area_loads.retain(|l| !null_areas.contains_key(&l.area));
        let add = |c: &mut osab_solver::model::LoadCase, s: &Shares, f: [f64; 6]| {
            for (id, w) in &s.0 {
                if *w != 0.0 {
                    c.node_loads.push(NodeLoad {
                        node: id.clone(),
                        fx: f[0] * w,
                        fy: f[1] * w,
                        fz: f[2] * w,
                        mx: f[3] * w,
                        my: f[4] * w,
                        mz: f[5] * w,
                    });
                }
            }
        };
        // Joint loads on joints left out of the mesh: the shape functions at the joint, every component.
        for l in oc.node_loads.iter().filter(|l| out.contains(&l.node)) {
            match floor.at_point(xyz[&l.node]) {
                Some(s) => {
                    add(c, &s, l.as_array());
                    note(&mut moved, &l.node);
                }
                None => note(&mut lost, &l.node),
            }
        }
        // Loads on null frames: along the frame, on the elements it lies on.
        for l in &oc.frame_loads {
            let Some(f) = null_frames.get(l.frame()) else {
                continue;
            };
            let (a, b) = (xyz[&f.nodes[0]], xyz[&f.nodes[1]]);
            let len = geom::dist3(a, b);
            let ax = axes::local_axes(a, b, f.angle);
            let dir = |d: &LoadDir| match d {
                LoadDir::X => unit(0),
                LoadDir::Y => unit(1),
                LoadDir::Z => unit(2),
                LoadDir::L1 => ax[0],
                LoadDir::L2 => ax[1],
                LoadDir::L3 => ax[2],
            };
            let (shares, v, whole) = match l {
                FrameLoad::Uniform {
                    dir: d,
                    w,
                    from,
                    to,
                    ..
                }
                | FrameLoad::Trapezoidal {
                    dir: d,
                    w1: w,
                    from,
                    to,
                    ..
                } => {
                    let (s, e) = (from.unwrap_or(0.0), to.unwrap_or(len));
                    let w2 = match l {
                        FrameLoad::Trapezoidal { w2, .. } => *w2,
                        _ => *w,
                    };
                    let (sh, carried) = floor.along(a, b, s / len, e / len, *w, w2);
                    (sh, dir(d), carried >= (e - s) - 1e3 * tol)
                }
                FrameLoad::Point { dir: d, p, at, .. } => {
                    let q = axes::add(a, axes::scale(axes::sub(b, a), at / len));
                    match floor.at_point(q) {
                        Some(mut sh) => {
                            sh.0.iter_mut().for_each(|e| e.1 *= p);
                            (sh, dir(d), true)
                        }
                        None => (Shares::default(), dir(d), false),
                    }
                }
            };
            add(c, &shares, [v[0], v[1], v[2], 0.0, 0.0, 0.0]);
            note(if whole { &mut moved } else { &mut lost }, &f.id);
        }
        // Loads on null areas: over the part on the elements in their plane.
        for l in &oc.area_loads {
            let Some(a) = null_areas.get(&l.area) else {
                continue;
            };
            let p: Vec<Vec3> = a.nodes.iter().map(|id| xyz[id]).collect();
            let ax = crate::build::area_axes(&p, a.angle);
            let d = match l.dir {
                LoadDir::X => unit(0),
                LoadDir::Y => unit(1),
                LoadDir::Z => unit(2),
                LoadDir::L1 => ax[0],
                LoadDir::L2 => ax[1],
                LoadDir::L3 => ax[2],
            };
            let (shares, carried) = floor.over(&p, ax[2]);
            let plane = Plane {
                origin: p[0],
                axes: ax,
            };
            let full = geom::area(&p.iter().map(|q| plane.to2(*q)).collect::<Vec<_>>()).abs();
            add(
                c,
                &shares,
                [d[0] * l.q, d[1] * l.q, d[2] * l.q, 0.0, 0.0, 0.0],
            );
            note(
                if carried >= full * (1.0 - 1e-9) {
                    &mut moved
                } else {
                    &mut lost
                },
                &a.id,
            );
        }
    }
    let mut notes = Vec::new();
    if !moved.is_empty() {
        notes.push(format!(
            "Loads on null objects and on joints left out of the mesh go to the floor elements under them: {}",
            moved.join(", ")
        ));
    }
    if !lost.is_empty() {
        notes.push(format!(
            "Loads not carried, wholly or in part (no floor element under them): {}",
            lost.join(", ")
        ));
    }
    let mut out: Vec<Id> = out.into_iter().collect();
    out.sort();
    (out, notes)
}

struct Elem {
    nodes: Vec<Id>,
    plane: Plane,
    /// Corners in the element's plane, in its node order.
    p2: Vec<P2>,
    /// The corners counterclockwise (for clipping).
    ccw: Vec<P2>,
}

/// The shell elements of an analysis model, for sharing loads onto their joints.
pub struct Floor {
    elems: Vec<Elem>,
    /// Each element's box, grown by its warp and three times the tolerance: a point or line on the
    /// element (within the tolerance) lies in it.
    boxes: Boxes,
    tol: f64,
}

/// Values at element joints (node id, value, in the order the joints come), added up by node; and where
/// each node is in the list.
#[derive(Default)]
pub struct Shares(pub Vec<(Id, f64)>, HashMap<Id, usize>);

impl Shares {
    fn add(&mut self, id: &Id, v: f64) {
        match self.1.get(id) {
            Some(&k) => self.0[k].1 += v,
            None => {
                self.1.insert(id.clone(), self.0.len());
                self.0.push((id.clone(), v));
            }
        }
    }
}

/// Gauss-Legendre points and weights on [0, 1] (exact to degree 7).
const GAUSS4: [(f64, f64); 4] = [
    (0.069_431_844_202_973_71, 0.173_927_422_568_726_93),
    (0.330_009_478_207_571_9, 0.326_072_577_431_273_07),
    (0.669_990_521_792_428_1, 0.326_072_577_431_273_07),
    (0.930_568_155_797_026_3, 0.173_927_422_568_726_93),
];

/// Dunavant's 7-point rule on a triangle (area coordinates, weights summing to 1; exact to degree 5).
const TRI7: [([f64; 3], f64); 7] = [
    ([1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0], 0.225),
    (
        [
            0.059_715_871_789_77,
            0.470_142_064_105_115,
            0.470_142_064_105_115,
        ],
        0.132_394_152_788_506,
    ),
    (
        [
            0.470_142_064_105_115,
            0.059_715_871_789_77,
            0.470_142_064_105_115,
        ],
        0.132_394_152_788_506,
    ),
    (
        [
            0.470_142_064_105_115,
            0.470_142_064_105_115,
            0.059_715_871_789_77,
        ],
        0.132_394_152_788_506,
    ),
    (
        [
            0.797_426_985_353_087,
            0.101_286_507_323_456,
            0.101_286_507_323_456,
        ],
        0.125_939_180_544_827,
    ),
    (
        [
            0.101_286_507_323_456,
            0.797_426_985_353_087,
            0.101_286_507_323_456,
        ],
        0.125_939_180_544_827,
    ),
    (
        [
            0.101_286_507_323_456,
            0.101_286_507_323_456,
            0.797_426_985_353_087,
        ],
        0.125_939_180_544_827,
    ),
];

impl Floor {
    pub fn new(m: &Model, tol: f64) -> Floor {
        Self::of(m, tol, |_| true)
    }

    /// The elements of `m` that `keep` accepts.
    pub fn of(m: &Model, tol: f64, keep: impl Fn(&Area) -> bool) -> Floor {
        let at: HashMap<&Id, Vec3> = m.nodes.iter().map(|n| (&n.id, coords(n))).collect();
        let elems = m
            .areas
            .iter()
            .filter(|a| keep(a))
            .filter(|a| !a.opening && (a.nodes.len() == 3 || a.nodes.len() == 4))
            .filter_map(|a| {
                let p: Vec<Vec3> = a
                    .nodes
                    .iter()
                    .map(|id| at.get(id).copied())
                    .collect::<Option<_>>()?;
                let plane = Plane {
                    origin: p[0],
                    axes: axes::area_axes(&p, 0.0),
                };
                let p2: Vec<P2> = p.iter().map(|q| plane.to2(*q)).collect();
                let mut ccw = p2.clone();
                if geom::area(&ccw) < 0.0 {
                    ccw.reverse();
                }
                let warp = p.iter().map(|q| plane.offset(*q)).fold(0.0, f64::max);
                Some((
                    Elem {
                        nodes: a.nodes.clone(),
                        plane,
                        p2,
                        ccw,
                    },
                    boxes::around(p, warp + 3.0 * tol),
                ))
            })
            .collect::<Vec<_>>();
        let (elems, boxes): (Vec<Elem>, Vec<Box3>) = elems.into_iter().unzip();
        Floor {
            elems,
            boxes: Boxes::new(boxes),
            tol,
        }
    }

    /// Shape functions of element `e` at `q` (in its plane): area coordinates on a triangle, bilinear on
    /// a quad (its natural coordinates found by Newton's method).
    fn shape(e: &Elem, q: P2) -> Vec<f64> {
        let p = &e.p2;
        if p.len() == 3 {
            let a = geom::orient(p[0], p[1], p[2]);
            return vec![
                geom::orient(q, p[1], p[2]) / a,
                geom::orient(p[0], q, p[2]) / a,
                geom::orient(p[0], p[1], q) / a,
            ];
        }
        const XI: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        let n = |s: f64, t: f64| XI.map(|c| 0.25 * (1.0 + c[0] * s) * (1.0 + c[1] * t));
        let (mut s, mut t) = (0.0, 0.0);
        for _ in 0..30 {
            let w = n(s, t);
            let x = (0..4).fold([0.0, 0.0], |acc, k| geom::add(acc, geom::scale(p[k], w[k])));
            let r = geom::sub(q, x);
            // Jacobian d(x, y) / d(s, t).
            let (mut j, mut k2) = ([0.0, 0.0], [0.0, 0.0]);
            for k in 0..4 {
                let ds = 0.25 * XI[k][0] * (1.0 + XI[k][1] * t);
                let dt = 0.25 * XI[k][1] * (1.0 + XI[k][0] * s);
                j = geom::add(j, geom::scale(p[k], ds));
                k2 = geom::add(k2, geom::scale(p[k], dt));
            }
            let det = geom::cross(j, k2);
            let (ds, dt) = (geom::cross(r, k2) / det, geom::cross(j, r) / det);
            s += ds;
            t += dt;
            if ds.abs() + dt.abs() < 1e-14 {
                break;
            }
        }
        n(s, t).to_vec()
    }

    /// The point in element `e`'s plane, if `p` lies in that plane and on the element.
    fn on(&self, e: &Elem, p: Vec3) -> Option<P2> {
        if e.plane.offset(p) > self.tol {
            return None;
        }
        let q = e.plane.to2(p);
        geom::inside(q, &e.ccw, self.tol).then_some(q)
    }

    fn coplanar(&self, e: &Elem, normal: Vec3, p: Vec3) -> bool {
        let c = axes::cross(e.plane.axes[2], normal);
        axes::dot(c, c).sqrt() < 1e-6 && e.plane.offset(p) <= self.tol
    }

    /// A point load at `p`: the shape functions of the first element it lies on, at `p`.
    pub fn at_point(&self, p: Vec3) -> Option<Shares> {
        self.boxes.at(p).into_iter().find_map(|i| {
            let e = &self.elems[i];
            let q = self.on(e, p)?;
            let mut s = Shares::default();
            for (id, v) in e.nodes.iter().zip(Self::shape(e, q)) {
                s.add(id, v);
            }
            Some(s)
        })
    }

    /// A line load on a-b (positions `t` from 0 at a to 1 at b), varying linearly from `w0` at `t0` to
    /// `w1` at `t1`: the integral of w N along the part on the elements, and the length carried.
    pub fn along(&self, a: Vec3, b: Vec3, t0: f64, t1: f64, w0: f64, w1: f64) -> (Shares, f64) {
        let len = geom::dist3(a, b);
        let at = |t: f64| axes::add(a, axes::scale(axes::sub(b, a), t));
        // Breaks where the line crosses element edges, so each stretch lies on one element.
        let mut ts = vec![t0, t1];
        for i in self.boxes.meeting(&boxes::around([a, b], 3.0 * self.tol)) {
            let e = &self.elems[i];
            if e.plane.offset(a) > self.tol || e.plane.offset(b) > self.tol {
                continue;
            }
            let (a2, b2) = (e.plane.to2(a), e.plane.to2(b));
            let n = e.p2.len();
            for k in 0..n {
                if let Some((t, _)) = geom::seg_cross(a2, b2, e.p2[k], e.p2[(k + 1) % n], 0.0) {
                    if t > t0 && t < t1 {
                        ts.push(t);
                    }
                }
            }
        }
        ts.sort_by(|x, y| x.total_cmp(y));
        ts.dedup_by(|x, y| (*x - *y).abs() * len <= self.tol);
        let mut out = Shares::default();
        let mut carried = 0.0;
        let w = |t: f64| {
            if t1 > t0 {
                w0 + (w1 - w0) * (t - t0) / (t1 - t0)
            } else {
                w0
            }
        };
        // A stretch along an edge shared by two elements goes to the one whose edge there is longer: the
        // coarser one, without the joints of the finer one hanging on it (probes n_nl_order and
        // n_nl_order_m).
        let edge_len = |e: &Elem, q: P2| -> f64 {
            let n = e.p2.len();
            (0..n)
                .filter(|&k| geom::seg_dist(q, e.p2[k], e.p2[(k + 1) % n]) <= self.tol)
                .map(|k| geom::dist(e.p2[k], e.p2[(k + 1) % n]))
                .fold(0.0, f64::max)
        };
        for s in ts.windows(2) {
            let (ta, tb) = (s[0], s[1]);
            let mid = at(0.5 * (ta + tb));
            let Some(e) = self
                .boxes
                .at(mid)
                .into_iter()
                .map(|i| &self.elems[i])
                .filter_map(|e| self.on(e, mid).map(|q| (e, edge_len(e, q))))
                .reduce(|a, b| if b.1 > a.1 + self.tol { b } else { a })
                .map(|(e, _)| e)
            else {
                continue;
            };
            carried += (tb - ta) * len;
            for (g, gw) in GAUSS4 {
                let t = ta + (tb - ta) * g;
                let n = Self::shape(e, e.plane.to2(at(t)));
                for (id, v) in e.nodes.iter().zip(n) {
                    out.add(id, gw * (tb - ta) * len * w(t) * v);
                }
            }
        }
        (out, carried)
    }

    /// A uniform load on a flat polygon (3D corners, unit normal `normal`): the integral of N over the
    /// part on each element in its plane, and the area carried.
    pub fn over(&self, poly: &[Vec3], normal: Vec3) -> (Shares, f64) {
        let mut out = Shares::default();
        let mut carried = 0.0;
        // The polygon's box, grown by its tilt from the elements' planes that `coplanar` allows.
        let b = boxes::around(poly.iter().copied(), 0.0);
        let reach = boxes::around(b, 3.0 * self.tol + 1e-5 * geom::dist3(b[0], b[1]));
        for i in self.boxes.meeting(&reach) {
            let e = &self.elems[i];
            if !self.coplanar(e, normal, poly[0]) {
                continue;
            }
            let subject: Vec<P2> = poly.iter().map(|q| e.plane.to2(*q)).collect();
            let clip = clip_convex(&subject, &e.ccw);
            if clip.len() < 3 {
                continue;
            }
            // A fan of triangles with signed areas (the clipped polygon is counterclockwise, maybe not
            // convex).
            for k in 1..clip.len() - 1 {
                let (p0, p1, p2) = (clip[0], clip[k], clip[k + 1]);
                let a = 0.5 * geom::orient(p0, p1, p2);
                if a == 0.0 {
                    continue;
                }
                carried += a;
                for (l, gw) in TRI7 {
                    let q = geom::add(
                        geom::add(geom::scale(p0, l[0]), geom::scale(p1, l[1])),
                        geom::scale(p2, l[2]),
                    );
                    for (id, v) in e.nodes.iter().zip(Self::shape(e, q)) {
                        out.add(id, gw * a * v);
                    }
                }
            }
        }
        (out, carried)
    }
}

/// The part of polygon `subject` (any simple polygon) inside the convex counterclockwise polygon `clip`
/// (Sutherland-Hodgman). For a concave subject the result may have zero-width bridges along the clip's
/// edges; they add nothing to integrals over it.
fn clip_convex(subject: &[P2], clip: &[P2]) -> Vec<P2> {
    let mut out: Vec<P2> = subject.to_vec();
    // Counterclockwise, so the fan of the result has positive areas.
    if geom::area(&out) < 0.0 {
        out.reverse();
    }
    let n = clip.len();
    for k in 0..n {
        let (a, b) = (clip[k], clip[(k + 1) % n]);
        let input = std::mem::take(&mut out);
        if input.is_empty() {
            break;
        }
        let side = |p: P2| geom::orient(a, b, p);
        for i in 0..input.len() {
            let (p, q) = (input[i], input[(i + 1) % input.len()]);
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                out.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                out.push(geom::lerp(p, q, sp / (sp - sq)));
            }
        }
    }
    out
}
