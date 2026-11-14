//! Edge constraints (on by default for floors and walls): every joint lying on an
//! edge of a shell element, between its two corners, is tied to that edge.

use std::collections::HashMap;

use osab_solver::axes::Vec3;
use osab_solver::model::{coords, EdgeConstraint, Model};

use crate::geom;

/// Joints lying inside an edge of an area element (within `Units::edge_tie` of it, and that far from its
/// ends), one record per joint and element edge.
pub fn find(m: &Model) -> Vec<EdgeConstraint> {
    if m.areas.is_empty() {
        return Vec::new();
    }
    let xyz: Vec<Vec3> = m.nodes.iter().map(coords).collect();
    let index: HashMap<&osab_solver::Id, usize> = m
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (&n.id, i))
        .collect();
    // Joints in a grid of cells about the size of an element edge.
    let mut edge_len = Vec::new();
    for a in &m.areas {
        for k in 0..a.nodes.len() {
            let (Some(&i), Some(&j)) = (
                index.get(&a.nodes[k]),
                index.get(&a.nodes[(k + 1) % a.nodes.len()]),
            ) else {
                continue;
            };
            edge_len.push(geom::dist3(xyz[i], xyz[j]));
        }
    }
    edge_len.sort_by(|a, b| a.total_cmp(b));
    let cell = edge_len
        .get(edge_len.len() / 2)
        .copied()
        .unwrap_or(1.0)
        .max(m.units.edge_tie());
    let key = |p: Vec3| p.map(|v| (v / cell).floor() as i64);
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for (i, p) in xyz.iter().enumerate() {
        grid.entry(key(*p)).or_default().push(i);
    }
    let tol = m.units.edge_tie();
    let mut out = Vec::new();
    for a in &m.areas {
        if a.opening {
            continue;
        }
        let n = a.nodes.len();
        for k in 0..n {
            let (ida, idb) = (&a.nodes[k], &a.nodes[(k + 1) % n]);
            let (Some(&ia), Some(&ib)) = (index.get(ida), index.get(idb)) else {
                continue;
            };
            let (pa, pb) = (xyz[ia], xyz[ib]);
            let len = geom::dist3(pa, pb);
            let (ka, kb) = (key(pa), key(pb));
            let lo = [0, 1, 2].map(|d| ka[d].min(kb[d]) - 1);
            let hi = [0, 1, 2].map(|d| ka[d].max(kb[d]) + 1);
            let mut found: Vec<(f64, usize)> = Vec::new();
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        let Some(list) = grid.get(&[x, y, z]) else {
                            continue;
                        };
                        for &j in list {
                            if a.nodes.contains(&m.nodes[j].id) {
                                continue;
                            }
                            if let Some(t) = geom::on_segment3(xyz[j], pa, pb, tol) {
                                if t * len > tol && (1.0 - t) * len > tol {
                                    found.push((t, j));
                                }
                            }
                        }
                    }
                }
            }
            found.sort_by(|x, y| x.0.total_cmp(&y.0));
            for (_, j) in found {
                out.push(EdgeConstraint {
                    area: a.id.clone(),
                    edge: [ida.clone(), idb.clone()],
                    node: m.nodes[j].id.clone(),
                });
            }
        }
    }
    out
}
