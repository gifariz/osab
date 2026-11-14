//! User wind loads on rigid diaphragms (docs/phases/LOAD_PATTERNS_PLAN.md, probe 2): Fx, Fy, Mz at a
//! point (x, y) of a story's diaphragm act through the constraint, exactly as a joint at that point
//! joined to the diaphragm and carrying the load (what the reference program adds at analysis), with no
//! joint added to the model; a point off the floor is allowed. Bad rows are refused.

use osab_solver::{analyze, Model, Results};
use serde_json::{json, Value};

/// Four unequal cantilever columns at the corners of a 6 x 4 plan with joints at 1.5 m (Story1) and 3 m
/// (Story2), all in diaphragm D1.
fn columns() -> Value {
    let corners = [(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)];
    let (mut nodes, mut frames, mut supports) = (Vec::new(), Vec::new(), Vec::new());
    for (k, (x, y)) in corners.iter().enumerate() {
        nodes.push(json!({"id": format!("b{k}"), "x": x, "y": y, "z": 0}));
        nodes.push(json!({"id": format!("m{k}"), "x": x, "y": y, "z": 1.5, "diaphragm": "D1"}));
        nodes.push(json!({"id": format!("t{k}"), "x": x, "y": y, "z": 3, "diaphragm": "D1"}));
        for (a, b, s) in [("b", "m", "1"), ("m", "t", "2")] {
            frames.push(json!({"id": format!("C{k}{s}"), "type": "frame", "nodes": [format!("{a}{k}"), format!("{b}{k}")],
                               "material": "S", "section": format!("S{k}")}));
        }
        supports.push(json!({"node": format!("b{k}"), "fix": "fixed"}));
    }
    let sections: Vec<Value> = (0..4)
        .map(|k| json!({"id": format!("S{k}"), "A": 0.01, "I22": 1e-5 * (k + 1) as f64, "I33": 2e-5 * (k + 1) as f64, "J": 1e-5}))
        .collect();
    json!({"materials": [{"id": "S", "E": 200e6, "nu": 0.3}], "sections": sections, "nodes": nodes, "frames": frames,
           "supports": supports, "diaphragms": [{"id": "D1"}],
           "stories": [{"id": "Base", "z": 0}, {"id": "Story1", "z": 1.5}, {"id": "Story2", "z": 3}]})
}

/// (story, z, fx, fy, mz, x, y) of the user wind rows.
const ROWS: [(&str, f64, f64, f64, f64, f64, f64); 3] = [
    ("Story2", 3.0, 20.0, 0.0, 0.0, 1.0, 1.0),
    ("Story1", 1.5, 10.0, 5.0, 3.0, 5.0, 2.0),
    ("Story2", 3.0, -4.0, 0.0, 0.0, 10.0, 10.0), // off the floor
];

fn user_wind() -> Value {
    let mut m = columns();
    let rows: Vec<Value> = ROWS
        .iter()
        .map(|(s, _, fx, fy, mz, x, y)| json!({"story": s, "diaphragm": "D1", "fx": fx, "fy": fy, "mz": mz, "x": x, "y": y}))
        .collect();
    m["load_patterns"] = json!([{"id": "WT", "type": "wind", "auto": "user-wind", "wind_point": "coordinates", "wind_loads": rows}]);
    m["load_cases"] = json!([{"id": "W", "loads": [{"pattern": "WT", "scale": -0.8}]}]);
    m["load_combinations"] =
        json!([{"id": "COMB", "type": "linear-add", "items": [{"case": "W", "scale": 2.5}]}]);
    m
}

/// The same loads on joints at the points, joined to the diaphragm.
fn extra_joints() -> Value {
    let mut m = columns();
    let mut loads = Vec::new();
    for (k, (_, z, fx, fy, mz, x, y)) in ROWS.iter().enumerate() {
        m["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": format!("p{k}"), "x": x, "y": y, "z": z, "diaphragm": "D1"}));
        loads.push(json!({"node": format!("p{k}"), "fx": fx, "fy": fy, "mz": mz}));
    }
    m["load_patterns"] = json!([{"id": "WT", "type": "wind", "node_loads": loads}]);
    m["load_cases"] = json!([{"id": "W", "loads": [{"pattern": "WT", "scale": -0.8}]}]);
    m["load_combinations"] =
        json!([{"id": "COMB", "type": "linear-add", "items": [{"case": "W", "scale": 2.5}]}]);
    m
}

fn run(m: &Value) -> Results {
    analyze(&Model::from_json(&m.to_string()).unwrap()).unwrap()
}

#[test]
fn a_user_wind_load_acts_like_a_loaded_joint_at_its_point() {
    let (a, b) = (run(&user_wind()), run(&extra_joints()));
    let mut compared = 0;
    for case in ["W", "COMB"] {
        let (ca, cb) = (a.case(&case.into()).unwrap(), b.case(&case.into()).unwrap());
        let scale = cb
            .displacements
            .iter()
            .flat_map(|d| d.values.as_array())
            .fold(0.0f64, |m, v| m.max(v.abs()));
        for d in &ca.displacements {
            let (u, v) = (
                d.values.as_array(),
                cb.displacement(&d.node).unwrap().as_array(),
            );
            for k in 0..6 {
                assert!(
                    (u[k] - v[k]).abs() <= 1e-10 * scale,
                    "{case} {}: {u:?} vs {v:?}",
                    d.node
                );
            }
            compared += 1;
        }
        // Statics: the base takes the loads times the factors.
        let f = if case == "W" { -0.8 } else { -2.0 };
        let fx: f64 = ca.reactions.iter().map(|r| r.values.fx).sum();
        let fy: f64 = ca.reactions.iter().map(|r| r.values.fy).sum();
        assert!((fx + f * 26.0).abs() < 1e-9 && (fy + f * 5.0).abs() < 1e-9);
        assert!(ca.equilibrium_ok());
    }
    assert_eq!(compared, 24);
    // No joint was added.
    assert!(a.cases[0]
        .displacements
        .iter()
        .all(|d| !d.node.0.starts_with('p')));
}

#[test]
fn bad_user_wind_loads_are_refused() {
    let err = |change: &dyn Fn(&mut Value)| {
        let mut m = user_wind();
        change(&mut m);
        match Model::from_json(&m.to_string()) {
            Err(e) => e.to_string(),
            Ok(model) => analyze(&model).unwrap_err().to_string(),
        }
    };
    assert!(
        err(&|m| m["load_patterns"][0]["wind_loads"][0]["story"] = json!("Roof"))
            .contains("unknown story Roof")
    );
    assert!(
        err(&|m| m["load_patterns"][0]["wind_loads"][0]["diaphragm"] = json!("D9"))
            .contains("unknown diaphragm D9")
    );
    assert!(err(&|m| m["load_patterns"][0]["type"] = json!("live")).contains("need a Wind pattern"));
    assert!(err(&|m| m["load_patterns"][0]
        .as_object_mut()
        .unwrap()
        .remove("auto")
        .map(|_| ())
        .unwrap())
    .contains("need a Wind pattern"));
    assert!(err(&|m| m["stories"] = json!([])).contains("need the model's stories"));
    // Semi-rigid with no floor: spread by mass instead, and these columns have none.
    assert!(err(&|m| m["diaphragms"][0]["semi_rigid"] = json!(true)).contains("has no mass there"));
}

#[test]
fn a_user_wind_row_at_a_story_without_diaphragm_joints_is_skipped_with_a_warning() {
    // The same model with one more row at Base, where D1 has no joints: the run goes on, the row is
    // listed once, and the results are those of the model without it.
    let mut m = user_wind();
    m["load_patterns"][0]["wind_loads"]
        .as_array_mut()
        .unwrap()
        .push(json!({"story": "Base", "diaphragm": "D1", "fx": 50, "x": 1, "y": 1}));
    let (a, b) = (run(&m), run(&user_wind()));
    let warned: Vec<&String> = a
        .notes
        .iter()
        .filter(|n| n.contains("user wind loads skipped"))
        .collect();
    assert_eq!(warned.len(), 1, "{:?}", a.notes);
    assert!(warned[0].contains("load case W: Base, D1"), "{}", warned[0]);
    assert!(!b
        .notes
        .iter()
        .any(|n| n.contains("user wind loads skipped")));
    for case in ["W", "COMB"] {
        let (ca, cb) = (a.case(&case.into()).unwrap(), b.case(&case.into()).unwrap());
        for d in &ca.displacements {
            assert_eq!(
                d.values.as_array(),
                cb.displacement(&d.node).unwrap().as_array()
            );
        }
    }
}
