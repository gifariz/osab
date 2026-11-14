//! Rectangular floor mesh with its default *localized meshing* (Phase 3b step 6f,
//! docs/phases/PHASE3B_PLAN.md, probe findings 2g). A floor and the objects over it (slab areas, drops, stiff
//! zones, openings) share one grid of lines along the floor's local axes 1 and 2:
//!
//! 1. Joint objects (supports, columns...) are taken first, last created first: a joint's line is kept
//!    when it is at least half the mesh size from every joint line kept before it.
//! 2. The floor's corners always give lines (its boundary).
//! 3. Corners of the objects over it come next, by slab type (Slab, Drop, Stiff), last created first,
//!    corners in reverse order: a line is kept when it is at least half the size from every line kept so
//!    far, the boundary included.
//! 4. The kept lines, without joint lines closer than half the size to the boundary, are the anchors: each
//!    span between two anchors is divided into ceil(span / size) equal pieces. These lines (and the joint
//!    lines near the boundary) cross everything.
//! 5. Localization. A joint whose line was not kept gets it between the kept lines on either side of it.
//!    Then, object by object as they were created (openings too), every corner off the grid gets a short
//!    line in each direction it is off: from the corner both ways to the first line it meets (a grid line
//!    or a short line drawn before), so it cuts only the cells next to it. Beam ends follow, then the
//!    joints of neighbouring meshes that are not on a line yet.

use crate::geom::{self, P2};

/// An object over the floor, or an opening: its creation order, slab type rank and corners.
pub struct Object {
    pub order: usize,
    pub rank: u8,
    pub opening: bool,
    pub corners: Vec<P2>,
}

pub struct Input<'a> {
    /// The floor's corners (in its plane).
    pub corners: &'a [P2],
    /// Lines cross this box (the floor and the objects over it).
    pub extent: [P2; 2],
    pub size: f64,
    /// Joint objects on the floor, in the order they are taken.
    pub joints: &'a [P2],
    pub objects: &'a [Object],
    /// Other points that must be mesh joints (beam ends).
    pub points: &'a [P2],
    /// Joints of the neighbouring meshes: they get a line only where they are not on one yet (a mesh
    /// line, or one of `edges`).
    pub shared: &'a [P2],
    /// Edges of the floors, openings and beams.
    pub edges: &'a [[P2; 2]],
    pub tol: f64,
}

/// A short line: along axis `1 - d` at coordinate `at` (axis d), from `a` to `b`.
#[derive(Clone, Copy)]
struct Short {
    d: usize,
    at: f64,
    a: f64,
    b: f64,
}

/// The mesh lines (full lines reach past the extent; the arrangement clips them).
pub fn lines(inp: &Input) -> Vec<[P2; 2]> {
    let tol = inp.tol;
    let half = 0.5 * inp.size - tol;
    let [lo, hi] = inp.extent;
    let pad = 0.01 * (hi[0] - lo[0] + hi[1] - lo[1]);
    let near = |list: &[f64], v: f64, gap: f64| list.iter().any(|a| (a - v).abs() < gap);
    let bound: [Vec<f64>; 2] = [0, 1].map(|d| {
        let mut b: Vec<f64> = inp.corners.iter().map(|c| c[d]).collect();
        sorted(&mut b, tol);
        b
    });
    // 1. Joint objects.
    let mut kept: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    let mut off_joint: Vec<(P2, usize)> = Vec::new();
    for j in inp.joints {
        for d in 0..2 {
            if near(&kept[d], j[d], tol) {
                continue;
            }
            if near(&kept[d], j[d], half) {
                off_joint.push((*j, d));
            } else {
                kept[d].push(j[d]);
            }
        }
    }
    // 2-3. The boundary, then the objects' corners.
    let mut anchors: [Vec<f64>; 2] = [0, 1].map(|d| {
        let mut a = bound[d].clone();
        a.extend(
            kept[d]
                .iter()
                .copied()
                .filter(|v| !near(&bound[d], *v, half)),
        );
        a
    });
    let mut all: [Vec<f64>; 2] = [0, 1].map(|d| {
        let mut a = bound[d].clone();
        a.extend(kept[d].iter().copied());
        a
    });
    let mut order: Vec<&Object> = inp.objects.iter().filter(|o| !o.opening).collect();
    order.sort_by_key(|o| (o.rank, std::cmp::Reverse(o.order)));
    for o in order {
        for p in o.corners.iter().rev() {
            for d in 0..2 {
                if !near(&all[d], p[d], half) {
                    all[d].push(p[d]);
                    anchors[d].push(p[d]);
                }
            }
        }
    }
    // 4. Anchors and their equal pieces.
    let grid: [Vec<f64>; 2] = [0, 1].map(|d| {
        sorted(&mut anchors[d], tol);
        let mut g = Vec::new();
        for w in anchors[d].windows(2) {
            let n = ((w[1] - w[0]) / inp.size - 1e-9).ceil().max(1.0) as usize;
            g.extend((0..n).map(|k| w[0] + (w[1] - w[0]) * k as f64 / n as f64));
        }
        g.extend(anchors[d].last().copied());
        g.extend(kept[d].iter().copied());
        sorted(&mut g, tol);
        g
    });
    sorted(&mut all[0], tol);
    sorted(&mut all[1], tol);
    let line = |d: usize, at: f64, a: f64, b: f64| -> [P2; 2] {
        if d == 0 {
            [[at, a], [at, b]]
        } else {
            [[a, at], [b, at]]
        }
    };
    let mut out: Vec<[P2; 2]> = Vec::new();
    for d in 0..2 {
        let e = 1 - d;
        for &g in &grid[d] {
            out.push(line(d, g, lo[e] - pad, hi[e] + pad));
        }
    }
    // 5. Localization. Joints: between the kept lines on either side.
    let mut shorts: Vec<Short> = Vec::new();
    for &(j, d) in &off_joint {
        let e = 1 - d;
        let a = all[e]
            .iter()
            .copied()
            .filter(|g| *g < j[e] - tol)
            .fold(lo[e] - pad, f64::max);
        let b = all[e]
            .iter()
            .copied()
            .filter(|g| *g > j[e] + tol)
            .fold(hi[e] + pad, f64::min);
        shorts.push(Short { d, at: j[d], a, b });
    }
    // A short line through `p` along axis 1 - d: both ways to the first grid or short line it meets.
    let short = |p: P2, d: usize, shorts: &[Short]| -> Short {
        let e = 1 - d;
        let crossing = |s: &Short| s.d == e && p[d] >= s.a - tol && p[d] <= s.b + tol;
        let stops = grid[e]
            .iter()
            .copied()
            .chain(shorts.iter().filter(|s| crossing(s)).map(|s| s.at));
        let (mut a, mut b) = (lo[e] - pad, hi[e] + pad);
        for g in stops {
            if g < p[e] - tol {
                a = a.max(g);
            } else if g > p[e] + tol {
                b = b.min(g);
            }
        }
        Short { d, at: p[d], a, b }
    };
    let mut objects: Vec<&Object> = inp.objects.iter().collect();
    objects.sort_by_key(|o| o.order);
    let points = objects
        .iter()
        .flat_map(|o| o.corners.iter())
        .chain(inp.points.iter());
    for p in points {
        for d in 0..2 {
            if !near(&grid[d], p[d], tol) {
                let s = short(*p, d, &shorts);
                shorts.push(s);
            }
        }
    }
    for p in inp.shared {
        for d in 0..2 {
            let e = 1 - d;
            let on_line = near(&grid[d], p[d], tol)
                || shorts.iter().any(|s| {
                    s.d == d && (s.at - p[d]).abs() <= tol && p[e] >= s.a - tol && p[e] <= s.b + tol
                })
                || inp
                    .edges
                    .iter()
                    .any(|s| geom::seg_dist(*p, s[0], s[1]) <= tol);
            if !on_line {
                let s = short(*p, d, &shorts);
                shorts.push(s);
            }
        }
    }
    out.extend(shorts.iter().map(|s| line(s.d, s.at, s.a, s.b)));
    out
}

/// A cell with one corner cut off by a skew line (a beam, a null frame or a skew edge): five corners
/// counterclockwise, four edges along the axes and one skew edge. It is split along an axis through
/// one end of the skew edge, from that end to the opposite side, choosing the end whose split leaves the
/// wider narrow part (probe findings 2h, nine cells in `n_skew_beam`, `n_skew_steep`, `n_nl_diag`,
/// `n_nl_fall`). Returns the two parts (four corners each, counterclockwise).
pub fn split_cut_corner(c: &[P2], tol: f64) -> Option<[Vec<P2>; 2]> {
    if c.len() != 5 {
        return None;
    }
    let along = |a: P2, b: P2| -> Option<usize> {
        if (b[1] - a[1]).abs() <= tol {
            Some(0)
        } else if (b[0] - a[0]).abs() <= tol {
            Some(1)
        } else {
            None
        }
    };
    let skew: Vec<usize> = (0..5)
        .filter(|&k| along(c[k], c[(k + 1) % 5]).is_none())
        .collect();
    let [s] = skew[..] else {
        return None;
    };
    let lo = [0, 1].map(|d| c.iter().map(|p| p[d]).fold(f64::INFINITY, f64::min));
    let hi = [0, 1].map(|d| c.iter().map(|p| p[d]).fold(f64::NEG_INFINITY, f64::max));
    // The cut through corner k, which lies on the axis edge starting at corner `edge`: across that edge
    // to the opposite side (a horizontal edge gives a cut at x = const). Returns the narrower of the two
    // parts' widths and the cut's far end.
    let split = |k: usize, edge: usize| -> Option<(f64, P2)> {
        let p = c[k];
        let e = along(c[edge], c[(edge + 1) % 5])?;
        let w = (p[e] - lo[e]).min(hi[e] - p[e]);
        let f = 1 - e;
        let mut far = p;
        far[f] = if (p[f] - lo[f]).abs() <= tol {
            hi[f]
        } else {
            lo[f]
        };
        (w > tol).then_some((w, far))
    };
    let (p, q) = (s, (s + 1) % 5);
    let cand = [
        split(p, (s + 4) % 5).map(|(w, f)| (w, p, f)),
        split(q, q).map(|(w, f)| (w, q, f)),
    ];
    let (_, k, far) = cand
        .into_iter()
        .flatten()
        .reduce(|a, b| if b.0 > a.0 + tol { b } else { a })?;
    // Walk the loop from corner k; the cut ends on the edge that holds `far`.
    let on_edge = |m: usize| {
        let (a, b) = (c[m], c[(m + 1) % 5]);
        geom::seg_dist(far, a, b) <= tol
    };
    let m = (1..5).map(|i| (k + i) % 5).find(|&m| on_edge(m))?;
    let mut first = vec![c[k]];
    let mut i = (k + 1) % 5;
    loop {
        first.push(c[i]);
        if i == m {
            break;
        }
        i = (i + 1) % 5;
    }
    first.push(far);
    let mut second = vec![far];
    let mut i = (m + 1) % 5;
    loop {
        second.push(c[i]);
        if i == k {
            break;
        }
        i = (i + 1) % 5;
    }
    let clean = |v: Vec<P2>| -> Vec<P2> {
        let mut out: Vec<P2> = Vec::new();
        for p in v {
            if out.last().is_none_or(|q| geom::dist(*q, p) > tol) {
                out.push(p);
            }
        }
        if out.len() > 1 && geom::dist(out[0], out[out.len() - 1]) <= tol {
            out.pop();
        }
        out
    };
    let (first, second) = (clean(first), clean(second));
    (first.len() == 4 && second.len() == 4).then_some([first, second])
}

/// Sorts and merges values closer than `tol`.
fn sorted(v: &mut Vec<f64>, tol: f64) {
    v.sort_by(|a, b| a.total_cmp(b));
    v.dedup_by(|a, b| (*a - *b).abs() <= tol);
}
