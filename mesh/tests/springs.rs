//! Line and area springs through the mesh step: a mat drawn as one area and meshed by the mesh step gives
//! the same results as the same elements drawn as 36 areas (each element's corners get their shares), and
//! a beam the mat's mesh cuts into pieces keeps its line spring on every piece.

use serde_json::{json, Value};

/// A 6 x 6 m mat at z = 0.5 on an area spring, with a beam along y = 3 on a line spring, 100 down at the
/// middle. `meshed`: one area meshed at 1 m; otherwise 36 areas of 1 x 1 m and six beam objects.
fn mat(meshed: bool) -> Value {
    let id = |i: usize, j: usize| format!("{i}_{j}");
    let node = |i: usize, j: usize| json!({"id": id(i, j), "x": i as f64, "y": j as f64, "z": 0.5});
    let (nodes, areas, frames, mesh): (Vec<Value>, Vec<Value>, Vec<Value>, Value) = if meshed {
        let corners = [(0, 0), (6, 0), (6, 6), (0, 6), (0, 3), (6, 3), (3, 3)];
        (
            corners.iter().map(|&(i, j)| node(i, j)).collect(),
            vec![
                json!({"id": "M", "nodes": [id(0, 0), id(6, 0), id(6, 6), id(0, 6)], "section": "SL", "area_spring": "AS"}),
            ],
            vec![
                json!({"id": "B", "type": "frame", "nodes": [id(0, 3), id(6, 3)], "material": "C", "section": "R", "line_spring": "LS"}),
            ],
            json!({"floor": "rectangular", "floor_size": 1.0}),
        )
    } else {
        let mut nodes = Vec::new();
        for i in 0..=6 {
            for j in 0..=6 {
                nodes.push(node(i, j));
            }
        }
        let areas = (0..6)
            .flat_map(|i| (0..6).map(move |j| (i, j)))
            .map(|(i, j)| json!({"id": format!("A{i}{j}"), "nodes": [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)], "section": "SL", "area_spring": "AS"}))
            .collect();
        let frames = (0..6)
            .map(|i| json!({"id": format!("B{i}"), "type": "frame", "nodes": [id(i, 3), id(i + 1, 3)], "material": "C", "section": "R", "line_spring": "LS"}))
            .collect();
        (nodes, areas, frames, json!("off"))
    };
    json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
        "sections": [{"id": "R", "A": 0.18, "I22": 0.00135, "I33": 0.0054, "J": 0.0037, "As2": 0.15, "As3": 0.15, "depth": 0.6, "width": 0.3}],
        "area_sections": [{"id": "SL", "type": "shell-thin", "material": "C", "thickness": 0.2}],
        "line_spring_props": [{"id": "LS", "u1": 2000.0, "u2": 3000.0, "u3": 4000.0, "r1": 500.0}],
        "area_spring_props": [{"id": "AS", "u1": 1000.0, "u2": 1000.0, "u3": 20000.0}],
        "nodes": nodes, "frames": frames, "areas": areas, "mesh": mesh,
        "load_cases": [{"id": "P", "node_loads": [{"node": id(3, 3), "fz": -100.0, "fx": 5.0}]}],
    })
}

#[test]
fn a_meshed_mat_and_a_split_beam_keep_their_springs() {
    let run = |m: Value| osab_mesh::api::analyze_value(m).unwrap();
    let (a, b) = (run(mat(true)), run(mat(false)));
    let (ca, cb) = (&a.cases[0], &b.cases[0]);
    assert!(ca.equilibrium_ok() && cb.equilibrium_ok());
    let scale = cb
        .displacements
        .iter()
        .flat_map(|d| d.values.as_array())
        .fold(0.0f64, |m, v| m.max(v.abs()));
    // The object joints of the meshed mat are joints of the drawn one, at the same places.
    for id in ["0_0", "6_0", "6_6", "0_6", "0_3", "6_3", "3_3"] {
        let (x, y) = (
            ca.displacement(&id.into()).unwrap(),
            cb.displacement(&id.into()).unwrap(),
        );
        for (p, q) in x.as_array().iter().zip(y.as_array()) {
            assert!((p - q).abs() <= 1e-9 * scale, "{id}: {x:?} vs {y:?}");
        }
    }
    // Every spring force taken together carries the loads.
    let total: f64 = ca.reactions.iter().map(|r| r.values.as_array()[2]).sum();
    assert!((total - 100.0).abs() < 1e-9, "{total}");
}
