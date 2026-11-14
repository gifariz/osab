//! User wind loads through the mesh step (docs/phases/LOAD_PATTERNS_PLAN.md): on a semi-rigid diaphragm
//! the load at (x, y) goes into the floor element under the point exactly like a joint load on a joint
//! left out of the mesh (the rule B41 checks against the reference program); on a rigid one it acts
//! through the constraint. Neither adds a joint or changes the mesh.

use osab_solver::{Model, Results};
use serde_json::{json, Value};

/// A 6 x 4 m Shell-Thin slab on four 3 m columns (twin floors),
/// meshed by default, the slab in diaphragm D1.
fn floor(semi_rigid: bool) -> Value {
    let corners = [(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)];
    let (mut nodes, mut frames, mut supports) = (Vec::new(), Vec::new(), Vec::new());
    for (k, (x, y)) in corners.iter().enumerate() {
        nodes.push(json!({"id": format!("b{k}"), "x": x, "y": y, "z": 0}));
        nodes.push(json!({"id": format!("t{k}"), "x": x, "y": y, "z": 3}));
        frames.push(json!({"id": format!("C{k}"), "type": "frame", "nodes": [format!("b{k}"), format!("t{k}")],
                           "material": "C", "section": "COL"}));
        supports.push(json!({"node": format!("b{k}"), "fix": "fixed"}));
    }
    json!({"materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
           "sections": [{"id": "COL", "A": 0.25, "I22": 5.2e-3, "I33": 5.2e-3, "J": 8.8e-3}],
           "area_sections": [{"id": "S200", "type": "shell-thin", "material": "C", "thickness": 0.2}],
           "nodes": nodes, "frames": frames, "supports": supports,
           "areas": [{"id": "F1", "nodes": ["t0", "t1", "t2", "t3"], "section": "S200", "diaphragm": "D1"}],
           "diaphragms": [{"id": "D1", "semi_rigid": semi_rigid}],
           "stories": [{"id": "Base", "z": 0}, {"id": "Story1", "z": 3}]})
}

fn with_user_wind(mut m: Value, x: f64, y: f64) -> Value {
    m["load_patterns"] = json!([{"id": "WS", "type": "wind", "auto": "user-wind", "wind_point": "coordinates",
        "wind_loads": [{"story": "Story1", "diaphragm": "D1", "fx": 20, "fy": 5, "mz": 3, "x": x, "y": y}]}]);
    m["load_cases"] = json!([{"id": "W", "loads": [{"pattern": "WS", "scale": 1.5}]}]);
    m
}

fn run(m: &Value) -> Result<Results, String> {
    let model: Model = serde_json::from_value(m.clone()).unwrap();
    osab_mesh::run(&model).map_err(|e| e.to_string())
}

fn base_shear(r: &Results) -> (f64, f64) {
    let c = &r.cases[0];
    (
        c.reactions.iter().map(|r| r.values.fx).sum(),
        c.reactions.iter().map(|r| r.values.fy).sum(),
    )
}

#[test]
fn on_a_semi_rigid_floor_it_acts_like_a_joint_load_left_out_of_the_mesh() {
    let a = run(&with_user_wind(floor(true), 1.3, 1.1)).unwrap();
    // The reference: a joint at the point with nothing attached (not in the mesh) and the same load.
    let mut m = floor(true);
    m["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "P", "x": 1.3, "y": 1.1, "z": 3}));
    m["load_patterns"] =
        json!([{"id": "JL", "node_loads": [{"node": "P", "fx": 20, "fy": 5, "mz": 3}]}]);
    m["load_cases"] = json!([{"id": "W", "loads": [{"pattern": "JL", "scale": 1.5}]}]);
    let b = run(&m).unwrap();
    let (ca, cb) = (&a.cases[0], &b.cases[0]);
    let scale = cb
        .displacements
        .iter()
        .flat_map(|d| d.values.as_array())
        .fold(0.0f64, |m, v| m.max(v.abs()));
    // The reference reports its extra joint too.
    assert_eq!(ca.displacements.len() + 1, cb.displacements.len());
    for d in &ca.displacements {
        let (u, v) = (
            d.values.as_array(),
            cb.displacement(&d.node).unwrap().as_array(),
        );
        for k in 0..6 {
            assert!(
                (u[k] - v[k]).abs() <= 1e-12 * scale,
                "{}: {u:?} vs {v:?}",
                d.node
            );
        }
    }
    let (fx, fy) = base_shear(&a);
    assert!((fx + 30.0).abs() < 1e-9 && (fy + 7.5).abs() < 1e-9);
    // The floor is not tied: its corners move differently in the plane.
    let ux: Vec<f64> = (0..4)
        .map(|k| {
            ca.displacement(&format!("t{k}").as_str().into())
                .unwrap()
                .ux
        })
        .collect();
    assert!((ux[0] - ux[2]).abs() > 1e-3 * ux[0].abs());
}

#[test]
fn on_a_semi_rigid_floor_a_point_off_the_floor_falls_back_to_spreading_by_mass() {
    // Without mass there is nothing to spread over.
    let e = run(&with_user_wind(floor(true), 10.0, 10.0)).unwrap_err();
    assert!(e.contains("has no mass there"), "{e}");
    let mut m = with_user_wind(floor(true), 10.0, 10.0);
    m["materials"][0]["weight"] = json!(24.0);
    let r = run(&m).unwrap();
    assert!(
        r.notes
            .iter()
            .any(|n| n.contains("lies on no floor element")
                && n.contains("spread over its joints by mass")),
        "{:?}",
        r.notes
    );
    let (fx, fy) = base_shear(&r);
    assert!((fx + 30.0).abs() < 1e-9 && (fy + 7.5).abs() < 1e-9);
    assert!(r.cases[0].equilibrium_ok());
}

#[test]
fn on_a_rigid_floor_it_acts_through_the_constraint_and_the_mesh_stays() {
    let a = run(&with_user_wind(floor(false), 10.0, 10.0)).unwrap();
    let (fx, fy) = base_shear(&a);
    assert!((fx + 30.0).abs() < 1e-9 && (fy + 7.5).abs() < 1e-9);
    assert!(a.cases[0].equilibrium_ok());
    let mut plain = floor(false);
    plain["load_cases"] = json!([{"id": "W"}]);
    let b = run(&plain).unwrap();
    assert_eq!(
        a.cases[0].displacements.len(),
        b.cases[0].displacements.len()
    );
    // Rigid: the corners keep their distances in the plane (u, v follow one rotation).
    let c = &a.cases[0];
    let d = |k: usize| c.displacement(&format!("t{k}").as_str().into()).unwrap();
    let (t0, t1) = (d(0), d(1));
    assert!((t0.ux - t1.ux).abs() < 1e-12 * t0.ux.abs().max(1e-9));
}
