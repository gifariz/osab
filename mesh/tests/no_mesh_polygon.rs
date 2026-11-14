//! An area set to no auto mesh with more than 4 corners: one element needs 3 or 4, so the mesh step meshes
//! it with the model's settings, as if it had no mesh option, and warns in its notes (docs/SOURCES.md).

use osab_solver::Model;
use serde_json::{json, Value};

/// An L-shaped 6 x 6 m floor (6 corners) with fixed corners, or a 6 x 6 m square (4 corners).
fn model(l_shape: bool, mesh: Option<Value>) -> Model {
    let corners: Vec<[f64; 2]> = if l_shape {
        vec![[0., 0.], [6., 0.], [6., 3.], [3., 3.], [3., 6.], [0., 6.]]
    } else {
        vec![[0., 0.], [6., 0.], [6., 6.], [0., 6.]]
    };
    let ids: Vec<String> = (1..=corners.len()).map(|k| k.to_string()).collect();
    let nodes: Vec<Value> = ids
        .iter()
        .zip(&corners)
        .map(|(id, p)| json!({"id": id, "x": p[0], "y": p[1], "z": 3}))
        .collect();
    let mut area = json!({"id": "F", "nodes": ids, "section": "S"});
    if let Some(m) = mesh {
        area["mesh"] = m;
    }
    let supports: Vec<Value> = ids
        .iter()
        .map(|id| json!({"node": id, "fix": "fixed"}))
        .collect();
    serde_json::from_value(json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}], "sections": [],
        "area_sections": [{"id": "S", "type": "shell-thin", "material": "C", "thickness": 0.2}],
        "nodes": nodes, "areas": [area], "supports": supports,
        "mesh": {"floor_size": 1.0},
        "load_cases": [{"id": "Q", "area_loads": [{"area": "F", "dir": "Z", "q": -5}]}]
    }))
    .unwrap()
}

#[test]
fn a_polygon_set_to_no_mesh_is_meshed_with_the_models_settings_and_warned() {
    let none = osab_mesh::mesh(&model(true, Some(json!("none")))).unwrap();
    let default = osab_mesh::mesh(&model(true, None)).unwrap();
    assert!(none.model.areas.len() > 1);
    assert_eq!(none.model.areas.len(), default.model.areas.len());
    assert_eq!(none.model.nodes.len(), default.model.nodes.len());
    let warned: Vec<&String> = none
        .notes
        .iter()
        .filter(|n| n.contains("no auto mesh"))
        .collect();
    assert_eq!(warned.len(), 1, "{:?}", none.notes);
    assert!(warned[0].contains("areas F "));
    assert!(!default.notes.iter().any(|n| n.contains("no auto mesh")));

    // The whole load reaches the supports: 27 m2 x 5 kN/m2.
    let r = osab_mesh::run(&model(true, Some(json!("none")))).unwrap();
    let fz: f64 = r.cases[0].reactions.iter().map(|x| x.values.fz).sum();
    assert!((fz - 135.0).abs() < 1e-6, "{fz}");
}

#[test]
fn a_quad_set_to_no_mesh_stays_one_element_without_a_warning() {
    let r = osab_mesh::mesh(&model(false, Some(json!("none")))).unwrap();
    assert_eq!(r.model.areas.len(), 1);
    assert!(!r.notes.iter().any(|n| n.contains("no auto mesh")));
}
