//! Walls in the mesh step: meshed Auto Rectangular by default, so floors line up with them;
//! Membrane walls stay one element (the default), tied to the floor's joints by edge constraints.

use osab_solver::Model;
use serde_json::json;

fn floor_on_wall(wall_type: &str) -> Model {
    serde_json::from_value(json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
        "sections": [{"id": "COL", "A": 0.16, "I22": 2.1e-3, "I33": 2.1e-3, "J": 3.6e-3}],
        "area_sections": [
            {"id": "SLAB", "type": "shell-thin", "material": "C", "thickness": 0.2},
            {"id": "WALL", "type": wall_type, "material": "C", "thickness": 0.25}],
        "nodes": [{"id": "1", "z": 3}, {"id": "2", "x": 4, "z": 3}, {"id": "3", "x": 4, "y": 4, "z": 3},
                  {"id": "4", "y": 4, "z": 3}, {"id": "5"}, {"id": "6", "x": 4},
                  {"id": "7", "x": 4, "y": 4}, {"id": "8", "y": 4}],
        "areas": [{"id": "F1", "nodes": ["1", "2", "3", "4"], "section": "SLAB"},
                  {"id": "W1", "nodes": ["5", "6", "2", "1"], "section": "WALL"}],
        "frames": [{"id": "C1", "type": "frame", "nodes": ["7", "3"], "material": "C", "section": "COL"},
                   {"id": "C2", "type": "frame", "nodes": ["8", "4"], "material": "C", "section": "COL"}],
        "supports": [{"node": "5", "fix": "fixed"}, {"node": "6", "fix": "fixed"},
                     {"node": "7", "fix": "fixed"}, {"node": "8", "fix": "fixed"}],
        "load_cases": [{"id": "Q", "area_loads": [{"area": "F1", "dir": "Z", "q": -10}]}]
    }))
    .unwrap()
}

#[test]
fn shell_walls_are_meshed_and_line_up_with_the_floor() {
    let m = osab_mesh::mesh(&floor_on_wall("shell-thin")).unwrap();
    let wall: Vec<_> = m
        .model
        .areas
        .iter()
        .filter(|a| a.id.0.starts_with("W1"))
        .collect();
    // 4 x 3 m at 1.25 m: 4 x 3 elements; the floor's 4 x 4.
    assert_eq!(wall.len(), 12);
    assert_eq!(m.model.areas.len() - wall.len(), 16);
    assert!(m.model.edge_constraints.is_empty());
    // The wall's base corners are fixed, so its mesh joints on the base get fixed too.
    assert_eq!(m.model.supports.len(), 4 + 3);
    let r = osab_mesh::run(&floor_on_wall("shell-thin")).unwrap();
    let fz: f64 = r.cases[0].reactions.iter().map(|x| x.values.fz).sum();
    assert!((fz - 160.0).abs() < 1e-8, "{fz}");
}

/// A 4 x 4 m area sloping 25 degrees: a wall by the default 20-degree limit, a floor with 35.
fn sloped_area(limits: serde_json::Value) -> Model {
    let (c, s) = (
        4.0 * 25f64.to_radians().cos(),
        4.0 * 25f64.to_radians().sin(),
    );
    let mut m = json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
        "sections": [],
        "area_sections": [{"id": "S", "type": "shell-thin", "material": "C", "thickness": 0.2}],
        "nodes": [{"id": "1"}, {"id": "2", "x": 4}, {"id": "3", "x": 4, "y": c, "z": s},
                  {"id": "4", "y": c, "z": s}],
        "areas": [{"id": "A1", "nodes": ["1", "2", "3", "4"], "section": "S"}],
        "mesh": {"floor": "rectangular", "floor_size": 1.0, "wall_size": 4.0}
    });
    if !limits.is_null() {
        m["angle_limits"] = limits;
    }
    Model::from_json(&m.to_string()).unwrap()
}

#[test]
fn the_floor_wall_angle_limit_decides_how_an_area_is_meshed() {
    let as_wall = osab_mesh::mesh(&sloped_area(serde_json::Value::Null)).unwrap();
    assert_eq!(as_wall.model.areas.len(), 1); // wall size 4 m
    let as_floor = osab_mesh::mesh(&sloped_area(json!({"beam_floor": 35}))).unwrap();
    assert_eq!(as_floor.model.areas.len(), 16); // floor size 1 m
    let bad = json!({"materials": [], "sections": [], "nodes": [], "angle_limits": {"column": 60, "beam_floor": 40}});
    assert!(Model::from_json(&bad.to_string()).is_err());
}

#[test]
fn membrane_walls_stay_one_element_tied_by_edge_constraints() {
    let m = osab_mesh::mesh(&floor_on_wall("membrane")).unwrap();
    assert!(m.model.areas.iter().any(|a| a.id.0 == "W1"));
    assert_eq!(m.model.areas.len(), 1 + 16);
    // The floor's three mesh joints on the wall's top edge.
    assert_eq!(m.model.edge_constraints.len(), 3);
    assert!(m.model.edge_constraints.iter().all(|e| e.area.0 == "W1"));
}
