//! Insertion points through the mesh step: a floor's elements keep its insertion point, and the pieces of
//! a beam the floor's mesh cuts keep the beam's (joint offsets along the beam follow a line from end i to
//! end j). A T-beam cantilever (beam at its top under a floor at its top, both transformed) meshed by the
//! mesh step gives the same results as the same elements drawn as areas and frames with no meshing.

use osab_solver::Model;
use serde_json::{json, Value};

fn rect(id: &str, depth: f64, width: f64) -> Value {
    let area = depth * width;
    json!({"id": id, "A": area, "I33": width * depth.powi(3) / 12.0, "I22": depth * width.powi(3) / 12.0,
           "J": 0.003707, "As2": 5.0 / 6.0 * area, "As3": 5.0 / 6.0 * area, "depth": depth, "width": width})
}

/// The T-beam cantilever along X (4 m), the floor 2 m wide on the beam's line, fixed at x = 0. With
/// `step` it is one floor and one beam meshed at that size; without, the same elements as objects.
fn t_beam(step: Option<f64>, offset: [f64; 3]) -> Value {
    let floor = json!({"point": "top", "transform": true});
    let beam = json!({"point": 8, "transform": true, "offset_i": [0.0, 0.0, 0.0], "offset_j": offset, "global": true});
    let base = |nodes: Vec<Value>,
                frames: Vec<Value>,
                areas: Vec<Value>,
                supports: Vec<Value>,
                mesh: Value| {
        json!({
            "materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
            "sections": [rect("B", 0.6, 0.3)],
            "area_sections": [{"id": "SL", "type": "shell-thin", "material": "C", "thickness": 0.2}],
            "nodes": nodes, "frames": frames, "areas": areas, "supports": supports, "mesh": mesh,
            "load_cases": [{"id": "PZ", "node_loads": [{"node": "tip", "fz": -10.0}]}, {"id": "PX", "node_loads": [{"node": "tip", "fx": 10.0}]}],
        })
    };
    let ys = [-1.0, 0.0, 1.0];
    match step {
        Some(h) => {
            let corner = |id: &str, x: f64, y: f64| json!({"id": id, "x": x, "y": y, "z": 3.0});
            let nodes = vec![
                corner("a", 0.0, -1.0),
                corner("b", 4.0, -1.0),
                corner("c", 4.0, 1.0),
                corner("d", 0.0, 1.0),
                corner("root", 0.0, 0.0),
                corner("tip", 4.0, 0.0),
            ];
            let frames = vec![
                json!({"id": "B", "type": "frame", "nodes": ["root", "tip"], "material": "C", "section": "B", "insertion": beam}),
            ];
            let areas = vec![
                json!({"id": "F", "nodes": ["a", "b", "c", "d"], "section": "SL", "insertion": floor}),
            ];
            let supports = ["a", "d", "root"]
                .iter()
                .map(|n| json!({"node": n, "fix": "fixed"}))
                .collect();
            base(
                nodes,
                frames,
                areas,
                supports,
                json!({"floor": "rectangular", "floor_size": h}),
            )
        }
        None => {
            let id = |i: usize, j: usize| match (i, j) {
                (0, 1) => "root".to_string(),
                (4, 1) => "tip".to_string(),
                _ => format!("{i}_{j}"),
            };
            let mut nodes = Vec::new();
            for (j, y) in ys.iter().enumerate() {
                for i in 0..=4 {
                    nodes.push(json!({"id": id(i, j), "x": i as f64, "y": y, "z": 3.0}));
                }
            }
            let mut areas = Vec::new();
            for j in 0..2 {
                for i in 0..4 {
                    areas.push(json!({"id": format!("A{i}{j}"), "nodes": [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)],
                                      "section": "SL", "insertion": floor}));
                }
            }
            // The beam's joint offset at j grows along it: each piece gets its share.
            let frames = (0..4)
                .map(|i| {
                    let t = |k: usize| offset.map(|v| v * k as f64 / 4.0);
                    json!({"id": format!("B{i}"), "type": "frame", "nodes": [id(i, 1), id(i + 1, 1)], "material": "C", "section": "B",
                           "insertion": {"point": 8, "transform": true, "offset_i": t(i), "offset_j": t(i + 1), "global": true}})
                })
                .collect();
            let supports = (0..3)
                .map(|j| json!({"node": id(0, j), "fix": "fixed"}))
                .collect();
            base(nodes, frames, areas, supports, json!("off"))
        }
    }
}

fn tip(m: &Value) -> Vec<[f64; 6]> {
    let r = osab_mesh::api::analyze_value(m.clone()).unwrap();
    ["PZ", "PX"]
        .iter()
        .map(|c| {
            let case = r.case(&(*c).into()).unwrap();
            assert!(case.equilibrium_ok());
            case.displacement(&"tip".into()).unwrap().as_array()
        })
        .collect()
}

#[test]
fn meshed_floor_and_split_beam_keep_their_insertion_points() {
    for offset in [[0.0; 3], [0.0, 0.0, -0.2]] {
        let meshed = tip(&t_beam(Some(1.0), offset));
        let drawn = tip(&t_beam(None, offset));
        for (a, b) in meshed.iter().zip(&drawn) {
            let scale = b.iter().fold(0.0f64, |m, x| m.max(x.abs()));
            for (x, y) in a.iter().zip(b) {
                assert!((x - y).abs() <= 1e-9 * scale, "{a:?} vs {b:?}");
            }
        }
    }
    // The insertion point changes the answer (the test would pass trivially otherwise).
    let mut plain = t_beam(None, [0.0; 3]);
    for a in plain["areas"].as_array_mut().unwrap() {
        a.as_object_mut().unwrap().remove("insertion");
    }
    assert!((tip(&plain)[0][2] - tip(&t_beam(None, [0.0; 3]))[0][2]).abs() > 1e-5);
    assert!(Model::from_json(&t_beam(Some(1.0), [0.0; 3]).to_string()).is_ok());
}
