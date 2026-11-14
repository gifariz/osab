//! Unstable models (a direction without stiffness: a part that is not held, a mechanism) run with a
//! warning by default: a tiny stiffness is put in each such direction, the directions are listed in the
//! messages and the results, and the rest of the structure is solved as if the unstable part were not
//! there. With `"on_instability": "stop"` the analysis stops at the first such joint and direction.

use osab_solver::results::Results;
use osab_solver::{analyze, Error, Model};
use serde_json::{json, Value};

fn model(nodes: Value, frames: Value, supports: Value, loads: Value, extra: Value) -> Model {
    let mut m = json!({
        "materials": [{"id": "S", "E": 200e6, "nu": 0.3}],
        "sections": [{"id": "R", "A": 0.01, "I22": 1e-4, "I33": 2e-4, "J": 1e-4}],
        "nodes": nodes, "frames": frames, "supports": supports,
        "load_cases": [{"id": "C", "node_loads": loads}],
    });
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    Model::from_json(&m.to_string()).unwrap()
}

fn frame(id: &str, i: &str, j: &str) -> Value {
    json!({"id": id, "type": "frame", "nodes": [i, j], "material": "S", "section": "R"})
}

fn warning(r: &Results) -> &String {
    r.notes
        .iter()
        .find(|n| n.starts_with("Warning: the structure is unstable"))
        .expect("the warning")
}

fn disp(r: &Results, node: &str) -> [f64; 6] {
    r.cases[0]
        .displacements
        .iter()
        .find(|d| d.node.to_string() == node)
        .unwrap()
        .values
        .as_array()
}

/// A beam held by nothing: six directions (its rigid-body motions) get a tiny stiffness, the run
/// finishes, and the load on it shows as an absurd displacement.
#[test]
fn floating_beam_runs_with_a_warning() {
    let m = model(
        json!([{"id": "1"}, {"id": "2", "x": 5}]),
        json!([frame("B", "1", "2")]),
        json!([]),
        json!([{"node": "2", "fz": -10}]),
        json!({}),
    );
    let r = analyze(&m).unwrap();
    assert_eq!(r.instabilities.len(), 6, "{:?}", r.instabilities);
    for i in &r.instabilities {
        assert!(i.digits_lost >= 12.0 && i.digits_lost <= 16.0, "{i:?}");
    }
    let w = warning(&r);
    assert!(w.contains("6 direction(s)"), "{w}");
    assert!(w.contains("not reliable"), "{w}");
    for i in &r.instabilities {
        assert!(w.contains(&format!("{}:{}", i.node, i.dof)), "{w}");
    }
    // Nothing holds the load: the beam moves by far more than it could bend (10 kN on a 5 m
    // cantilever of this section gives about 0.01 m).
    let uz = disp(&r, "2")[2].abs();
    assert!(uz > 1e3, "{uz:e}");
    // The list is in the results as written to a file.
    let text = serde_json::to_string(&r).unwrap();
    assert!(
        text.contains("\"instabilities\":[{\"node\":"),
        "{}",
        &text[..200]
    );
}

/// A mechanism: a member on one pin turns freely about it (three directions).
#[test]
fn mechanism_runs_with_a_warning() {
    let m = model(
        json!([{"id": "1"}, {"id": "2", "x": 3, "z": 4}]),
        json!([frame("B", "1", "2")]),
        json!([{"node": "1", "fix": "pinned"}]),
        json!([{"node": "2", "fx": 4, "fz": 3}]),
        json!({}),
    );
    let r = analyze(&m).unwrap();
    assert_eq!(r.instabilities.len(), 3, "{:?}", r.instabilities);
    warning(&r);
}

fn portal(loose: bool, extra: Value) -> Model {
    let mut nodes = vec![
        json!({"id": "1"}),
        json!({"id": "2", "z": 4}),
        json!({"id": "3", "x": 6, "z": 4}),
        json!({"id": "4", "x": 6}),
    ];
    let mut frames = vec![
        frame("C1", "1", "2"),
        frame("G", "2", "3"),
        frame("C2", "4", "3"),
    ];
    let mut loads = vec![
        json!({"node": "2", "fx": 20}),
        json!({"node": "3", "fz": -50}),
    ];
    if loose {
        nodes.push(json!({"id": "8", "x": 10, "z": 1}));
        nodes.push(json!({"id": "9", "x": 13, "z": 1}));
        frames.push(frame("L", "8", "9"));
        loads.push(json!({"node": "9", "fz": -5}));
    }
    model(
        json!(nodes),
        json!(frames),
        json!([{"node": "1", "fix": "fixed"}, {"node": "4", "fix": "fixed"}]),
        json!(loads),
        extra,
    )
}

/// A loose, loaded beam next to a stable frame: the frame's results are those of the frame alone.
#[test]
fn stable_part_is_not_changed_by_a_loose_object() {
    let alone = analyze(&portal(false, json!({}))).unwrap();
    assert!(alone.instabilities.is_empty());
    assert!(!alone.notes.iter().any(|n| n.contains("unstable")));
    let both = analyze(&portal(true, json!({}))).unwrap();
    assert_eq!(both.instabilities.len(), 6);
    assert!(both
        .instabilities
        .iter()
        .all(|i| ["8", "9"].contains(&i.node.to_string().as_str())));
    let close = |a: f64, b: f64, what: &str| {
        assert!(
            (a - b).abs() <= 1e-9 * b.abs().max(1e-9),
            "{what}: {a:e} vs {b:e}"
        )
    };
    for n in ["1", "2", "3", "4"] {
        for (a, b) in disp(&both, n).iter().zip(disp(&alone, n)) {
            close(*a, b, &format!("displacement at {n}"));
        }
    }
    for (fa, fb) in alone.cases[0].frames.iter().zip(&both.cases[0].frames) {
        for (a, b) in fb.end_forces.iter().zip(fa.end_forces) {
            close(*a, b, &format!("end force of {}", fa.element));
        }
    }
    for (ra, rb) in alone.cases[0]
        .reactions
        .iter()
        .zip(&both.cases[0].reactions)
    {
        assert_eq!(ra.node, rb.node);
        for (a, b) in rb.values.as_array().iter().zip(ra.values.as_array()) {
            close(*a, b, &format!("reaction at {}", ra.node));
        }
    }
}

/// The option to stop: the first joint and direction, as an error.
#[test]
fn stop_option_refuses_an_unstable_model() {
    let err = analyze(&portal(true, json!({"on_instability": "stop"}))).expect_err("refused");
    match &err {
        Error::Unstable { node, .. } => {
            assert!(["8", "9"].contains(&node.to_string().as_str()), "{err}")
        }
        _ => panic!("{err}"),
    }
    assert!(err.to_string().contains("unstable"));
    // A stable model is the same with either setting, and the default is not written to a file.
    let a = analyze(&portal(false, json!({"on_instability": "stop"}))).unwrap();
    let b = analyze(&portal(false, json!({"on_instability": "continue"}))).unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    let saved = serde_json::to_string(&portal(false, json!({}))).unwrap();
    assert!(!saved.contains("on_instability"));
    let saved = serde_json::to_string(&portal(false, json!({"on_instability": "stop"}))).unwrap();
    assert!(saved.contains("\"on_instability\":\"stop\""));
}
