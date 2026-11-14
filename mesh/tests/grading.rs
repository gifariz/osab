//! Grading (`"refine"`, on by default): in the irregular regions of a General floor mesh the elements
//! are half the size at re-entrant and opening corners and grow back to the full size over two
//! elements; four-sided regions keep the equal grid, and the Rectangular mesh does not change.

use std::collections::BTreeSet;

use osab_solver::Model;
use serde_json::{json, Value};

type P = [f64; 2];

/// A floor at z = 3 with these corners and openings, fixed at its corners, 10 kN/m² down.
fn floor(corners: &[P], holes: &[&[P]], mesh: Value) -> Model {
    let mut nodes = Vec::new();
    let mut areas = Vec::new();
    let mut add = |id: &str, pts: &[P], extra: Value| {
        let ids: Vec<String> = (0..pts.len()).map(|k| format!("{id}_{k}")).collect();
        for (n, p) in ids.iter().zip(pts) {
            nodes.push(json!({"id": n, "x": p[0], "y": p[1], "z": 3}));
        }
        let mut a = json!({"id": id, "nodes": ids});
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        areas.push(a);
    };
    add("F", corners, json!({"section": "S"}));
    for (k, h) in holes.iter().enumerate() {
        add(&format!("O{k}"), h, json!({"opening": true}));
    }
    let supports: Vec<Value> = (0..corners.len())
        .map(|k| json!({"node": format!("F_{k}"), "fix": "fixed"}))
        .collect();
    serde_json::from_value(json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}], "sections": [],
        "area_sections": [{"id": "S", "type": "shell-thin", "material": "C", "thickness": 0.2}],
        "nodes": nodes, "areas": areas, "supports": supports, "mesh": mesh,
        "load_cases": [{"id": "Q", "area_loads": [{"area": "F", "dir": "Z", "q": -10.0}]}]
    }))
    .unwrap()
}

struct Elem {
    pts: Vec<P>,
}

impl Elem {
    fn centroid(&self) -> P {
        let n = self.pts.len() as f64;
        let s = self
            .pts
            .iter()
            .fold([0.0, 0.0], |a, p| [a[0] + p[0], a[1] + p[1]]);
        [s[0] / n, s[1] / n]
    }
    fn area(&self) -> f64 {
        let p = &self.pts;
        (0..p.len())
            .map(|k| p[k][0] * p[(k + 1) % p.len()][1] - p[(k + 1) % p.len()][0] * p[k][1])
            .sum::<f64>()
            .abs()
            / 2.0
    }
    /// Size: the square root of the area (quads), or of twice the area (triangles).
    fn size(&self) -> f64 {
        (self.area() * if self.pts.len() == 3 { 2.0 } else { 1.0 }).sqrt()
    }
    fn angles(&self) -> Vec<f64> {
        let p = &self.pts;
        let n = p.len();
        (0..n)
            .map(|k| {
                let a = [
                    p[(k + n - 1) % n][0] - p[k][0],
                    p[(k + n - 1) % n][1] - p[k][1],
                ];
                let b = [p[(k + 1) % n][0] - p[k][0], p[(k + 1) % n][1] - p[k][1]];
                (a[0] * b[1] - a[1] * b[0])
                    .abs()
                    .atan2(a[0] * b[0] + a[1] * b[1])
                    .to_degrees()
            })
            .collect()
    }
    /// Longest side over shortest side.
    fn aspect(&self) -> f64 {
        let p = &self.pts;
        let n = p.len();
        let sides: Vec<f64> = (0..n)
            .map(|k| {
                let (a, b) = (p[k], p[(k + 1) % n]);
                ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
            })
            .collect();
        sides.iter().cloned().fold(0.0, f64::max) / sides.iter().cloned().fold(f64::MAX, f64::min)
    }
}

fn elements(m: &Model) -> Vec<Elem> {
    let r = osab_mesh::mesh(m).unwrap();
    let at: std::collections::HashMap<&str, P> = r
        .model
        .nodes
        .iter()
        .map(|n| (n.id.0.as_str(), [n.x, n.y]))
        .collect();
    r.model
        .areas
        .iter()
        .map(|a| Elem {
            pts: a.nodes.iter().map(|n| at[n.0.as_str()]).collect(),
        })
        .collect()
}

fn dist(a: P, b: P) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Mean size of the elements with a corner at a point.
fn size_at(e: &[Elem], p: P) -> f64 {
    let at: Vec<f64> = e
        .iter()
        .filter(|x| x.pts.iter().any(|q| dist(*q, p) < 1e-6))
        .map(|x| x.size())
        .collect();
    assert!(!at.is_empty());
    at.iter().sum::<f64>() / at.len() as f64
}

/// Mean element size within `r` of a point.
fn size_near(e: &[Elem], p: P, r: f64) -> f64 {
    let near: Vec<f64> = e
        .iter()
        .filter(|x| dist(x.centroid(), p) < r)
        .map(|x| x.size())
        .collect();
    assert!(!near.is_empty());
    near.iter().sum::<f64>() / near.len() as f64
}

const L: [P; 6] = [
    [0.0, 0.0],
    [8.0, 0.0],
    [8.0, 4.0],
    [4.0, 4.0],
    [4.0, 8.0],
    [0.0, 8.0],
];
const HOLE: [P; 4] = [[1.0, 5.0], [2.0, 5.0], [2.0, 7.0], [1.0, 7.0]];
/// B36's floor: a skew edge and an opening near the re-entrant corner.
const SKEW: [P; 6] = [
    [0.0, 0.0],
    [9.0, 0.0],
    [9.0, 3.0],
    [4.0, 3.5],
    [3.5, 8.0],
    [0.0, 8.0],
];
const SKEW_HOLE: [P; 4] = [[2.0, 2.0], [3.0, 2.0], [3.0, 2.8], [2.0, 2.8]];

#[test]
fn elements_are_half_size_at_the_corners_and_full_size_away_from_them() {
    let on = elements(&floor(&L, &[&HOLE], json!({"floor_size": 1.0})));
    let off = elements(&floor(
        &L,
        &[&HOLE],
        json!({"floor_size": 1.0, "refine": false}),
    ));
    assert!(on.len() > off.len(), "{} vs {}", on.len(), off.len());
    for corner in [[4.0, 4.0], [1.0, 5.0], [2.0, 7.0]] {
        let (s, s_off) = (size_at(&on, corner), size_at(&off, corner));
        println!("at {corner:?}: {s:.2} graded, {s_off:.2} not");
        assert!(
            s < 0.7 && s < 0.8 * s_off,
            "size {s} ({s_off} without grading) at {corner:?}"
        );
    }
    // Far from the corners: the full size.
    let far = size_near(&on, [6.5, 1.0], 1.0);
    assert!((far - 1.0).abs() < 0.15, "size {far}");
}

#[test]
fn graded_meshes_keep_good_elements_and_cover_the_floor() {
    let shapes: [(&[P], Vec<&[P]>); 4] = [
        (&L, vec![&HOLE]),
        (&SKEW, vec![&SKEW_HOLE]),
        // T shape.
        (
            &[
                [0.0, 0.0],
                [3.0, 0.0],
                [3.0, 4.0],
                [6.0, 4.0],
                [6.0, 6.0],
                [-3.0, 6.0],
                [-3.0, 4.0],
                [0.0, 4.0],
            ],
            vec![],
        ),
        // Many-sided polygon with an opening near an edge.
        (
            &[
                [0.0, 0.0],
                [5.0, -1.0],
                [8.0, 1.0],
                [9.0, 5.0],
                [6.0, 8.0],
                [2.0, 7.5],
                [-1.0, 4.0],
            ],
            vec![&[[6.5, 2.0], [7.5, 2.2], [7.3, 3.2], [6.4, 3.0]]],
        ),
    ];
    for (corners, holes) in &shapes {
        let m = floor(corners, holes, json!({"floor_size": 1.0}));
        let e = elements(&m);
        let poly = |p: &[P]| {
            (0..p.len())
                .map(|k| p[k][0] * p[(k + 1) % p.len()][1] - p[(k + 1) % p.len()][0] * p[k][1])
                .sum::<f64>()
                .abs()
                / 2.0
        };
        let expected = poly(corners) - holes.iter().map(|h| poly(h)).sum::<f64>();
        let total: f64 = e.iter().map(|x| x.area()).sum();
        assert!(
            (total - expected).abs() < 1e-6,
            "area {total} vs {expected}"
        );
        let (mut lo, mut hi, mut asp) = (180.0f64, 0.0f64, 0.0f64);
        for x in &e {
            for a in x.angles() {
                lo = lo.min(a);
                hi = hi.max(a);
            }
            asp = asp.max(x.aspect());
        }
        let poor = e
            .iter()
            .filter(|x| x.angles().iter().any(|&a| !(45.0..=135.0).contains(&a)))
            .count();
        println!(
            "{} elements ({poor} outside 45-135), angles {lo:.1}..{hi:.1}, aspect {asp:.2}",
            e.len()
        );
        // The manual's guideline (10.2.1): corner angles near 90 degrees (45-135 best), aspect ratio
        // below 4. The 2:1 size transitions leave a few elements a little outside the best range.
        assert!(
            lo > 30.0 && hi < 150.0 && asp < 3.0,
            "angles {lo}..{hi}, aspect {asp}"
        );
        assert!(
            poor * 100 <= e.len() * 15,
            "{poor} of {} outside 45-135",
            e.len()
        );
        // Same model, same mesh.
        let again = elements(&m);
        assert!(e.iter().zip(&again).all(|(a, b)| a.pts == b.pts));
    }
}

/// Elements as sets of rounded corner coordinates.
fn element_set(e: &[Elem]) -> BTreeSet<Vec<(i64, i64)>> {
    e.iter()
        .map(|x| {
            let mut c: Vec<(i64, i64)> = x
                .pts
                .iter()
                .map(|p| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64))
                .collect();
            c.sort_unstable();
            c
        })
        .collect()
}

#[test]
fn four_sided_regions_and_the_rectangular_mesh_do_not_change() {
    // A rectangle with no opening: the equal grid whatever the option.
    let rect = [[0.0, 0.0], [7.0, 0.0], [7.0, 5.0], [0.0, 5.0]];
    let on = element_set(&elements(&floor(&rect, &[], json!({"floor_size": 1.0}))));
    let off = element_set(&elements(&floor(
        &rect,
        &[],
        json!({"floor_size": 1.0, "refine": false}),
    )));
    assert_eq!(on, off);
    // Rectangular mesh: the option is not used.
    let r = |refine: bool| {
        element_set(&elements(&floor(
            &L,
            &[&HOLE],
            json!({"floor": "rectangular", "floor_size": 1.0, "refine": refine}),
        )))
    };
    assert_eq!(r(true), r(false));
    // B42's L floor on beams with the General mesh: the beams cut it into three 4 x 4 m squares; the two
    // without the opening keep their equal grid, the one with it is graded.
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/benchmarks/b42_l_floor_beams/model.json"),
    )
    .unwrap();
    let mut v: Value = serde_json::from_str(&text).unwrap();
    let run = |v: &mut Value, refine: bool| {
        v["mesh"] = json!({"floor": "general", "floor_size": 1.0, "refine": refine});
        elements(&serde_json::from_value(v.clone()).unwrap())
    };
    let (on, off) = (run(&mut v, true), run(&mut v, false));
    let outside = |e: &[Elem]| {
        element_set(
            &e.iter()
                .filter(|x| {
                    let c = x.centroid();
                    !(c[0] < 4.0 && c[1] > 4.0)
                })
                .map(|x| Elem { pts: x.pts.clone() })
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(outside(&on), outside(&off));
    assert_eq!(outside(&on).len(), 32);
    assert!(on.len() > off.len());
}
