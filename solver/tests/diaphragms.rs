//! Rigid diaphragm rules (docs/phases/PHASE4_PLAN.md, step 6 and probe finding 5): one constraint per diaphragm
//! and story, a joint between levels joins the story above, masters and refused assignments.

use osab_solver::{analyze, Model};
use serde_json::{json, Value};

/// Four 3 m cantilever columns at the corners of a 6 x 4 plan, joints at 1.5 m and 3 m on each, the top
/// joints (and the 1.5 m joints when `mid`) in diaphragm D1; 10 kN in X at one top corner.
fn columns(mid: bool, stories: Value) -> Value {
    let corners = [(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)];
    let (mut nodes, mut frames, mut supports) = (Vec::new(), Vec::new(), Vec::new());
    for (k, (x, y)) in corners.iter().enumerate() {
        nodes.push(json!({"id": format!("b{k}"), "x": x, "y": y, "z": 0}));
        let mut m = json!({"id": format!("m{k}"), "x": x, "y": y, "z": 1.5});
        if mid {
            m["diaphragm"] = json!("D1");
        }
        nodes.push(m);
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
           "supports": supports, "diaphragms": [{"id": "D1"}], "stories": stories,
           "load_cases": [{"id": "L", "node_loads": [{"node": "t1", "fx": 10}]}]})
}

fn disp(m: &Value, node: &str) -> [f64; 6] {
    let r = analyze(&Model::from_json(&m.to_string()).unwrap()).unwrap();
    r.case(&"L".into())
        .unwrap()
        .displacement(&node.into())
        .unwrap()
        .as_array()
}

fn error(m: &Value) -> String {
    Model::from_json(&m.to_string()).unwrap_err().to_string()
}

#[test]
fn joints_between_levels_join_the_story_above() {
    let stories = json!([{"id": "Base", "z": 0}, {"id": "S1", "z": 3}]);
    let m = columns(true, stories);
    // All eight joints follow one master: the 1.5 m joints move with the 3 m ones in UX, UY and RZ.
    let t = disp(&m, "t0");
    for n in ["m0", "m1", "m2", "m3", "t1", "t2", "t3"] {
        let d = disp(&m, n);
        let r = [
            d[0] - t[0] + t[5] * (if n.ends_with(['2', '3']) { 4.0 } else { 0.0 }),
            d[5] - t[5],
        ];
        assert!(r.iter().all(|v| v.abs() < 1e-12), "{n}: {d:?} vs t0 {t:?}");
    }
    // Without stories each height is a level: the 1.5 m joints move apart from the tops.
    let m = columns(true, json!([]));
    assert!((disp(&m, "m0")[0] - disp(&m, "t0")[0]).abs() > 1e-6);
}

#[test]
fn tops_move_as_one_in_plane_and_keep_their_own_bending() {
    let m = columns(false, json!([]));
    let r = analyze(&Model::from_json(&m.to_string()).unwrap()).unwrap();
    let c = r.case(&"L".into()).unwrap();
    let d: Vec<[f64; 6]> = (0..4)
        .map(|k| {
            c.displacement(&format!("t{k}").as_str().into())
                .unwrap()
                .as_array()
        })
        .collect();
    // Rigid in plane: the X motion differs only by the twist, distances stay.
    let (u0, u3, th) = (d[0][0], d[3][0], d[0][5]);
    assert!((u3 - (u0 - th * 4.0)).abs() < 1e-12);
    assert!(d.iter().all(|x| (x[5] - th).abs() < 1e-15));
    // Own rotations about Y (a cantilever's tip rotation 3 u / 2h, its own: not tied).
    for x in &d {
        assert!((x[4] - 1.5 * x[0] / 3.0).abs() < 1e-12 * x[0].abs().max(1e-12));
    }
    assert!(r
        .notes
        .iter()
        .any(|n| n.contains("Rigid diaphragms: 1 constraints (4 joints)")));
}

#[test]
fn refused_assignments() {
    let mut m = columns(false, json!([]));
    m["nodes"][2]["diaphragm"] = json!("D9");
    assert!(error(&m).contains("unknown diaphragm D9"), "{}", error(&m));

    let mut m = columns(false, json!([]));
    m["supports"].as_array_mut().unwrap().extend([
        json!({"node": "t0", "fix": "pinned"}),
        json!({"node": "t1", "fix": ["ux"]}),
    ]);
    assert!(
        error(&m).contains("both restrained in UX, UY or RZ"),
        "{}",
        error(&m)
    );

    let mut m = columns(false, json!([]));
    m["links"] = json!([{"id": "L1", "type": "rigid", "nodes": ["t0", "m0"]}]);
    assert!(
        error(&m).contains("joined by a rigid link"),
        "{}",
        error(&m)
    );

    let mut m = columns(false, json!([{"id": "B", "z": 0}, {"id": "A", "z": 0}]));
    m["diaphragms"] = json!([{"id": "D1"}]);
    assert!(error(&m).contains("from the lowest up"), "{}", error(&m));
}

#[test]
fn a_restrained_joint_becomes_the_master() {
    // t2 held in UY only: the whole diaphragm level cannot move in Y there; X still follows the load.
    let mut m = columns(false, json!([]));
    m["supports"]
        .as_array_mut()
        .unwrap()
        .push(json!({"node": "t2", "fix": ["uy"]}));
    let d = disp(&m, "t2");
    assert!(d[1].abs() < 1e-15 && d[0] > 0.0);
}
