//! Quad-dominant mesh of an irregular face: a square lattice of spacing h inside the face, a constrained
//! Delaunay triangulation of the lattice, the boundary and fixed points, triangles paired into quads
//! (best pairs first), then smoothing that keeps the boundary. Written from the textbook methods
//! (Bowyer-Watson insertion, edge recovery by flips, greedy pairing, Laplacian smoothing).

use std::collections::HashMap;

use crate::geom::{self, P2};
use crate::grade;

/// What to mesh: `pts` (boundary and fixed points; they stay where they are), the edges that must
/// appear (`fixed_edges`, index pairs), the face as polygons (`outer` counterclockwise, `holes`), the
/// target size and the lattice's origin (its rows and columns follow the area's local axes).
pub struct Input<'a> {
    pub pts: Vec<P2>,
    pub fixed_edges: Vec<[usize; 2]>,
    pub outer: &'a [P2],
    pub holes: &'a [Vec<P2>],
    pub h: f64,
    pub origin: P2,
    /// Grading (`crate::grade`): the lattice gets half spacing near these corners. Empty: none.
    pub hot: &'a [P2],
}

/// The mesh: all points (the given ones first, unchanged) and elements (3 or 4 indices, counterclockwise).
pub struct Output {
    pub pts: Vec<P2>,
    pub elems: Vec<Vec<usize>>,
}

/// Pairs need corner angles within this many degrees of 90 to become a quad.
const MAX_SKEW: f64 = 60.0;
const SMOOTH_PASSES: usize = 8;

pub fn mesh(inp: &Input) -> Result<Output, String> {
    let (mut pts, tris) = triangles(inp)?;
    let nfixed = inp.pts.len();
    let mut elems = pair_into_quads(&pts, &tris, &inp.fixed_edges);
    smooth(&mut pts, &mut elems, nfixed);
    Ok(Output { pts, elems })
}

/// The constrained Delaunay triangles of the lattice, the boundary and the fixed points (counterclockwise),
/// before they are paired into quads: for a field solved on triangles (a section's torsion).
pub fn triangles(inp: &Input) -> Result<(Vec<P2>, Vec<[usize; 3]>), String> {
    let nfixed = inp.pts.len();
    let mut pts = inp.pts.clone();
    let inside_face = |p: P2| {
        geom::inside(p, inp.outer, 0.0) && inp.holes.iter().all(|h| !geom::inside(p, h, 0.0))
    };
    // Lattice points well inside the face and away from the fixed points and edges.
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in inp.outer {
        for d in 0..2 {
            lo[d] = lo[d].min(p[d]);
            hi[d] = hi[d].max(p[d]);
        }
    }
    let h = inp.h;
    let field = grade::Field { h, hot: inp.hot };
    // With grading the lattice has half spacing; its in-between points are kept only where the size
    // field is below 3/4 of the full size.
    let sub = if inp.hot.is_empty() { 1 } else { 2 };
    let step = h / sub as f64;
    let (i0, i1) = (
        ((lo[0] - inp.origin[0]) / h).floor() as i64 * sub,
        ((hi[0] - inp.origin[0]) / h).ceil() as i64 * sub,
    );
    let (j0, j1) = (
        ((lo[1] - inp.origin[1]) / h).floor() as i64 * sub,
        ((hi[1] - inp.origin[1]) / h).ceil() as i64 * sub,
    );
    for i in i0..=i1 {
        for j in j0..=j1 {
            let p = [
                inp.origin[0] + i as f64 * step,
                inp.origin[1] + j as f64 * step,
            ];
            let size = field.size(p);
            if (i % sub != 0 || j % sub != 0) && size >= 0.75 * h {
                continue;
            }
            if !inside_face(p) {
                continue;
            }
            let gap = 0.6 * size;
            let near_edge = inp
                .fixed_edges
                .iter()
                .any(|e| geom::seg_dist(p, inp.pts[e[0]], inp.pts[e[1]]) < gap);
            let near_point = pts[..nfixed].iter().any(|q| geom::dist(*q, p) < gap);
            if !near_edge && !near_point {
                pts.push(p);
            }
        }
    }
    // The edges are recovered while the enclosing triangle is still there: next to boundary points that
    // are nearly in line, the triangulation's own border can miss an edge between them, and only flips
    // of triangles at the enclosing points bring it in.
    let (all, mut tris) = delaunay(&pts)?;
    recover_edges(&all, &mut tris, &inp.fixed_edges)?;
    tris.retain(|t| t.iter().all(|&v| v < pts.len()));
    tris.retain(|t| {
        let c = geom::scale(
            geom::add(geom::add(pts[t[0]], pts[t[1]]), pts[t[2]]),
            1.0 / 3.0,
        );
        inside_face(c)
    });
    if tris.is_empty() {
        return Err("the face is too thin to mesh".into());
    }
    Ok((pts, tris))
}

fn incircle(a: P2, b: P2, c: P2, p: P2) -> f64 {
    let (ax, ay) = (a[0] - p[0], a[1] - p[1]);
    let (bx, by) = (b[0] - p[0], b[1] - p[1]);
    let (cx, cy) = (c[0] - p[0], c[1] - p[1]);
    (ax * ax + ay * ay) * (bx * cy - cx * by) - (bx * bx + by * by) * (ax * cy - cx * ay)
        + (cx * cx + cy * cy) * (ax * by - bx * ay)
}

/// Bowyer-Watson Delaunay triangulation (counterclockwise triangles) of the points and an enclosing
/// triangle: returns the points with the enclosing triangle's three corners appended, and the triangles,
/// those at the three corners included.
fn delaunay(pts: &[P2]) -> Result<(Vec<P2>, Vec<[usize; 3]>), String> {
    let n = pts.len();
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in pts {
        for d in 0..2 {
            lo[d] = lo[d].min(p[d]);
            hi[d] = hi[d].max(p[d]);
        }
    }
    let size = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-12);
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let mut all = pts.to_vec();
    all.push([mid[0] - 20.0 * size, mid[1] - 10.0 * size]);
    all.push([mid[0] + 20.0 * size, mid[1] - 10.0 * size]);
    all.push([mid[0], mid[1] + 20.0 * size]);
    let mut tris: Vec<[usize; 3]> = vec![[n, n + 1, n + 2]];
    let eps = 1e-12 * size.powi(4);
    for i in 0..n {
        let p = all[i];
        let mut bad = Vec::new();
        for (k, t) in tris.iter().enumerate() {
            if incircle(all[t[0]], all[t[1]], all[t[2]], p) > eps {
                bad.push(k);
            }
        }
        if bad.is_empty() {
            return Err("triangulation failed (a point outside every triangle)".into());
        }
        let mut count: HashMap<(usize, usize), usize> = HashMap::new();
        for &k in &bad {
            let t = tris[k];
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let mut cavity = Vec::new();
        for &k in &bad {
            let t = tris[k];
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                if count[&(a.min(b), a.max(b))] == 1 {
                    cavity.push([a, b]);
                }
            }
        }
        bad.sort_unstable();
        for &k in bad.iter().rev() {
            tris.swap_remove(k);
        }
        for [a, b] in cavity {
            if geom::orient(all[a], all[b], p) <= 0.0 {
                return Err("triangulation failed (points too close together)".into());
            }
            tris.push([a, b, i]);
        }
    }
    Ok((all, tris))
}

fn key(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn edge_map(tris: &[[usize; 3]]) -> HashMap<(usize, usize), Vec<usize>> {
    let mut m: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (k, t) in tris.iter().enumerate() {
        for e in 0..3 {
            m.entry(key(t[e], t[(e + 1) % 3])).or_default().push(k);
        }
    }
    m
}

/// Makes every fixed edge an edge of the triangulation by flipping the edges that cross it (the first
/// flippable one in edge order, again until the edge appears). The edge map is kept up to date at each
/// flip.
fn recover_edges(pts: &[P2], tris: &mut [[usize; 3]], fixed: &[[usize; 2]]) -> Result<(), String> {
    let mut m = edge_map(tris);
    for &[a, b] in fixed {
        for _round in 0..10_000 {
            if m.contains_key(&key(a, b)) {
                break;
            }
            // Edges that properly cross a-b.
            let mut crossing: Vec<(usize, usize)> = m
                .keys()
                .copied()
                .filter(|&(c, d)| {
                    c != a && c != b && d != a && d != b && {
                        let (pa, pb, pc, pd) = (pts[a], pts[b], pts[c], pts[d]);
                        geom::orient(pa, pb, pc) * geom::orient(pa, pb, pd) < 0.0
                            && geom::orient(pc, pd, pa) * geom::orient(pc, pd, pb) < 0.0
                    }
                })
                .collect();
            if crossing.is_empty() {
                return Err(format!(
                    "could not recover a boundary edge near ({:.3}, {:.3})",
                    pts[a][0], pts[a][1]
                ));
            }
            crossing.sort_unstable();
            let mut flipped = false;
            for (c, d) in crossing {
                let ts = &m[&(c, d)];
                if ts.len() != 2 {
                    continue;
                }
                let (t1, t2) = (ts[0], ts[1]);
                let e = *tris[t1].iter().find(|&&v| v != c && v != d).unwrap();
                let f = *tris[t2].iter().find(|&&v| v != c && v != d).unwrap();
                // Convex quad c-e-d-f: the new diagonal e-f must cross c-d.
                let (pc, pd, pe, pf) = (pts[c], pts[d], pts[e], pts[f]);
                if geom::orient(pe, pf, pc) * geom::orient(pe, pf, pd) >= 0.0 {
                    continue;
                }
                let ccw = |t: [usize; 3]| {
                    if geom::orient(pts[t[0]], pts[t[1]], pts[t[2]]) > 0.0 {
                        t
                    } else {
                        [t[0], t[2], t[1]]
                    }
                };
                tris[t1] = ccw([e, f, c]);
                tris[t2] = ccw([e, f, d]);
                // c-d becomes e-f; d-e moves from t1 to t2 and c-f from t2 to t1.
                m.remove(&(c, d));
                m.insert(key(e, f), vec![t1, t2]);
                for (edge, from, to) in [(key(d, e), t1, t2), (key(c, f), t2, t1)] {
                    let list = m.get_mut(&edge).expect("edge of a triangle");
                    for t in list.iter_mut().filter(|t| **t == from) {
                        *t = to;
                    }
                    list.sort_unstable();
                }
                flipped = true;
                break;
            }
            if !flipped {
                return Err("could not recover a boundary edge (no flip possible)".into());
            }
        }
    }
    Ok(())
}

/// Corner angles (degrees) of a polygon given counterclockwise.
fn angles(p: &[P2]) -> Vec<f64> {
    let n = p.len();
    (0..n)
        .map(|k| {
            let a = geom::sub(p[(k + n - 1) % n], p[k]);
            let b = geom::sub(p[(k + 1) % n], p[k]);
            geom::cross(b, a).atan2(geom::dot(a, b)).to_degrees()
        })
        .collect()
}

fn convex(p: &[P2]) -> bool {
    let n = p.len();
    (0..n).all(|k| geom::orient(p[k], p[(k + 1) % n], p[(k + 2) % n]) > 0.0)
}

/// Greedy pairing of neighbouring triangles (not across fixed edges) into quads, best first.
fn pair_into_quads(pts: &[P2], tris: &[[usize; 3]], fixed: &[[usize; 2]]) -> Vec<Vec<usize>> {
    let fixed: std::collections::HashSet<(usize, usize)> =
        fixed.iter().map(|e| key(e[0], e[1])).collect();
    let m = edge_map(tris);
    let mut cands: Vec<(f64, usize, usize, [usize; 4])> = Vec::new();
    for (&(c, d), ts) in &m {
        if ts.len() != 2 || fixed.contains(&(c, d)) {
            continue;
        }
        let (t1, t2) = (ts[0], ts[1]);
        // t1 = (c, d, e) in some rotation; the quad goes e -> c -> f -> d around.
        let r = tris[t1];
        let k = (0..3).find(|&k| r[k] != c && r[k] != d).unwrap();
        let (e, u, v) = (r[k], r[(k + 1) % 3], r[(k + 2) % 3]);
        let f = *tris[t2].iter().find(|&&x| x != c && x != d).unwrap();
        let q = [e, u, f, v];
        let p: Vec<P2> = q.iter().map(|&i| pts[i]).collect();
        if !convex(&p) {
            continue;
        }
        let worst = angles(&p)
            .iter()
            .map(|a| (a - 90.0).abs())
            .fold(0.0, f64::max);
        if worst <= MAX_SKEW {
            cands.push((worst, t1, t2, q));
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.3.cmp(&b.3)));
    let mut used = vec![false; tris.len()];
    let mut out = Vec::new();
    for (_, t1, t2, q) in cands {
        if used[t1] || used[t2] {
            continue;
        }
        used[t1] = true;
        used[t2] = true;
        out.push(q.to_vec());
    }
    for (k, t) in tris.iter().enumerate() {
        if !used[k] {
            out.push(t.to_vec());
        }
    }
    out
}

fn valid(pts: &[P2], e: &[usize]) -> bool {
    let p: Vec<P2> = e.iter().map(|&i| pts[i]).collect();
    if p.len() == 3 {
        geom::orient(p[0], p[1], p[2]) > 0.0
    } else {
        convex(&p)
    }
}

/// Laplacian smoothing of the free points (index >= `nfixed`); a move that would spoil an element is
/// not made.
fn smooth(pts: &mut [P2], elems: &mut [Vec<usize>], nfixed: usize) {
    let n = pts.len();
    let mut nbr: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut of: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, e) in elems.iter().enumerate() {
        for i in 0..e.len() {
            let (a, b) = (e[i], e[(i + 1) % e.len()]);
            if !nbr[a].contains(&b) {
                nbr[a].push(b);
            }
            if !nbr[b].contains(&a) {
                nbr[b].push(a);
            }
            of[a].push(k);
        }
    }
    for _ in 0..SMOOTH_PASSES {
        for v in nfixed..n {
            if nbr[v].is_empty() {
                continue;
            }
            let mut c = [0.0; 2];
            for &w in &nbr[v] {
                c = geom::add(c, pts[w]);
            }
            let target = geom::scale(c, 1.0 / nbr[v].len() as f64);
            let old = pts[v];
            pts[v] = target;
            if !of[v].iter().all(|&k| valid(pts, &elems[k])) {
                pts[v] = old;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(outer: &[P2], holes: &[Vec<P2>], h: f64) -> Output {
        // Boundary points every h/2-ish: the polygon's corners split equally.
        let mut pts = Vec::new();
        let mut edges = Vec::new();
        for poly in std::iter::once(outer.to_vec()).chain(holes.iter().cloned()) {
            let start = pts.len();
            let n = poly.len();
            for k in 0..n {
                let (a, b) = (poly[k], poly[(k + 1) % n]);
                let m = (geom::dist(a, b) / h).ceil() as usize;
                for s in 0..m {
                    pts.push(geom::lerp(a, b, s as f64 / m as f64));
                }
            }
            let end = pts.len();
            for i in start..end {
                edges.push([i, if i + 1 == end { start } else { i + 1 }]);
            }
        }
        mesh(&Input {
            pts,
            fixed_edges: edges,
            outer,
            holes,
            h,
            origin: [0.0, 0.0],
            hot: &[],
        })
        .unwrap()
    }

    fn total_area(o: &Output) -> f64 {
        o.elems
            .iter()
            .map(|e| geom::area(&e.iter().map(|&i| o.pts[i]).collect::<Vec<_>>()))
            .sum()
    }

    #[test]
    fn a_long_sliver_with_nearly_straight_edges_is_meshed() {
        // A wedge 26 m long and 6 mm wide at most: its top edge level, its bottom edge tilted, both split
        // every 26/27 m, and a line from the top edge's last point to the bottom edge's last one before
        // the tip. Along the bottom edge the points are in line, and the triangulation's own border missed
        // an edge there (when the enclosing triangle went first).
        let (l, w, n) = (26.0, 0.006, 27);
        let x = |k: usize| -(k as f64) * l / n as f64;
        let mut outer = vec![[0.0, 0.0]];
        outer.extend((0..n - 1).map(|k| [x(k), w]));
        let low = |k: usize| [x(k), w * k as f64 / n as f64];
        outer.push(low(n - 1));
        outer.push([x(n), w]);
        outer.extend((1..n).rev().map(low));
        // The point before the tip is walked twice (there and back): one point, two edges.
        let mut pts: Vec<P2> = Vec::new();
        let ids: Vec<usize> = outer
            .iter()
            .map(|p| {
                pts.iter().position(|q| q == p).unwrap_or_else(|| {
                    pts.push(*p);
                    pts.len() - 1
                })
            })
            .collect();
        let mut edges: Vec<[usize; 2]> = Vec::new();
        for k in 0..ids.len() {
            let e = [ids[k], ids[(k + 1) % ids.len()]];
            if !edges.contains(&e) && !edges.contains(&[e[1], e[0]]) {
                edges.push(e);
            }
        }
        let o = mesh(&Input {
            pts,
            fixed_edges: edges,
            outer: &outer,
            holes: &[],
            h: 1.0,
            origin: [0.0, 0.0],
            hot: &[],
        })
        .unwrap();
        let want = geom::area(&outer);
        assert!(
            (total_area(&o) - want).abs() < 1e-9 * want,
            "{} {want}",
            total_area(&o)
        );
    }

    #[test]
    fn l_shape_is_covered_with_mostly_good_quads() {
        let l = [
            [0.0, 0.0],
            [6.0, 0.0],
            [6.0, 3.0],
            [3.0, 3.0],
            [3.0, 6.0],
            [0.0, 6.0],
        ];
        let o = run(&l, &[], 1.25);
        assert!((total_area(&o) - 27.0).abs() < 1e-9);
        let quads = o.elems.iter().filter(|e| e.len() == 4).count();
        assert!(
            quads * 10 >= o.elems.len() * 7,
            "{quads} quads of {}",
            o.elems.len()
        );
        for e in &o.elems {
            assert!(valid(&o.pts, e));
        }
    }

    #[test]
    fn opening_and_skew_edges() {
        let outer = [[0.0, 0.0], [6.0, 0.0], [5.0, 4.0], [1.5, 4.0]];
        let hole = vec![[2.5, 1.0], [2.5, 2.2], [3.5, 2.2], [3.5, 1.0]];
        let o = run(&outer, &[hole], 1.0);
        let want = (6.0 + 3.5) / 2.0 * 4.0 - 1.2;
        assert!((total_area(&o) - want).abs() < 1e-9);
        for e in &o.elems {
            assert!(valid(&o.pts, e));
        }
    }
}
