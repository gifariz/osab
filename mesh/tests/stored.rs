//! Stored meshes (step 8b): an area keeps the mesh stored on it, whatever the mesh
//! settings; its neighbours line up with its joints; its loads, restraints on edges and openings act as
//! with the auto mesh.

use std::collections::HashMap;

use osab_solver::model::{Id, UserMesh};
use osab_solver::Model;
use serde_json::{json, Value};

// The object model is read as the run step reads it (the solver checks the analysis model it makes).
fn model(v: Value) -> Model {
    serde_json::from_value(v).unwrap()
}

fn base(areas: Value, nodes: Value, supports: Value, loads: Value, mesh: Value) -> Model {
    model(json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
        "sections": [],
        "area_sections": [
            {"id": "S", "type": "shell-thin", "material": "C", "thickness": 0.2},
            {"id": "D", "type": "shell-thin", "material": "C", "thickness": 0.3, "slab_type": "drop"}],
        "nodes": nodes,
        "areas": areas,
        "supports": supports,
        "load_cases": [{"id": "Q", "area_loads": loads}],
        "mesh": mesh
    }))
}

/// Stores the current auto mesh of `area` on it, as the app's Edit > Store Mesh does.
fn store(m: &Model, area: &str) -> Model {
    let meshed = osab_mesh::mesh(m).unwrap();
    let mut at: HashMap<Id, [f64; 3]> = m
        .nodes
        .iter()
        .map(|n| (n.id.clone(), [n.x, n.y, n.z]))
        .collect();
    for j in &meshed.info.joints {
        at.insert(j.id.clone(), [j.x, j.y, j.z]);
    }
    let mut joints: Vec<Id> = Vec::new();
    let mut elements = Vec::new();
    for el in meshed.info.shells.iter().filter(|e| e.parent.0 == area) {
        let e: Vec<usize> = el
            .nodes
            .iter()
            .map(|id| match joints.iter().position(|j| j == id) {
                Some(k) => k,
                None => {
                    joints.push(id.clone());
                    joints.len() - 1
                }
            })
            .collect();
        elements.push(e);
    }
    assert!(!elements.is_empty(), "{area} has no mesh elements");
    let mut out = m.clone();
    let a = out.areas.iter_mut().find(|a| a.id.0 == area).unwrap();
    a.user_mesh = Some(UserMesh {
        joints: joints.iter().map(|id| at[id]).collect(),
        elements,
    });
    out
}

fn reaction_z(m: &Model) -> f64 {
    let r = osab_mesh::run(m).unwrap();
    r.cases[0].reactions.iter().map(|x| x.values.fz).sum()
}

/// A 5 x 4 m slab on its corners (4 x 4 elements at 1.25 m).
fn slab(mesh: Value) -> Model {
    base(
        json!([{"id": "F1", "nodes": ["1", "2", "3", "4"], "section": "S"}]),
        json!([{"id": "1"}, {"id": "2", "x": 5}, {"id": "3", "x": 5, "y": 4}, {"id": "4", "y": 4}]),
        json!([{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"},
               {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}]),
        json!([{"area": "F1", "dir": "Z", "q": -10}]),
        mesh,
    )
}

#[test]
fn a_stored_auto_mesh_gives_the_same_analysis_and_stays_fixed() {
    let auto = slab(Value::Null);
    let stored = store(&auto, "F1");
    let (a, s) = (
        osab_mesh::run(&auto).unwrap(),
        osab_mesh::run(&stored).unwrap(),
    );
    assert_eq!(s.mesh.as_ref().unwrap().shells.len(), 16);
    assert!(s
        .mesh
        .as_ref()
        .unwrap()
        .shells
        .iter()
        .all(|e| e.parent.0 == "F1"));
    assert!(s.cases[0].shells.iter().all(|e| e.area.0 == "F1"));
    // The same mesh: every joint at the same point moves the same.
    let pos = |r: &osab_solver::Results, m: &Model| -> HashMap<String, [f64; 3]> {
        let mut at: HashMap<String, [f64; 3]> = m
            .nodes
            .iter()
            .map(|n| (n.id.0.clone(), [n.x, n.y, n.z]))
            .collect();
        for j in &r.mesh.as_ref().unwrap().joints {
            at.insert(j.id.0.clone(), [j.x, j.y, j.z]);
        }
        at
    };
    let (pa, ps) = (pos(&a, &auto), pos(&s, &stored));
    let key = |p: [f64; 3]| format!("{:.6},{:.6},{:.6}", p[0], p[1], p[2]);
    let ua: HashMap<String, f64> = a.cases[0]
        .displacements
        .iter()
        .map(|d| (key(pa[&d.node.0]), d.values.uz))
        .collect();
    let largest = ua.values().fold(0.0f64, |m, v| m.max(v.abs()));
    assert_eq!(
        s.cases[0].displacements.len(),
        a.cases[0].displacements.len()
    );
    for d in &s.cases[0].displacements {
        let want = ua[&key(ps[&d.node.0])];
        assert!(
            (d.values.uz - want).abs() <= 1e-10 * largest,
            "{} {} {want}",
            d.node.0,
            d.values.uz
        );
    }
    // Other mesh settings do not change a stored mesh.
    let mut finer = stored.clone();
    finer.mesh = slab(json!({"floor_size": 0.5})).mesh;
    assert_eq!(osab_mesh::mesh(&finer).unwrap().info.shells.len(), 16);
    assert!((reaction_z(&finer) - 200.0).abs() < 1e-9);
}

#[test]
fn neighbours_line_up_with_a_stored_mesh() {
    // F1 (0..4) stored as 2 x 2 elements; F2 (4..8) auto-meshed at 1 m: on the shared edge F2's joints
    // at y = 1 and 3 hang on F1's elements (edge constraints), y = 2 is F1's stored joint.
    let m = base(
        json!([{"id": "F1", "nodes": ["1", "2", "5", "6"], "section": "S"},
               {"id": "F2", "nodes": ["2", "3", "4", "5"], "section": "S"}]),
        json!([{"id": "1"}, {"id": "2", "x": 4}, {"id": "3", "x": 8}, {"id": "4", "x": 8, "y": 4},
               {"id": "5", "x": 4, "y": 4}, {"id": "6", "y": 4}]),
        json!([{"node": "1", "fix": "fixed"}, {"node": "3", "fix": "fixed"},
               {"node": "4", "fix": "fixed"}, {"node": "6", "fix": "fixed"}]),
        json!([{"area": "F1", "dir": "Z", "q": -10}, {"area": "F2", "dir": "Z", "q": -10}]),
        json!({"floor_size": 1.0}),
    );
    let mut s = m.clone();
    let grid: Vec<[f64; 3]> = (0..3)
        .flat_map(|j| (0..3).map(move |i| [2.0 * i as f64, 2.0 * j as f64, 0.0]))
        .collect();
    s.areas[0].user_mesh = Some(UserMesh {
        joints: grid,
        elements: vec![
            vec![0, 1, 4, 3],
            vec![1, 2, 5, 4],
            vec![3, 4, 7, 6],
            vec![4, 5, 8, 7],
        ],
    });
    let meshed = osab_mesh::mesh(&s).unwrap();
    let f1 = meshed
        .info
        .shells
        .iter()
        .filter(|e| e.parent.0 == "F1")
        .count();
    let f2 = meshed
        .info
        .shells
        .iter()
        .filter(|e| e.parent.0 == "F2")
        .count();
    assert_eq!((f1, f2), (4, 16));
    assert_eq!(meshed.model.edge_constraints.len(), 2);
    assert!((reaction_z(&s) - 320.0).abs() < 1e-9);
}

#[test]
fn loads_over_a_stored_area_reach_the_drop_panel_inside_it() {
    // A 6 x 6 m slab with a 2 x 2 m drop panel in its middle: the slab's stored mesh leaves the drop's
    // part out, but the slab's load still acts on its whole area (the rule for overlaps).
    let m = base(
        json!([{"id": "F1", "nodes": ["1", "2", "3", "4"], "section": "S"},
               {"id": "F2", "nodes": ["5", "6", "7", "8"], "section": "D"}]),
        json!([{"id": "1"}, {"id": "2", "x": 6}, {"id": "3", "x": 6, "y": 6}, {"id": "4", "y": 6},
               {"id": "5", "x": 2, "y": 2}, {"id": "6", "x": 4, "y": 2}, {"id": "7", "x": 4, "y": 4},
               {"id": "8", "x": 2, "y": 4}]),
        json!([{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"},
               {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}]),
        json!([{"area": "F1", "dir": "Z", "q": -10}, {"area": "F2", "dir": "Z", "q": -5}]),
        json!({"floor": "rectangular", "floor_size": 1.0}),
    );
    let s = store(&m, "F1");
    let auto = osab_mesh::mesh(&m).unwrap();
    let kept = osab_mesh::mesh(&s).unwrap();
    assert_eq!(kept.info.shells.len(), auto.info.shells.len());
    assert!((reaction_z(&s) - (360.0 + 20.0)).abs() < 1e-9);
    assert!((reaction_z(&s) - reaction_z(&m)).abs() < 1e-9);
}

#[test]
fn a_stored_wall_keeps_its_restraints_on_edges_and_openings_stay_holes() {
    // A 4 x 3 m wall fixed at its base corners: its base mesh joints are fixed too (walls' default).
    let wall = base(
        json!([{"id": "W1", "nodes": ["1", "2", "3", "4"], "section": "S"}]),
        json!([{"id": "1"}, {"id": "2", "x": 4}, {"id": "3", "x": 4, "z": 3}, {"id": "4", "z": 3}]),
        json!([{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"}]),
        json!([{"area": "W1", "dir": "X", "q": 2}]),
        json!({"wall_size": 1.0}),
    );
    let auto = osab_mesh::mesh(&wall).unwrap();
    let kept = osab_mesh::mesh(&store(&wall, "W1")).unwrap();
    assert_eq!(kept.model.supports.len(), auto.model.supports.len());
    assert_eq!(kept.model.supports.len(), 5);

    // A floor with an opening: stored, the opening is the hole in the stored mesh.
    let holed = base(
        json!([{"id": "F1", "nodes": ["1", "2", "3", "4"], "section": "S"},
               {"id": "O1", "nodes": ["5", "6", "7", "8"], "opening": true}]),
        json!([{"id": "1"}, {"id": "2", "x": 6}, {"id": "3", "x": 6, "y": 6}, {"id": "4", "y": 6},
               {"id": "5", "x": 2, "y": 2}, {"id": "6", "x": 3, "y": 2}, {"id": "7", "x": 3, "y": 3},
               {"id": "8", "x": 2, "y": 3}]),
        json!([{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"},
               {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}]),
        json!([{"area": "F1", "dir": "Z", "q": -10}]),
        json!({"floor": "rectangular", "floor_size": 1.0}),
    );
    let s = store(&holed, "F1");
    assert_eq!(
        osab_mesh::mesh(&s).unwrap().info.shells.len(),
        osab_mesh::mesh(&holed).unwrap().info.shells.len()
    );
    assert!((reaction_z(&s) - 350.0).abs() < 1e-9);
}

#[test]
fn openings_at_a_corner_and_across_two_stored_floors_are_their_holes() {
    // F1 (0..4) and F2 (4..8) by 4 m, both stored. O1 covers F1's corner at the origin (the mesh has no
    // joint there), O2 lies across the two floors: both are holes in the stored meshes already.
    let m = base(
        json!([{"id": "F1", "nodes": ["1", "2", "5", "6"], "section": "S"},
               {"id": "F2", "nodes": ["2", "3", "4", "5"], "section": "S"},
               {"id": "O1", "nodes": ["11", "12", "13", "14"], "opening": true},
               {"id": "O2", "nodes": ["21", "22", "23", "24"], "opening": true}]),
        json!([{"id": "1"}, {"id": "2", "x": 4}, {"id": "3", "x": 8}, {"id": "4", "x": 8, "y": 4},
               {"id": "5", "x": 4, "y": 4}, {"id": "6", "y": 4},
               {"id": "11", "x": -0.5, "y": -0.5}, {"id": "12", "x": 1, "y": -0.5},
               {"id": "13", "x": 1, "y": 1}, {"id": "14", "x": -0.5, "y": 1},
               {"id": "21", "x": 3, "y": 2}, {"id": "22", "x": 5, "y": 2},
               {"id": "23", "x": 5, "y": 3}, {"id": "24", "x": 3, "y": 3}]),
        json!([{"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"},
               {"node": "6", "fix": "fixed"}]),
        json!([{"area": "F1", "dir": "Z", "q": -10}, {"area": "F2", "dir": "Z", "q": -10}]),
        json!({"floor_size": 1.0}),
    );
    // Both stored from the same auto mesh.
    let mut s = store(&m, "F1");
    s.areas[1].user_mesh = store(&m, "F2").areas[1].user_mesh.clone();
    assert_eq!(
        osab_mesh::mesh(&s).unwrap().info.shells.len(),
        osab_mesh::mesh(&m).unwrap().info.shells.len()
    );
    // 32 m2 less the openings' 1 m2 (in F1) and 2 m2: loads over openings act on nothing.
    assert!((reaction_z(&s) - 290.0).abs() < 1e-9);
    assert!((reaction_z(&s) - reaction_z(&m)).abs() < 1e-9);
}

#[test]
fn overlapping_stored_elements_are_both_analysed() {
    // A stored element given twice (as in a mesh from another program): both count, with a note.
    let mut s = store(&slab(Value::Null), "F1");
    let um = s.areas[0].user_mesh.as_mut().unwrap();
    um.elements.push(um.elements[0].clone());
    let meshed = osab_mesh::mesh(&s).unwrap();
    assert_eq!(meshed.info.shells.len(), 17);
    assert!(
        meshed
            .notes
            .iter()
            .any(|n| n.contains("both are analysed in full") && n.contains("F1-1/F1-17")),
        "{:?}",
        meshed.notes
    );
    // The load acts on both: 200 kN and 10 kN/m2 on the 1.25 m2 element once more.
    assert!((reaction_z(&s) - 212.5).abs() < 1e-9);
}

#[test]
fn a_stored_mesh_that_no_longer_fits_is_refused() {
    let mut s = store(&slab(Value::Null), "F1");
    s.nodes[2].x = 6.0; // corner 3 moved
    let e = osab_mesh::mesh(&s).err().unwrap().to_string();
    assert!(e.contains("area F1: its stored mesh no longer fits"), "{e}");
    let mut bad = store(&slab(Value::Null), "F1");
    bad.areas[0]
        .user_mesh
        .as_mut()
        .unwrap()
        .elements
        .push(vec![0, 1]);
    assert!(osab_mesh::mesh(&bad)
        .err()
        .unwrap()
        .to_string()
        .contains("not 3 or 4"));
}
