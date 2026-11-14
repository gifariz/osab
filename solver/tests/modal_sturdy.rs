//! The modal case on awkward models: directions without stiffness are held (they do not become modes
//! of their own), equal modes do not upset the count of eigenvalues, and a modal case that cannot be
//! made does not take the static cases with it.

use osab_solver::results::Results;
use osab_solver::{analyze, Model};
use serde_json::{json, Value};

fn frame(id: &str, i: &str, j: &str) -> Value {
    json!({"id": id, "type": "frame", "nodes": [i, j], "material": "S", "section": "R"})
}

fn mass(node: &str, m: f64) -> Value {
    json!({"node": node, "ux": m, "uy": m, "uz": m})
}

/// A portal frame (columns 4 m, beam 6 m) with a mass at each top joint, and when asked a loose beam
/// beside it with small masses of its own.
fn portal(loose: bool, modes: usize, extra: Value) -> Model {
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
    let mut masses = vec![mass("2", 10.0), mass("3", 12.0)];
    if loose {
        nodes.push(json!({"id": "8", "x": 10, "z": 1}));
        nodes.push(json!({"id": "9", "x": 13, "z": 1}));
        frames.push(frame("L", "8", "9"));
        masses.push(mass("8", 0.02));
        masses.push(mass("9", 0.03));
    }
    let mut m = json!({
        "materials": [{"id": "S", "E": 200e6, "nu": 0.3}],
        "sections": [{"id": "R", "A": 0.01, "I22": 1e-4, "I33": 2e-4, "J": 1e-4}],
        "nodes": nodes, "frames": frames, "masses": masses,
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "4", "fix": "fixed"}],
        "load_cases": [{"id": "C", "node_loads": [{"node": "2", "fx": 20}]}],
        "modal": {"id": "Modal", "modes": modes},
    });
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    Model::from_json(&m.to_string()).unwrap()
}

fn periods(r: &Results) -> Vec<f64> {
    r.modal
        .as_ref()
        .expect("modal results")
        .modes
        .iter()
        .map(|m| m.period)
        .collect()
}

/// A loose beam beside the frame: its six directions without stiffness are held, the frame's periods
/// are those of the frame alone, and no mode has a period beyond the frame's first.
#[test]
fn loose_part_does_not_change_the_frames_periods() {
    let alone = periods(&analyze(&portal(false, 4, json!({}))).unwrap());
    let r = analyze(&portal(true, 4, json!({}))).unwrap();
    assert_eq!(r.instabilities.len(), 6);
    let both = periods(&r);
    assert_eq!(both.len(), 4);
    for (a, b) in both.iter().zip(&alone) {
        assert!((a - b).abs() <= 1e-9 * b, "{both:?} vs {alone:?}");
    }
    let note = r
        .notes
        .iter()
        .find(|n| n.starts_with("Modal case Modal: 6 direction(s) without stiffness were held"))
        .expect("the note about held directions");
    assert!(note.contains("not reliable"), "{note}");
    // The static case is as before: the loose beam's joints are listed, the frame is solved.
    assert!(r.failed.is_empty());
}

/// More modes asked for than the frame has: the loose beam's own modes (it is held where it had no
/// stiffness) follow the frame's, and the run finishes.
#[test]
fn more_modes_than_the_stable_part_has() {
    let alone = periods(&analyze(&portal(false, 6, json!({}))).unwrap());
    let r = analyze(&portal(true, 12, json!({}))).unwrap();
    let both = periods(&r);
    assert!(both.len() > alone.len(), "{both:?}");
    for (a, b) in both.iter().zip(&alone) {
        assert!((a - b).abs() <= 1e-9 * b, "{both:?} vs {alone:?}");
    }
    // Held directions make no mode with a huge period.
    assert!(
        both.iter().all(|t| *t <= alone[0] * (1.0 + 1e-9)),
        "{both:?}"
    );
}

/// A square frame has equal modes in X and Y. Asking for one mode splits the pair; the count of
/// eigenvalues below it must still come out.
#[test]
fn equal_modes_split_by_the_number_asked() {
    let m = json!({
        "materials": [{"id": "S", "E": 200e6, "nu": 0.3}],
        "sections": [{"id": "R", "A": 0.01, "I22": 2e-4, "I33": 2e-4, "J": 1e-4}],
        "nodes": [{"id": "1"}, {"id": "2", "x": 5}, {"id": "3", "x": 5, "y": 5}, {"id": "4", "y": 5},
                  {"id": "5", "z": 3}, {"id": "6", "x": 5, "z": 3}, {"id": "7", "x": 5, "y": 5, "z": 3},
                  {"id": "8", "y": 5, "z": 3}],
        "frames": [frame("C1", "1", "5"), frame("C2", "2", "6"), frame("C3", "3", "7"), frame("C4", "4", "8"),
                   frame("B1", "5", "6"), frame("B2", "6", "7"), frame("B3", "8", "7"), frame("B4", "5", "8")],
        "masses": [mass("5", 5.0), mass("6", 5.0), mass("7", 5.0), mass("8", 5.0)],
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"},
                     {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}],
        "load_cases": [{"id": "C", "node_loads": [{"node": "5", "fx": 1}]}],
    });
    let run = |modes: usize| {
        let mut m = m.clone();
        m["modal"] = json!({"id": "Modal", "modes": modes});
        analyze(&Model::from_json(&m.to_string()).unwrap()).unwrap()
    };
    let three = periods(&run(3));
    assert!((three[0] - three[1]).abs() <= 1e-9 * three[0], "{three:?}");
    for modes in [1, 2] {
        let r = run(modes);
        let t = periods(&r);
        assert_eq!(t.len(), modes);
        assert!(
            (t[0] - three[0]).abs() <= 1e-9 * three[0],
            "{t:?} vs {three:?}"
        );
        assert!(r.failed.is_empty(), "{:?}", r.failed);
        assert!(!r.notes.iter().any(|n| n.contains("could not be completed")));
    }
}

/// A modal case that cannot give what the response spectrum cases need (here: no mass anywhere) is
/// named as failed with them; the static cases and the combinations without them have their results.
#[test]
fn a_modal_failure_keeps_the_static_results() {
    let m = portal(
        false,
        4,
        json!({
            "masses": [],
            "load_cases": [{"id": "A", "node_loads": [{"node": "2", "fx": 20}]},
                           {"id": "B", "node_loads": [{"node": "3", "fz": -30}]}],
            "spectrum_functions": [{"id": "F", "points": [[0, 1], [1, 1]]}],
            "spectrum_cases": [{"id": "RSX", "loads": [{"dir": "X", "function": "F", "scale": 9.80665}]}],
            "load_combinations": [
                {"id": "AB", "type": "linear-add", "items": [{"case": "A", "scale": 1}, {"case": "B", "scale": 1.5}]},
                {"id": "ARS", "type": "linear-add", "items": [{"case": "A", "scale": 1}, {"case": "RSX", "scale": 1}]},
                {"id": "ENV", "type": "envelope", "items": [{"combo": "AB", "scale": 1}, {"combo": "ARS", "scale": 1}]}],
        }),
    );
    let r = analyze(&m).unwrap();
    let ids: Vec<String> = r.cases.iter().map(|c| c.id.to_string()).collect();
    assert_eq!(ids, ["A", "B", "AB"]);
    assert!(r.modal.is_none());
    assert_eq!(r.failed.len(), 1);
    assert_eq!(r.failed[0].case.to_string(), "Modal");
    assert!(r.failed[0].reason.contains("no modes"), "{:?}", r.failed);
    let text = r.notes.join(" | ");
    assert!(r.notes[0].starts_with("Warning: "), "{text}");
    for left_out in ["RSX", "ARS", "ENV"] {
        assert!(text.contains(left_out), "{left_out} not named in: {text}");
    }
    // The same model with nothing depending on the modal case is not a failure: a note only.
    let m = portal(false, 4, json!({"masses": []}));
    let r = analyze(&m).unwrap();
    assert!(r.failed.is_empty());
    assert!(
        r.notes.iter().any(|n| n.contains("not run")),
        "{:?}",
        r.notes
    );
}
