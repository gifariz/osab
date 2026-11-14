//! Planar arrangement: an area's edges and the lines inside it (beams, walls, extension lines) are split
//! where they cross or touch, and the faces between them are found. Each face is meshed on its own.

use std::collections::{HashMap, HashSet};

use crate::boxes::{self, Boxes};
use crate::geom::{self, P2};

/// A face of the arrangement: its outer boundary (counterclockwise, vertex indices) and holes
/// (clockwise). Lines ending inside the face appear in the walk twice (there and back).
#[derive(Debug, Clone)]
pub struct Face {
    pub outer: Vec<usize>,
    pub holes: Vec<Vec<usize>>,
    /// A point inside the face (near its longest edge).
    pub inside: P2,
}

pub struct Arrangement {
    pub verts: Vec<P2>,
    pub faces: Vec<Face>,
}

/// Splits `segs` at their crossings and at `points` lying on them, and finds the bounded faces that lie
/// inside `region` (a counterclockwise outer polygon) and outside every polygon in `holes`.
pub fn arrange(
    segs: &[[P2; 2]],
    points: &[P2],
    region: &[P2],
    holes: &[Vec<P2>],
    tol: f64,
) -> Arrangement {
    // Vertices: merged within tol (into the first vertex within tol), found through cells of 4 tol.
    let mut verts: Vec<P2> = Vec::new();
    let cell = if tol > 0.0 { 4.0 * tol } else { 1.0 };
    let key = |p: P2| p.map(|v| (v / cell).floor() as i64);
    let mut cells: HashMap<[i64; 2], Vec<usize>> = HashMap::new();
    let mut vert = |p: P2, verts: &mut Vec<P2>| -> usize {
        let k = key(p);
        let mut first: Option<usize> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &i in cells.get(&[k[0] + dx, k[1] + dy]).into_iter().flatten() {
                    if geom::dist(verts[i], p) <= tol && first.is_none_or(|f| i < f) {
                        first = Some(i);
                    }
                }
            }
        }
        first.unwrap_or_else(|| {
            verts.push(p);
            cells.entry(k).or_default().push(verts.len() - 1);
            verts.len() - 1
        })
    };
    // Split parameters of every segment. Only segments (and points) whose boxes, grown by twice the
    // tolerance, meet can cross or touch, so only those are compared.
    let flat = |p: P2| [p[0], p[1], 0.0];
    let seg_boxes = Boxes::new(
        segs.iter()
            .map(|s| boxes::around(s.map(flat), 2.0 * tol))
            .collect(),
    );
    let point_boxes = Boxes::new(points.iter().map(|p| [flat(*p); 2]).collect());
    let mut cuts: Vec<Vec<f64>> = segs.iter().map(|_| vec![0.0, 1.0]).collect();
    for i in 0..segs.len() {
        for j in seg_boxes.near(i).into_iter().filter(|&j| j > i) {
            let ([a, b], [c, d]) = (segs[i], segs[j]);
            // Segments within the tolerance of one line (both ends of one on the other's line) are
            // collinear: their crossing point would be anywhere along the overlap.
            let collinear = (line_dist(a, c, d) <= tol && line_dist(b, c, d) <= tol)
                || (line_dist(c, a, b) <= tol && line_dist(d, a, b) <= tol);
            let cross = if collinear {
                None
            } else {
                geom::seg_cross(a, b, c, d, tol)
            };
            if let Some((t, u)) = cross {
                cuts[i].push(t.clamp(0.0, 1.0));
                cuts[j].push(u.clamp(0.0, 1.0));
            } else {
                // Parallel: overlapping collinear segments split each other at their ends.
                for (k, [p, q], others) in [(i, segs[i], segs[j]), (j, segs[j], segs[i])] {
                    for e in others {
                        if geom::seg_dist(e, p, q) <= tol {
                            let d = geom::sub(q, p);
                            cuts[k].push(
                                (geom::dot(geom::sub(e, p), d) / geom::dot(d, d)).clamp(0.0, 1.0),
                            );
                        }
                    }
                }
            }
        }
        let [a, b] = segs[i];
        let around_i = boxes::around(segs[i].map(flat), 2.0 * tol);
        for p in point_boxes
            .meeting(&around_i)
            .into_iter()
            .map(|k| points[k])
        {
            if geom::seg_dist(p, a, b) <= tol {
                let d = geom::sub(b, a);
                cuts[i].push((geom::dot(geom::sub(p, a), d) / geom::dot(d, d)).clamp(0.0, 1.0));
            }
        }
    }
    let mut edges: Vec<[usize; 2]> = Vec::new();
    let mut have: HashSet<[usize; 2]> = HashSet::new();
    for (s, c) in segs.iter().zip(cuts.iter_mut()) {
        c.sort_by(|a, b| a.total_cmp(b));
        let ids: Vec<usize> = c
            .iter()
            .map(|&t| vert(geom::lerp(s[0], s[1], t), &mut verts))
            .collect();
        for w in ids.windows(2) {
            let (a, b) = (w[0].min(w[1]), w[0].max(w[1]));
            if a != b && have.insert([a, b]) {
                edges.push([a, b]);
            }
        }
    }

    // Half-edges: 2e = a->b, 2e+1 = b->a. Outgoing half-edges at each vertex sorted by angle.
    let n = verts.len();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    let from = |h: usize| {
        if h.is_multiple_of(2) {
            edges[h / 2][0]
        } else {
            edges[h / 2][1]
        }
    };
    let to = |h: usize| {
        if h.is_multiple_of(2) {
            edges[h / 2][1]
        } else {
            edges[h / 2][0]
        }
    };
    for h in 0..2 * edges.len() {
        out[from(h)].push(h);
    }
    let ang = |h: usize| {
        let d = geom::sub(verts[to(h)], verts[from(h)]);
        d[1].atan2(d[0])
    };
    for o in &mut out {
        o.sort_by(|&a, &b| ang(a).total_cmp(&ang(b)));
    }
    // Next half-edge of the face on the left: at the end vertex, the outgoing edge just clockwise of the
    // way back.
    let next = |h: usize| {
        let v = to(h);
        let back = h ^ 1;
        let o = &out[v];
        let k = o.iter().position(|&x| x == back).expect("twin");
        o[(k + o.len() - 1) % o.len()]
    };
    let mut seen = vec![false; 2 * edges.len()];
    let mut cycles: Vec<Vec<usize>> = Vec::new();
    for h0 in 0..2 * edges.len() {
        if seen[h0] {
            continue;
        }
        let mut cyc = Vec::new();
        let mut h = h0;
        while !seen[h] {
            seen[h] = true;
            cyc.push(from(h));
            h = next(h);
        }
        cycles.push(cyc);
    }
    let pts = |c: &[usize]| c.iter().map(|&i| verts[i]).collect::<Vec<P2>>();
    // A point safely inside a counterclockwise cycle: near the middle of its longest edge, a little to
    // the left (the inside).
    let probe = |c: &[usize]| -> P2 {
        let p = pts(c);
        let k = (0..p.len())
            .max_by(|&a, &b| {
                geom::dist(p[a], p[(a + 1) % p.len()])
                    .total_cmp(&geom::dist(p[b], p[(b + 1) % p.len()]))
            })
            .unwrap();
        let (a, b) = (p[k], p[(k + 1) % p.len()]);
        let d = geom::sub(b, a);
        let l = geom::norm(d);
        let left = [-d[1] / l, d[0] / l];
        geom::add(
            geom::lerp(a, b, 0.5),
            geom::scale(left, (10.0 * tol).min(0.01 * l)),
        )
    };
    let inside_region =
        |p: P2| geom::inside(p, region, 0.0) && holes.iter().all(|h| !geom::inside(p, h, 0.0));
    let mut faces: Vec<Face> = cycles
        .iter()
        .filter(|c| c.len() >= 3 && geom::area(&pts(c)) > 0.0 && inside_region(probe(c)))
        .map(|c| Face {
            outer: c.clone(),
            holes: Vec::new(),
            inside: probe(c),
        })
        .collect();
    // Clockwise cycles inside a face, not touching its outer boundary: holes of the smallest face around.
    for c in cycles.iter().filter(|c| geom::area(&pts(c)) < 0.0) {
        let p = verts[c[0]];
        let host = faces
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.outer.contains(&c[0]) && geom::inside(p, &pts(&f.outer), 0.0))
            .min_by(|a, b| geom::area(&pts(&a.1.outer)).total_cmp(&geom::area(&pts(&b.1.outer))))
            .map(|(i, _)| i);
        if let Some(i) = host {
            faces[i].holes.push(c.clone());
        }
    }
    Arrangement { verts, faces }
}

/// Distance of `p` from the line through `a` and `b` (from `a` when they coincide).
fn line_dist(p: P2, a: P2, b: P2) -> f64 {
    let d = geom::sub(b, a);
    let l = geom::norm(d);
    if l == 0.0 {
        return geom::dist(p, a);
    }
    geom::cross(d, geom::sub(p, a)).abs() / l
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, s: f64) -> Vec<[P2; 2]> {
        let p = [[x0, y0], [x0 + s, y0], [x0 + s, y0 + s], [x0, y0 + s]];
        (0..4).map(|k| [p[k], p[(k + 1) % 4]]).collect()
    }

    #[test]
    fn a_line_across_a_square_makes_two_faces() {
        let mut segs = square(0.0, 0.0, 4.0);
        segs.push([[1.5, -1.0], [1.5, 5.0]]);
        let region = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let a = arrange(&segs, &[], &region, &[], 1e-9);
        assert_eq!(a.faces.len(), 2);
        let areas: Vec<f64> = a
            .faces
            .iter()
            .map(|f| geom::area(&f.outer.iter().map(|&i| a.verts[i]).collect::<Vec<_>>()))
            .collect();
        assert!(areas.iter().any(|x| (x - 6.0).abs() < 1e-9));
        assert!(areas.iter().any(|x| (x - 10.0).abs() < 1e-9));
    }

    #[test]
    fn an_opening_becomes_a_hole_and_a_dangling_line_stays_in_the_face() {
        let mut segs = square(0.0, 0.0, 6.0);
        segs.extend(square(2.0, 2.0, 1.0));
        segs.push([[0.0, 5.0], [1.0, 5.0]]);
        let region = [[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]];
        let hole = vec![[2.0, 2.0], [3.0, 2.0], [3.0, 3.0], [2.0, 3.0]];
        let a = arrange(&segs, &[], &region, &[hole], 1e-9);
        assert_eq!(a.faces.len(), 1);
        assert_eq!(a.faces[0].holes.len(), 1);
        // The dangling line adds its far end and walks there and back: 4 corners + (5, 0, 5 back).
        assert_eq!(a.faces[0].outer.len(), 4 + 3);
    }
}
