//! Membrane floors (probe findings 2d): the floor is cut into panels at its beams and walls,
//! each panel is one element, and its vertical load goes to the panel's edges by 45-degree tributary
//! areas (the straight skeleton): to the beams along an edge as trapezoidal loads, and where an edge has
//! no beam, to the joints along it (a wall's joints, or the edge's two ends). With one-way load
//! distribution (`Site::one_way`) the load goes as strips along local axis 1 instead
//! (`one_way_tributary`), to the same beams, walls and joints.
//!
//! A panel that is not convex (an L-shape, a ring around an opening) is first cut at each re-entrant
//! corner along the extension of its edge closest to local axis 1 (an L-shaped floor is cut along
//! the line that continues its X-direction edge from the inside corner).

use osab_solver::axes::{self, Vec3};

use crate::arrange::Arrangement;
use crate::geom::{self, Plane, P2};

/// One cut per re-entrant corner of every face that is not convex or has holes: from the corner along
/// the extension of its edge closest to local axis 1 (u in the plane), to the face's boundary.
pub fn convex_cuts(arr: &Arrangement, tol: f64) -> Vec<[P2; 2]> {
    let mut cuts = Vec::new();
    for f in &arr.faces {
        let loops: Vec<&Vec<usize>> = std::iter::once(&f.outer).chain(f.holes.iter()).collect();
        let edges: Vec<(usize, usize)> = loops
            .iter()
            .flat_map(|c| (0..c.len()).map(move |k| (c[k], c[(k + 1) % c.len()])))
            .filter(|(a, b)| a != b)
            .collect();
        for c in &loops {
            let n = c.len();
            for k in 0..n {
                let (u, v, w) = (c[(k + n - 1) % n], c[k], c[(k + 1) % n]);
                let (pu, pv, pw) = (arr.verts[u], arr.verts[v], arr.verts[w]);
                let din = geom::sub(pv, pu);
                let dout = geom::sub(pw, pv);
                let (lin, lout) = (geom::norm(din), geom::norm(dout));
                if lin <= tol || lout <= tol {
                    continue;
                }
                // Turning right (or straight back at the tip of a line ending inside) = re-entrant.
                let tip = u == w;
                let turn = geom::cross(din, dout) / (lin * lout);
                if !(tip || turn < -1e-9) {
                    continue;
                }
                // Candidate directions: the incoming edge carried on, the outgoing edge carried back.
                let a = geom::scale(din, 1.0 / lin);
                let b = geom::scale(dout, -1.0 / lout);
                let dir = if tip || a[0].abs() >= b[0].abs() - 1e-9 {
                    a
                } else {
                    b
                };
                if let Some(p) = ray_hit(arr, &edges, v, dir, tol) {
                    cuts.push([pv, p]);
                }
            }
        }
    }
    cuts
}

/// Nearest point where the ray from vertex `v` along `dir` meets one of `edges` (not touching `v`).
fn ray_hit(arr: &Arrangement, edges: &[(usize, usize)], v: usize, dir: P2, tol: f64) -> Option<P2> {
    let p = arr.verts[v];
    let mut best: Option<f64> = None;
    for &(a, b) in edges {
        if a == v || b == v {
            continue;
        }
        let (pa, pb) = (arr.verts[a], arr.verts[b]);
        let s = geom::sub(pb, pa);
        let den = geom::cross(dir, s);
        if den.abs() <= 1e-12 * geom::norm(s) {
            continue;
        }
        let w = geom::sub(pa, p);
        let t = geom::cross(w, s) / den;
        let u = geom::cross(w, dir) / den;
        let tu = tol / geom::norm(s);
        if t > tol && u >= -tu && u <= 1.0 + tu && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best.map(|t| geom::add(p, geom::scale(dir, t)))
}

/// Whether a face is convex with no holes (within `tol`).
pub fn is_convex(arr: &Arrangement, outer: &[usize], holes: usize, tol: f64) -> bool {
    if holes > 0 {
        return false;
    }
    let n = outer.len();
    (0..n).all(|k| {
        let (u, v, w) = (outer[(k + n - 1) % n], outer[k], outer[(k + 1) % n]);
        if u == w {
            return false;
        }
        let (din, dout) = (
            geom::sub(arr.verts[v], arr.verts[u]),
            geom::sub(arr.verts[w], arr.verts[v]),
        );
        let (lin, lout) = (geom::norm(din), geom::norm(dout));
        lin <= tol || lout <= tol || geom::cross(din, dout) / (lin * lout) >= -1e-9
    })
}

/// Positions in `pts` of the corners of a closed loop, without points lying on the line between their
/// neighbours (within `tol`).
pub fn corners(pts: &[P2], tol: f64) -> Vec<usize> {
    let mut keep: Vec<usize> = (0..pts.len()).collect();
    loop {
        let n = keep.len();
        if n <= 3 {
            return keep;
        }
        let drop = (0..n).find(|&k| {
            let (a, b, c) = (
                pts[keep[(k + n - 1) % n]],
                pts[keep[k]],
                pts[keep[(k + 1) % n]],
            );
            geom::seg_dist(b, a, c) <= tol || geom::dist(a, b) <= tol
        });
        match drop {
            Some(k) => {
                keep.remove(k);
            }
            None => return keep,
        }
    }
}

/// A stretch of a load profile along an edge: from `s0` to `s1` (distances from the edge's start), the
/// tributary depth going linearly from `d0` to `d1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub s0: f64,
    pub s1: f64,
    pub d0: f64,
    pub d1: f64,
}

/// Tributary areas of a convex counterclockwise element: for every edge k (corner k to k + 1), the depth
/// of the area it carries along it; the depths add up to the element's area. A quad (or more corners)
/// splits at 45 degrees: each point goes to the edge whose line is nearest (the straight skeleton of a
/// convex polygon). A triangle gives each edge the triangle between it and the centroid, a third of the
/// area each (probes m_tri and m_tri_b).
pub fn tributary(poly: &[P2]) -> Vec<Vec<Piece>> {
    let n = poly.len();
    if n == 3 {
        let c = geom::scale(geom::add(geom::add(poly[0], poly[1]), poly[2]), 1.0 / 3.0);
        let third = geom::area(poly).abs() / 3.0;
        return (0..3)
            .map(|k| {
                let (a, b) = (poly[k], poly[(k + 1) % 3]);
                let l = geom::dist(a, b);
                let t = geom::scale(geom::sub(b, a), 1.0 / l);
                // Peak under the centroid (kept on the edge for needle-like triangles, same area).
                let sc = geom::dot(geom::sub(c, a), t).clamp(0.0, l);
                let h = 2.0 * third / l;
                [(0.0, sc, 0.0, h), (sc, l, h, 0.0)]
                    .into_iter()
                    .filter(|p| p.1 - p.0 > 1e-12 * l)
                    .map(|(s0, s1, d0, d1)| Piece { s0, s1, d0, d1 })
                    .collect()
            })
            .collect();
    }
    let frame = |k: usize| {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        let d = geom::sub(b, a);
        let l = geom::norm(d);
        let t = geom::scale(d, 1.0 / l);
        (a, t, [-t[1], t[0]], l)
    };
    (0..n)
        .map(|e| {
            let (ae, te, ne, le) = frame(e);
            // The region nearer to edge e's line than to any other edge's line.
            let mut region: Vec<P2> = poly.to_vec();
            for f in (0..n).filter(|&f| f != e) {
                let (af, _, nf, _) = frame(f);
                // Keep n_e.(p - a_e) - n_f.(p - a_f) <= 0.
                let g = |p: P2| geom::dot(ne, geom::sub(p, ae)) - geom::dot(nf, geom::sub(p, af));
                region = clip(&region, g);
                if region.len() < 3 {
                    break;
                }
            }
            if region.len() < 3 {
                return Vec::new();
            }
            let sd: Vec<(f64, f64)> = region
                .iter()
                .map(|p| {
                    let r = geom::sub(*p, ae);
                    (geom::dot(r, te).clamp(0.0, le), geom::dot(r, ne).max(0.0))
                })
                .collect();
            let mut ss: Vec<f64> = sd.iter().map(|x| x.0).collect();
            ss.sort_by(|a, b| a.total_cmp(b));
            ss.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * le);
            // Depth at s: the highest point of the region's boundary above s.
            let depth = |s: f64| {
                let m = sd.len();
                (0..m)
                    .filter_map(|k| {
                        let ((s0, d0), (s1, d1)) = (sd[k], sd[(k + 1) % m]);
                        let (lo, hi) = (s0.min(s1), s0.max(s1));
                        if s < lo - 1e-12 * le || s > hi + 1e-12 * le {
                            return None;
                        }
                        Some(if hi - lo <= 1e-12 * le {
                            d0.max(d1)
                        } else {
                            d0 + (d1 - d0) * (s - s0) / (s1 - s0)
                        })
                    })
                    .fold(0.0, f64::max)
            };
            ss.windows(2)
                .filter(|w| w[1] - w[0] > 1e-12 * le)
                .map(|w| Piece {
                    s0: w[0],
                    s1: w[1],
                    d0: depth(w[0]),
                    d1: depth(w[1]),
                })
                .collect()
        })
        .collect()
}

/// One-way load distribution of a convex counterclockwise element (an option, probe 11,
/// docs/elements/ONE_WAY_LOAD.md): strips along local axis 1 (x in the plane), each giving half its
/// load to each end. Along edge k the depth is l / 2 x |sin a|, l the strip's length through the point
/// and a the angle between the edge and axis 1; an edge along axis 1 carries nothing. The depths add up
/// to the element's area.
pub fn one_way_tributary(poly: &[P2]) -> Vec<Vec<Piece>> {
    let n = poly.len();
    // Length of the strip (chord along x) at height y.
    let chord = |y: f64| {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for k in 0..n {
            let (p, q) = (poly[k], poly[(k + 1) % n]);
            if (p[1] - y) * (q[1] - y) > 0.0 {
                continue;
            }
            let xs = if (q[1] - p[1]).abs() <= f64::EPSILON * (p[1].abs() + q[1].abs()).max(1.0) {
                [p[0], q[0]]
            } else {
                let x = p[0] + (q[0] - p[0]) * (y - p[1]) / (q[1] - p[1]);
                [x, x]
            };
            for x in xs {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
        (hi - lo).max(0.0)
    };
    (0..n)
        .map(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % n]);
            let d = geom::sub(b, a);
            let len = geom::norm(d);
            let t = geom::scale(d, 1.0 / len);
            let sin = t[1].abs();
            if sin <= 1e-12 {
                return Vec::new();
            }
            // The chord is linear in y between the corners' heights: break the edge there.
            let mut ss: Vec<f64> = poly
                .iter()
                .map(|c| (c[1] - a[1]) / t[1])
                .filter(|&s| s > 0.0 && s < len)
                .chain([0.0, len])
                .collect();
            ss.sort_by(|x, y| x.total_cmp(y));
            ss.dedup_by(|x, y| (*x - *y).abs() <= 1e-12 * len);
            let depth = |s: f64| chord(a[1] + t[1] * s) / 2.0 * sin;
            ss.windows(2)
                .filter(|w| w[1] - w[0] > 1e-12 * len)
                .map(|w| Piece {
                    s0: w[0],
                    s1: w[1],
                    d0: depth(w[0]),
                    d1: depth(w[1]),
                })
                .collect()
        })
        .collect()
}

/// Clips a convex polygon to g(p) <= 0 (g linear).
fn clip(poly: &[P2], g: impl Fn(P2) -> f64) -> Vec<P2> {
    let n = poly.len();
    let mut out = Vec::new();
    for k in 0..n {
        let (p, q) = (poly[k], poly[(k + 1) % n]);
        let (gp, gq) = (g(p), g(q));
        if gp <= 0.0 {
            out.push(p);
        }
        if (gp < 0.0 && gq > 0.0) || (gp > 0.0 && gq < 0.0) {
            out.push(geom::lerp(p, q, gp / (gp - gq)));
        }
    }
    out
}

/// Integrals of the depth and of s times the depth over `c..d` of a profile.
pub fn moments(pieces: &[Piece], c: f64, d: f64) -> (f64, f64) {
    pieces.iter().fold((0.0, 0.0), |(f, m), p| {
        let (pf, pm) = osab_solver::elements::distributed_part(p.d0, p.d1, p.s0, p.s1, c, d);
        (f + pf, m + pm)
    })
}

/// Shares of the profile's load on `c..d` at joints `js` along it (positions, sorted, `c` and `d`
/// included): each stretch between two joints gives half its load to each of them (probe m_L:
/// also when the load on the stretch is not symmetric).
pub fn joint_shares(pieces: &[Piece], js: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; js.len()];
    for k in 0..js.len().saturating_sub(1) {
        let (a, b) = (js[k], js[k + 1]);
        if b - a <= 0.0 {
            continue;
        }
        let (f, _) = moments(pieces, a, b);
        out[k] += f / 2.0;
        out[k + 1] += f / 2.0;
    }
    out
}

/// The parts of a profile on `c..d`, as (start, end, depth at start, depth at end).
pub fn part(pieces: &[Piece], c: f64, d: f64) -> Vec<Piece> {
    pieces
        .iter()
        .filter_map(|p| {
            let (a, b) = (p.s0.max(c), p.s1.min(d));
            if b - a <= 1e-12 * (p.s1 - p.s0).max(d - c) {
                return None;
            }
            let at = |s: f64| p.d0 + (p.d1 - p.d0) * (s - p.s0) / (p.s1 - p.s0);
            Some(Piece {
                s0: a,
                s1: b,
                d0: at(a),
                d1: at(b),
            })
        })
        .collect()
}

/// Where a Membrane floor's vertical load goes, for a unit load (1 per area): trapezoidal loads on frames
/// (frame index, from, to, depth at from, depth at to; distances along the frame from its end i) and
/// shares at joints (joint index, area).
#[derive(Debug, Default)]
pub struct Plan {
    pub frames: Vec<(usize, f64, f64, f64, f64)>,
    pub joints: Vec<(usize, f64)>,
}

/// What a Membrane floor's elements rest on.
pub struct Site<'a> {
    pub plane: &'a Plane,
    /// Frames that can carry floor loads: end i and end j.
    pub frames: &'a [(Vec3, Vec3)],
    /// Analysis joints: position, and whether something other than Membrane floors holds it up (a
    /// frame, a support, a spring, a link, a wall or another shell).
    pub joints: &'a [(Vec3, bool)],
    /// Edges of walls and other shells that carry vertical load.
    pub support_edges: &'a [(Vec3, Vec3)],
    /// One-way load distribution (`one_way_tributary`) instead of two-way (`tributary`).
    pub one_way: bool,
    pub tol: f64,
}

/// Load transfer of Membrane floor elements (probe findings 2d and step 6d). Each element's
/// tributary areas (`tributary`) go to its edges; where a beam lies along an edge, that stretch's load
/// becomes a trapezoidal load on the beam; along a wall, it goes to the wall's joints. The rest (edges
/// with nothing under them) depends on how many of the element's corners are held up:
/// - three or more: each stretch between two joints gives half its load to each of them, and a half
///   that lands on a joint nothing holds up moves on to the nearest held joint of that stretch
///   (probes m_free, m_L, m_cant);
/// - fewer (all held corners on one line, like a panel hanging from one beam): the rest is spread evenly
///   along the element's beams and walls (probes m_cant2, m_tri, m_hole).
pub fn plan(site: &Site, elements: &[Vec<Vec3>]) -> Result<Plan, String> {
    let tol = site.tol;
    let mut out = Plan::default();
    let joint_at = |p: Vec3| {
        site.joints
            .iter()
            .position(|(q, _)| geom::dist3(*q, p) <= tol)
    };
    for corners in elements {
        let mut corners = corners.clone();
        let mut p2: Vec<P2> = corners.iter().map(|q| site.plane.to2(*q)).collect();
        if geom::area(&p2) < 0.0 {
            corners.reverse();
            p2.reverse();
        }
        let n = corners.len();
        let trib = if site.one_way {
            one_way_tributary(&p2)
        } else {
            tributary(&p2)
        };
        let held: Vec<Option<usize>> = corners
            .iter()
            .map(|q| joint_at(*q).filter(|&j| site.joints[j].1))
            .collect();
        let n_held = held.iter().filter(|j| j.is_some()).count();
        let edge = |k: usize| {
            let (a3, b3) = (corners[k], corners[(k + 1) % n]);
            let len = geom::dist3(a3, b3);
            (a3, axes::scale(axes::sub(b3, a3), 1.0 / len), len)
        };
        // Stretches of every edge: on a beam (frame index), on a wall, or free.
        let mut beams: Vec<(usize, usize, f64, f64)> = Vec::new();
        let mut walls: Vec<(usize, f64, f64)> = Vec::new();
        let mut free: Vec<(usize, f64, f64)> = Vec::new();
        for k in 0..n {
            let (a3, t, len) = edge(k);
            let on_line = |p: Vec3| {
                let r = axes::sub(p, a3);
                let off = axes::sub(r, axes::scale(t, axes::dot(r, t)));
                axes::dot(off, off).sqrt() <= tol
            };
            let s_of = |p: Vec3| axes::dot(axes::sub(p, a3), t);
            let mut rest: Vec<(f64, f64)> = vec![(0.0, len)];
            for (fi, (f0, f1)) in site.frames.iter().enumerate() {
                if !(on_line(*f0) && on_line(*f1)) {
                    continue;
                }
                let (sa, sb) = (s_of(*f0).min(s_of(*f1)), s_of(*f0).max(s_of(*f1)));
                let mut next = Vec::new();
                for &(c0, c1) in &rest {
                    let (lo, hi) = (c0.max(sa), c1.min(sb));
                    if hi - lo <= tol {
                        next.push((c0, c1));
                        continue;
                    }
                    if lo - c0 > tol {
                        next.push((c0, lo));
                    }
                    if c1 - hi > tol {
                        next.push((hi, c1));
                    }
                    beams.push((fi, k, lo, hi));
                }
                rest = next;
            }
            for (lo, hi) in rest {
                let (p, q) = (
                    axes::add(a3, axes::scale(t, lo)),
                    axes::add(a3, axes::scale(t, hi)),
                );
                let on_wall = site.support_edges.iter().any(|(e0, e1)| {
                    geom::on_segment3(p, *e0, *e1, tol).is_some()
                        && geom::on_segment3(q, *e0, *e1, tol).is_some()
                });
                if on_wall {
                    walls.push((k, lo, hi));
                } else {
                    free.push((k, lo, hi));
                }
            }
        }
        // Joints along a stretch, sorted (its two ends must be joints).
        let joints_on = |k: usize, lo: f64, hi: f64| -> Result<Vec<(f64, usize)>, String> {
            let (a3, t, _) = edge(k);
            let mut js: Vec<(f64, usize)> = site
                .joints
                .iter()
                .enumerate()
                .filter_map(|(j, (q, _))| {
                    let r = axes::sub(*q, a3);
                    let s = axes::dot(r, t);
                    let off = axes::sub(r, axes::scale(t, s));
                    (axes::dot(off, off).sqrt() <= tol && s >= lo - tol && s <= hi + tol)
                        .then_some((s.clamp(lo, hi), j))
                })
                .collect();
            js.sort_by(|a, b| a.0.total_cmp(&b.0));
            js.dedup_by(|a, b| (a.0 - b.0).abs() <= tol);
            let ends = js.first().is_some_and(|j| j.0 - lo <= tol)
                && js.last().is_some_and(|j| hi - j.0 <= tol);
            if ends {
                Ok(js)
            } else {
                Err("no joint at the end of a panel edge to take its load".to_string())
            }
        };
        let frame_x = |fi: usize, k: usize, s: f64| {
            let (f0, f1) = site.frames[fi];
            let lf = geom::dist3(f0, f1);
            let (a3, t, _) = edge(k);
            let uf = axes::scale(axes::sub(f1, f0), 1.0 / lf);
            axes::dot(axes::sub(axes::add(a3, axes::scale(t, s)), f0), uf).clamp(0.0, lf)
        };
        let on_frame = |fi: usize, k: usize, piece: Piece, out: &mut Plan| {
            let (mut x0, mut x1) = (frame_x(fi, k, piece.s0), frame_x(fi, k, piece.s1));
            let (mut w0, mut w1) = (piece.d0, piece.d1);
            if x0 > x1 {
                std::mem::swap(&mut x0, &mut x1);
                std::mem::swap(&mut w0, &mut w1);
            }
            if w0.max(w1) > 0.0 && x1 - x0 > tol {
                out.frames.push((fi, x0, x1, w0, w1));
            }
        };
        // Each stretch between two joints gives half its load to each.
        let to_joints = |k: usize,
                         lo: f64,
                         hi: f64,
                         prof: &[Piece],
                         out: &mut Vec<(usize, f64)>|
         -> Result<Vec<(f64, usize)>, String> {
            let js = joints_on(k, lo, hi)?;
            let pos: Vec<f64> = js.iter().map(|j| j.0).collect();
            for ((_, j), share) in js.iter().zip(joint_shares(prof, &pos)) {
                out.push((*j, share));
            }
            Ok(js)
        };
        for &(fi, k, lo, hi) in &beams {
            for piece in part(&trib[k], lo, hi) {
                on_frame(fi, k, piece, &mut out);
            }
        }
        for &(k, lo, hi) in &walls {
            to_joints(k, lo, hi, &part(&trib[k], lo, hi), &mut out.joints)?;
        }
        let support_len: f64 = beams.iter().map(|b| b.3 - b.2).sum::<f64>()
            + walls.iter().map(|w| w.2 - w.1).sum::<f64>();
        if n_held < 3 && support_len > tol {
            let rest: f64 = free
                .iter()
                .map(|&(k, lo, hi)| moments(&trib[k], lo, hi).0)
                .sum();
            let u = rest / support_len;
            let even = |lo: f64, hi: f64| Piece {
                s0: lo,
                s1: hi,
                d0: u,
                d1: u,
            };
            for &(fi, k, lo, hi) in &beams {
                on_frame(fi, k, even(lo, hi), &mut out);
            }
            for &(k, lo, hi) in &walls {
                to_joints(k, lo, hi, &[even(lo, hi)], &mut out.joints)?;
            }
            continue;
        }
        for &(k, lo, hi) in &free {
            let mut shares = Vec::new();
            let js = to_joints(k, lo, hi, &part(&trib[k], lo, hi), &mut shares)?;
            for (j, share) in shares {
                if site.joints[j].1 {
                    out.joints.push((j, share));
                    continue;
                }
                // Not held up: to the nearest held joint of the stretch, else of the element.
                let s = js.iter().find(|x| x.1 == j).map_or(0.0, |x| x.0);
                let near = js
                    .iter()
                    .filter(|x| site.joints[x.1].1)
                    .min_by(|a, b| (a.0 - s).abs().total_cmp(&(b.0 - s).abs()))
                    .map(|x| x.1)
                    .or_else(|| {
                        held.iter().flatten().copied().min_by(|&a, &b| {
                            geom::dist3(site.joints[a].0, site.joints[j].0)
                                .total_cmp(&geom::dist3(site.joints[b].0, site.joints[j].0))
                        })
                    })
                    .ok_or_else(|| {
                        "a panel has no beam, wall or held-up corner to rest on".to_string()
                    })?;
                out.joints.push((near, share));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrange::arrange;

    fn shares(poly: &[P2]) -> Vec<f64> {
        tributary(poly)
            .iter()
            .map(|p| moments(p, f64::NEG_INFINITY, f64::INFINITY).0)
            .collect()
    }

    #[test]
    fn rectangle_gives_trapezoids_and_triangles() {
        // Probe m_beams: 6 x 4 m at 5 kPa puts 40 kN on each 6 m beam and 20 kN on each 4 m beam.
        let poly = [[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]];
        let a = shares(&poly);
        for (got, want) in a.iter().zip([8.0, 4.0, 8.0, 4.0]) {
            assert!((got - want).abs() < 1e-12, "{a:?}");
        }
        let t = tributary(&poly);
        assert_eq!(
            t[0],
            vec![
                Piece {
                    s0: 0.0,
                    s1: 2.0,
                    d0: 0.0,
                    d1: 2.0
                },
                Piece {
                    s0: 2.0,
                    s1: 4.0,
                    d0: 2.0,
                    d1: 2.0
                },
                Piece {
                    s0: 4.0,
                    s1: 6.0,
                    d0: 2.0,
                    d1: 0.0
                },
            ]
        );
    }

    #[test]
    fn one_way_spans_along_axis_1() {
        // Probe 11: 8 x 4 m, axis 1 along the 8 m side: all to the 4 m edges, 16 m2 each, uniform
        // 4 m deep; the edges along axis 1 carry nothing.
        let t = one_way_tributary(&[[0.0, 0.0], [8.0, 0.0], [8.0, 4.0], [0.0, 4.0]]);
        assert!(t[0].is_empty() && t[2].is_empty(), "{t:?}");
        for k in [1, 3] {
            assert_eq!(t[k].len(), 1);
            assert!(
                (t[k][0].d0 - 4.0).abs() < 1e-12 && (t[k][0].d1 - 4.0).abs() < 1e-12,
                "{t:?}"
            );
        }
        // Probe 11 F (trapezoid, 37.5 m2: half to each of the two sloping/vertical edges) and H
        // (triangle: 8 m2 to each edge across axis 1).
        for (poly, want) in [
            (
                vec![[0.0, 0.0], [8.0, 0.0], [7.0, 5.0], [0.0, 5.0]],
                vec![0.0, 18.75, 0.0, 18.75],
            ),
            (
                vec![[0.0, 0.0], [8.0, 0.0], [0.0, 4.0]],
                vec![0.0, 8.0, 8.0],
            ),
        ] {
            let got: Vec<f64> = one_way_tributary(&poly)
                .iter()
                .map(|p| moments(p, f64::NEG_INFINITY, f64::INFINITY).0)
                .collect();
            for (g, w) in got.iter().zip(&want) {
                assert!((g - w).abs() < 1e-9, "{got:?}");
            }
        }
        // Turned 30 degrees (probe 11 E in the floor's own axes): the depths add up to the area, and
        // next to a corner of the edge across axis 1 the depth is l / 2 x |sin a|.
        let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let rot = |p: [f64; 2]| [c * p[0] + s * p[1], -s * p[0] + c * p[1]];
        let poly: Vec<P2> = [[0.0, 0.0], [8.0, 0.0], [8.0, 4.0], [0.0, 4.0]]
            .map(rot)
            .to_vec();
        let t = one_way_tributary(&poly);
        let total: f64 = t
            .iter()
            .map(|p| moments(p, f64::NEG_INFINITY, f64::INFINITY).0)
            .sum();
        assert!((total - 32.0).abs() < 1e-9, "{total}");
        // Edge 3 (x = 0 in plan, from y = 4 down to 0): at its lower end the strip is 8 m long.
        let last = t[3].last().unwrap();
        assert!((last.d1 - 8.0 / 2.0 * c).abs() < 1e-9, "{t:?}");
    }

    #[test]
    fn trapezoid_splits_into_strips() {
        // Probe m_trap at 5 kPa: 34.38, 20.62, 24.38, 20.62 kN.
        let poly = [[0.0, 0.0], [6.0, 0.0], [5.0, 4.0], [1.0, 4.0]];
        let a: Vec<f64> = shares(&poly).iter().map(|x| 5.0 * x).collect();
        for (got, want) in a.iter().zip([34.38, 20.62, 24.38, 20.62]) {
            assert!((got - want).abs() < 0.005, "{a:?}");
        }
        assert!((a.iter().sum::<f64>() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn triangle_edges_carry_a_third_each() {
        // Probe m_tri_b: 20 kN on each of the three beams at 60 kN; m_tri: the peak under the centroid.
        let a = shares(&[[0.0, 0.0], [6.0, 0.0], [2.0, 4.0]]);
        for got in &a {
            assert!((got - 4.0).abs() < 1e-12, "{a:?}");
        }
        let t = tributary(&[[0.0, 0.0], [6.0, 0.0], [3.0, 4.0]]);
        assert_eq!(
            t[0][0],
            Piece {
                s0: 0.0,
                s1: 3.0,
                d0: 0.0,
                d1: 4.0 / 3.0
            }
        );
    }

    #[test]
    fn free_edge_goes_half_to_each_end() {
        // Probe m_L: also when the load on the stretch is not symmetric.
        let p = [Piece {
            s0: 0.0,
            s1: 6.0,
            d0: 0.0,
            d1: 3.0,
        }];
        let s = joint_shares(&p, &[0.0, 6.0]);
        assert!(
            (s[0] - 4.5).abs() < 1e-12 && (s[1] - 4.5).abs() < 1e-12,
            "{s:?}"
        );
        // A joint in between: each stretch to its own two ends.
        let s = joint_shares(&p, &[0.0, 3.0, 6.0]);
        let want = [0.5 * 2.25, 0.5 * 2.25 + 0.5 * 6.75, 0.5 * 6.75];
        for (got, w) in s.iter().zip(want) {
            assert!((got - w).abs() < 1e-12, "{s:?}");
        }
    }

    fn cut_once(outer: &[P2], holes: &[Vec<P2>]) -> (Vec<[P2; 2]>, usize) {
        let mut segs: Vec<[P2; 2]> = (0..outer.len())
            .map(|k| [outer[k], outer[(k + 1) % outer.len()]])
            .collect();
        for h in holes {
            segs.extend((0..h.len()).map(|k| [h[k], h[(k + 1) % h.len()]]));
        }
        let arr = arrange(&segs, &[], outer, holes, 1e-9);
        let cuts = convex_cuts(&arr, 1e-9);
        segs.extend(cuts.iter().copied());
        let arr = arrange(&segs, &[], outer, holes, 1e-9);
        assert!(arr
            .faces
            .iter()
            .all(|f| is_convex(&arr, &f.outer, f.holes.len(), 1e-9)));
        (cuts, arr.faces.len())
    }

    #[test]
    fn l_shape_is_cut_along_its_x_edge() {
        let l = [
            [0.0, 0.0],
            [6.0, 0.0],
            [6.0, 3.0],
            [3.0, 3.0],
            [3.0, 6.0],
            [0.0, 6.0],
        ];
        let (cuts, faces) = cut_once(&l, &[]);
        assert_eq!(cuts, vec![[[3.0, 3.0], [0.0, 3.0]]]);
        assert_eq!(faces, 2);
    }

    #[test]
    fn ring_around_an_opening_becomes_four_rectangles() {
        let outer = [[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]];
        let hole = vec![[2.0, 2.0], [4.0, 2.0], [4.0, 4.0], [2.0, 4.0]];
        let (cuts, faces) = cut_once(&outer, &[hole]);
        assert_eq!(cuts.len(), 4);
        assert_eq!(faces, 4);
    }

    #[test]
    fn collinear_points_are_not_corners() {
        let p = [[0.0, 0.0], [3.0, 0.0], [6.0, 0.0], [6.0, 3.0], [0.0, 3.0]];
        assert_eq!(corners(&p, 1e-9), vec![0, 2, 3, 4]);
    }
}
