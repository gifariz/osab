//! Null objects and the *Include in mesh* option in the mesh step (Phase 3b step 6g, probe findings
//! 2h): null frames and areas get no elements and pass their loads to the floor under them (or drop them,
//! where there is no floor), joints with nothing attached are left out of the mesh with
//! their loads going to the element they lie in, and a frame left out of the mesh is tied to the floor
//! where it crosses element edges.

use osab_solver::Model;
use serde_json::{json, Value};

/// A 6 x 5 m slab at z = 3 (corners fixed), Rectangular mesh 1.25 m, plus `extra` merged in (nodes,
/// frames and areas are appended).
fn slab(extra: Value) -> Model {
    let mut m = json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
        "sections": [{"id": "B", "A": 0.12, "I22": 9e-4, "I33": 1.6e-3, "J": 1.2e-3}],
        "area_sections": [{"id": "S", "type": "shell-thin", "material": "C", "thickness": 0.2}],
        "nodes": [{"id": "1", "z": 3}, {"id": "2", "x": 6, "z": 3}, {"id": "3", "x": 6, "y": 5, "z": 3},
                  {"id": "4", "y": 5, "z": 3}],
        "areas": [{"id": "F", "nodes": ["1", "2", "3", "4"], "section": "S"}],
        "frames": [],
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"},
                     {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}],
        "mesh": {"floor": "rectangular"},
        "load_cases": [{"id": "L"}]
    });
    for (k, v) in extra.as_object().unwrap() {
        match (m[k].as_array_mut(), v.as_array()) {
            (Some(list), Some(more)) if k != "load_cases" => list.extend(more.iter().cloned()),
            _ => m[k] = v.clone(),
        }
    }
    serde_json::from_value(m).unwrap()
}

fn total_fz(r: &osab_solver::Results) -> f64 {
    r.cases[0].reactions.iter().map(|x| x.values.fz).sum()
}

#[test]
fn null_frame_cuts_the_mesh_and_passes_its_load() {
    let m = slab(json!({
        "nodes": [{"id": "a", "x": 2, "z": 3}, {"id": "b", "x": 2, "y": 5, "z": 3}],
        "frames": [{"id": "N", "type": "frame", "nodes": ["a", "b"]}],
        "load_cases": [{"id": "L", "frame_loads": [{"type": "uniform", "frame": "N", "dir": "Z", "w": -10}]}]
    }));
    assert!(m.frames[0].is_null());
    let r = osab_mesh::run(&m).unwrap();
    // x = 0, 1, ... 6 (the null line's end j is kept like a column's line), no frame.
    assert_eq!(r.mesh.as_ref().unwrap().shells.len(), 24);
    assert!(r.cases[0].frames.is_empty());
    assert!((total_fz(&r) - 50.0).abs() < 1e-9);
}

#[test]
fn null_area_does_not_cut_the_mesh_and_loads_the_floor_under_it() {
    let m = slab(json!({
        "nodes": [{"id": "a", "x": 1.7, "y": 1.3, "z": 3}, {"id": "b", "x": 3.9, "y": 1.3, "z": 3},
                  {"id": "c", "x": 3.9, "y": 3.1, "z": 3}, {"id": "d", "x": 1.7, "y": 3.1, "z": 3}],
        "areas": [{"id": "NA", "nodes": ["a", "b", "c", "d"]}],
        "load_cases": [{"id": "L", "area_loads": [{"area": "NA", "dir": "Z", "q": -2}]}]
    }));
    assert!(m.areas[1].is_null());
    let r = osab_mesh::run(&m).unwrap();
    // The plain 5 x 4 grid; the null area's corners are left out (no displacement, no note about them).
    assert_eq!(r.mesh.as_ref().unwrap().shells.len(), 20);
    assert!((total_fz(&r) - 7.92).abs() < 1e-9);
    assert!(!r
        .notes
        .iter()
        .any(|n| n.starts_with("Restrained automatically")));
}

#[test]
fn null_loads_over_no_floor_are_dropped_with_a_note() {
    // A null area over a bay of beams with no floor : its load reaches nothing.
    let m: Model = serde_json::from_value(json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
        "sections": [{"id": "B", "A": 0.12, "I22": 9e-4, "I33": 1.6e-3, "J": 1.2e-3}],
        "nodes": [{"id": "1"}, {"id": "2", "x": 6}, {"id": "3", "x": 6, "y": 5}, {"id": "4", "y": 5}],
        "frames": [{"id": "B1", "type": "frame", "nodes": ["1", "2"], "material": "C", "section": "B"},
                   {"id": "B2", "type": "frame", "nodes": ["2", "3"], "material": "C", "section": "B"},
                   {"id": "B3", "type": "frame", "nodes": ["3", "4"], "material": "C", "section": "B"},
                   {"id": "B4", "type": "frame", "nodes": ["4", "1"], "material": "C", "section": "B"}],
        "areas": [{"id": "NA", "nodes": ["1", "2", "3", "4"]}],
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"},
                     {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}],
        "load_cases": [{"id": "L", "area_loads": [{"area": "NA", "dir": "Z", "q": -5}]}]
    }))
    .unwrap();
    let r = osab_mesh::run(&m).unwrap();
    assert!(total_fz(&r).abs() < 1e-12);
    assert!(r
        .notes
        .iter()
        .any(|n| n.contains("not carried") && n.ends_with("NA")));
}

#[test]
fn loaded_free_joint_is_left_out_unless_included() {
    let point = |in_mesh: Value| {
        let mut j = json!({"id": "p", "x": 2.3, "y": 1.7, "z": 3});
        if !in_mesh.is_null() {
            j["in_mesh"] = in_mesh;
        }
        slab(json!({
            "nodes": [j],
            "load_cases": [{"id": "L", "node_loads": [{"node": "p", "fz": -10, "mx": 2}]}]
        }))
    };
    // Default: the plain grid; the load goes to the four joints of the element round it.
    let r = osab_mesh::run(&point(Value::Null)).unwrap();
    let mesh = r.mesh.as_ref().unwrap();
    assert_eq!(mesh.shells.len(), 20);
    assert!((total_fz(&r) - 10.0).abs() < 1e-9);
    let p = r.cases[0]
        .displacements
        .iter()
        .find(|d| d.node.0 == "p")
        .unwrap();
    assert_eq!(p.values.uz, 0.0);
    // Include in mesh: its lines are kept like a column's.
    let r = osab_mesh::run(&point(json!(true))).unwrap();
    assert_eq!(r.mesh.as_ref().unwrap().shells.len(), 25);
    assert!((total_fz(&r) - 10.0).abs() < 1e-9);
}

#[test]
fn frame_left_out_of_the_mesh_is_tied_where_it_crosses_element_edges() {
    // A beam across the slab with Include in mesh off.
    let beam = |in_mesh: Option<bool>| {
        let mut f = json!({"id": "B1", "type": "frame", "nodes": ["a", "b"], "material": "C", "section": "B"});
        if let Some(v) = in_mesh {
            f["in_mesh"] = json!(v);
        }
        slab(json!({
            "nodes": [{"id": "a", "y": 2.6, "z": 3}, {"id": "b", "x": 6, "y": 2.6, "z": 3}],
            "frames": [f],
            "load_cases": [{"id": "L", "area_loads": [{"area": "F", "dir": "Z", "q": -5}]}]
        }))
    };
    let m = osab_mesh::mesh(&beam(Some(false))).unwrap();
    assert_eq!(m.model.areas.len(), 20);
    assert_eq!(m.model.frames.len(), 5);
    // Four crossings on two element edges each, and the two ends on the slab's edges.
    assert_eq!(m.model.edge_constraints.len(), 10);
    // Default: the floor is cut along the beam (25 elements, no edge constraints).
    let m = osab_mesh::mesh(&beam(None)).unwrap();
    assert_eq!(m.model.areas.len(), 25);
    assert!(m.model.edge_constraints.is_empty());
}

#[test]
fn mesh_off_passes_free_joint_and_null_loads_to_the_elements() {
    let m = slab(json!({
        "mesh": "off",
        "nodes": [{"id": "p", "x": 1.5, "y": 1.0, "z": 3},
                  {"id": "a", "x": 1, "y": 4, "z": 3}, {"id": "b", "x": 5, "y": 4, "z": 3}],
        "frames": [{"id": "N", "type": "frame", "nodes": ["a", "b"]}],
        "load_cases": [{"id": "L", "node_loads": [{"node": "p", "fz": -10}],
                        "frame_loads": [{"type": "point", "frame": "N", "dir": "Z", "p": -4, "at": 1}]}]
    }));
    let r = osab_mesh::run(&m).unwrap();
    assert_eq!(r.cases[0].shells.len(), 1);
    assert!((total_fz(&r) - 14.0).abs() < 1e-9);
}

#[test]
fn skew_cut_cells_are_split_into_two_quads() {
    // A skew beam: every cell the beam cuts into five sides becomes two quads.
    let m = slab(json!({
        "nodes": [{"id": "a", "x": 0.6, "y": 0.3, "z": 3}, {"id": "b", "x": 5.4, "y": 4.6, "z": 3}],
        "frames": [{"id": "B1", "type": "frame", "nodes": ["a", "b"], "material": "C", "section": "B"}]
    }));
    let m = osab_mesh::mesh(&m).unwrap();
    assert_eq!(m.model.areas.len(), 40);
    assert_eq!(m.model.edge_constraints.len(), 8);
}
