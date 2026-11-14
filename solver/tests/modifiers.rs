//! Property modifiers on frames (closed-form cantilever, section times object:
//! docs/phases/PHASE4_PLAN.md), the Stiff slab type expressed as modifiers, the JSON format
//! and the validation messages.

use osab_solver::model::{AreaModifiers, FrameModifiers};
use osab_solver::{analyze, Model};
use serde_json::{json, Value};

const E: f64 = 25e6;
const G: f64 = E / 2.4;
const L: f64 = 3.0;
const SEC: [f64; 6] = [0.25, 0.2, 0.2, 0.0088, 0.0052, 0.0052]; // A As2 As3 J I22 I33

/// A vertical 3 m cantilever (local 2 = +X) with tip loads 5 X, 3 Y, -50 Z and a torque of 2 about Z.
fn cantilever(section_mods: Value, frame_mods: Value) -> Value {
    json!({
        "materials": [{"id": "C", "E": E, "G": G}],
        "sections": [{"id": "S", "A": SEC[0], "As2": SEC[1], "As3": SEC[2], "J": SEC[3], "I22": SEC[4], "I33": SEC[5],
                      "modifiers": section_mods}],
        "nodes": [{"id": "1"}, {"id": "2", "z": L}],
        "frames": [{"id": "C1", "type": "frame", "nodes": ["1", "2"], "material": "C", "section": "S",
                    "modifiers": frame_mods}],
        "supports": [{"node": "1", "fix": "fixed"}],
        "load_cases": [{"id": "TIP", "node_loads": [{"node": "2", "fx": 5, "fy": 3, "fz": -50, "mz": 2}]}],
    })
}

fn tip(model: &Value) -> [f64; 6] {
    let res = analyze(&Model::from_json(&model.to_string()).unwrap()).unwrap();
    res.case(&"TIP".into())
        .unwrap()
        .displacement(&"2".into())
        .unwrap()
        .as_array()
}

/// Closed form with each section value times its modifier `m` (A As2 As3 J I22 I33).
fn hand(m: [f64; 6]) -> [f64; 4] {
    let [a, as2, as3, j, i22, i33] = std::array::from_fn(|k| SEC[k] * m[k]);
    [
        5.0 * L.powi(3) / (3.0 * E * i33) + 5.0 * L / (G * as2),
        3.0 * L.powi(3) / (3.0 * E * i22) + 3.0 * L / (G * as3),
        -50.0 * L / (E * a),
        2.0 * L / (G * j),
    ]
}

fn assert_close(u: [f64; 6], h: [f64; 4]) {
    for (ours, want) in [u[0], u[1], u[2], u[5]].iter().zip(h) {
        assert!(
            (ours - want).abs() <= 1e-10 * want.abs(),
            "{ours:e} vs {want:e}"
        );
    }
}

#[test]
fn frame_modifiers_scale_their_own_terms_and_multiply() {
    let m = json!({"A": 0.3, "As2": 0.6, "As3": 0.7, "J": 0.2, "I22": 0.4, "I33": 0.5, "mass": 0.8, "weight": 0.9});
    let v = [0.3, 0.6, 0.7, 0.2, 0.4, 0.5];
    assert_close(tip(&cantilever(json!({}), json!({}))), hand([1.0; 6]));
    assert_close(tip(&cantilever(m.clone(), json!({}))), hand(v));
    assert_close(tip(&cantilever(json!({}), m.clone())), hand(v));
    assert_close(tip(&cantilever(m.clone(), m)), hand(v.map(|x| x * x)));
}

#[test]
fn missing_modifiers_are_one_and_ones_are_not_written() {
    let model = Model::from_json(&cantilever(json!({"I33": 0.35}), json!({})).to_string()).unwrap();
    assert_eq!(model.sections[0].modifiers.I33, 0.35);
    assert_eq!(model.sections[0].modifiers.A, 1.0);
    assert!(model.frames[0].modifiers.is_one());
    let back: Value = serde_json::from_str(&osab_solver::api::model_to_json(&model)).unwrap();
    assert_eq!(back["sections"][0]["modifiers"], json!({"I33": 0.35}));
    assert!(back["frames"][0].get("modifiers").is_none());
    assert_eq!(
        model.frame_modifiers(&model.frames[0]),
        FrameModifiers {
            I33: 0.35,
            ..Default::default()
        }
    );
}

/// A floor with a section and object modifiers.
fn floor_json(section: Value, area_mods: Value) -> Value {
    json!({
            "materials": [{"id": "C", "E": E, "nu": 0.2}],
            "sections": [],
            "area_sections": [section],
            "nodes": [{"id": "1"}, {"id": "2", "x": 2}, {"id": "3", "x": 2, "y": 2}, {"id": "4", "y": 2}],
            "frames": [],
            "areas": [{"id": "F1", "nodes": ["1", "2", "3", "4"], "section": "P", "modifiers": area_mods}],
            "supports": [{"node": "1", "fix": "fixed"}],
            "load_cases": [],
    })
}

fn floor(section: Value, area_mods: Value) -> Model {
    Model::from_json(&floor_json(section, area_mods).to_string()).unwrap()
}

/// The message a model is refused with (when it is read, else when it is analysed).
fn problem(model: Value) -> String {
    match Model::from_json(&model.to_string()) {
        Err(e) => e.to_string(),
        Ok(m) => analyze(&m).unwrap_err().to_string(),
    }
}

#[test]
fn area_modifiers_multiply_and_stiff_uses_fixed_values() {
    let slab = json!({"id": "P", "type": "shell-thin", "material": "C", "thickness": 0.2,
                      "modifiers": {"m11": 0.25, "m22": 0.25, "m12": 0.25, "weight": 0.5}});
    let m = floor(slab, json!({"m11": 2.0, "f11": 0.7}));
    let got = m.area_modifiers(&m.areas[0]);
    assert_eq!(
        got,
        AreaModifiers {
            f11: 0.7,
            m11: 0.5,
            m22: 0.25,
            m12: 0.25,
            weight: 0.5,
            ..Default::default()
        }
    );
    // A Stiff section's modifiers are replaced by bending x 100, no mass and no weight.
    let stiff = json!({"id": "P", "type": "shell-thin", "material": "C", "thickness": 0.2, "slab_type": "stiff",
                       "modifiers": {"m11": 0.25}});
    let m = floor(stiff, json!({"v13": 0.5}));
    assert_eq!(
        m.area_modifiers(&m.areas[0]),
        AreaModifiers {
            v13: 0.5,
            ..AreaModifiers::STIFF
        }
    );
    // A wall section has no slab type: a Stiff left in its file is not used.
    let wall = json!({"id": "P", "type": "shell-thin", "material": "C", "thickness": 0.2, "slab_type": "stiff",
                      "property_type": "wall", "modifiers": {"m11": 0.25}});
    let m = floor(wall, json!({}));
    assert_eq!(
        m.area_modifiers(&m.areas[0]),
        AreaModifiers {
            m11: 0.25,
            ..Default::default()
        }
    );
    assert_eq!(
        m.area_sections[0].property_type,
        Some(osab_solver::model::AreaPropertyType::Wall)
    );
}

#[test]
fn bad_modifiers_are_refused() {
    let err = |section: Value, frame: Value| problem(cantilever(section, frame));
    assert!(err(json!({"I33": 0.0}), json!({}))
        .contains("section S: property modifier I33 must be a number > 0"));
    assert!(err(json!({}), json!({"J": -1.0}))
        .contains("frame C1: property modifier J must be a number > 0"));
    assert!(err(json!({}), json!({"mass": -0.5}))
        .contains("frame C1: property modifier mass must be a number >= 0"));
    // Mass and weight may be 0 (the Stiff type uses that).
    let ok = cantilever(json!({"mass": 0.0, "weight": 0.0}), json!({}));
    assert!(analyze(&Model::from_json(&ok.to_string()).unwrap()).is_ok());
    let slab = json!({"id": "P", "type": "shell-thin", "material": "C", "thickness": 0.2, "modifiers": {"f12": 0.0}});
    let e = problem(floor_json(slab, json!({})));
    assert!(
        e.contains("area section P: property modifier f12 must be a number > 0"),
        "{e}"
    );
    let slab = json!({"id": "P", "type": "shell-thin", "material": "C", "thickness": 0.2});
    let e = problem(floor_json(slab, json!({"v23": -2.0})));
    assert!(
        e.contains("area F1: property modifier v23 must be a number > 0"),
        "{e}"
    );
}
