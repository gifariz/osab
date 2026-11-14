//! Self-weight (material weight per volume times the load pattern's multiplier): frames over
//! their length with the weight modifiers (not the A modifier), trusses half to each end, areas by
//! thickness, a Stiff section weightless, multipliers in combinations, validation.

use osab_solver::{analyze, Model};
use serde_json::{json, Value};

const W: f64 = 24.0;

fn model(extra: Value) -> Model {
    let mut m = json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2, "weight": W}],
        "sections": [{"id": "S", "A": 0.16, "I22": 2e-3, "I33": 2e-3, "J": 3e-3,
                      "modifiers": {"A": 0.3, "weight": 0.8}}],
        "nodes": [{"id": "1"}, {"id": "2", "z": 3}, {"id": "3", "x": 10}, {"id": "4", "x": 14}],
        "frames": [{"id": "C1", "type": "frame", "nodes": ["1", "2"], "material": "C", "section": "S"},
                   {"id": "T1", "type": "truss", "nodes": ["3", "4"], "material": "C", "section": "S"}],
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "3", "fix": "pinned"}, {"node": "4", "fix": "pinned"}],
        "load_cases": [{"id": "DEAD", "self_weight": 1}, {"id": "HALF", "self_weight": 0.5}],
        "load_combinations": [{"id": "ULS", "type": "linear-add", "items": [{"case": "DEAD", "scale": 1.2}, {"case": "HALF", "scale": 2}]}],
    });
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    Model::from_json(&m.to_string()).unwrap()
}

fn fz(r: &osab_solver::Results, case: &str, node: &str) -> f64 {
    r.case(&case.into())
        .unwrap()
        .reaction(&node.into())
        .unwrap()
        .fz
}

#[test]
fn frames_and_trusses_weigh_material_weight_times_area_and_weight_modifier() {
    let r = analyze(&model(json!({}))).unwrap();
    let col = W * 0.16 * 3.0 * 0.8;
    let truss = W * 0.16 * 4.0 * 0.8;
    assert!((fz(&r, "DEAD", "1") - col).abs() < 1e-9);
    assert!((fz(&r, "HALF", "1") - col / 2.0).abs() < 1e-9);
    assert!(
        (fz(&r, "DEAD", "3") - truss / 2.0).abs() < 1e-9
            && (fz(&r, "DEAD", "4") - truss / 2.0).abs() < 1e-9
    );
    // ULS = 1.2 DEAD + 2 HALF: the multipliers add up, 2.2 times the weight.
    assert!((fz(&r, "ULS", "1") - 2.2 * col).abs() < 1e-9);
    let c = r.case(&"DEAD".into()).unwrap();
    assert!(c.equilibrium_ok());
    let p = &c.frames[0].stations;
    assert!(
        (p[0].P + col).abs() < 1e-9 && p.last().unwrap().P.abs() < 1e-9,
        "axial force grows down the column"
    );
}

#[test]
fn areas_weigh_by_thickness_and_a_stiff_section_weighs_nothing() {
    let floor = |slab_type: &str| {
        model(json!({
            "area_sections": [{"id": "P", "type": "shell-thin", "material": "C", "thickness": 0.2, "slab_type": slab_type}],
            "nodes": [{"id": "1"}, {"id": "2", "x": 4}, {"id": "3", "x": 4, "y": 3}, {"id": "4", "y": 3}],
            "frames": [],
            "areas": [{"id": "F1", "nodes": ["1", "2", "3", "4"], "section": "P", "modifiers": {"weight": 0.5}}],
            "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"}, {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}],
            "load_combinations": [],
        }))
    };
    let r = analyze(&floor("slab")).unwrap();
    let total: f64 = ["1", "2", "3", "4"].iter().map(|n| fz(&r, "DEAD", n)).sum();
    assert!((total - W * 0.2 * 12.0 * 0.5).abs() < 1e-9);
    let r = analyze(&floor("stiff")).unwrap();
    let total: f64 = ["1", "2", "3", "4"].iter().map(|n| fz(&r, "DEAD", n)).sum();
    assert!(total.abs() < 1e-12);
}

#[test]
fn negative_weight_is_refused() {
    let m = json!({"materials": [{"id": "C", "E": 25e6, "nu": 0.2, "weight": -1}], "sections": [], "nodes": [],
                   "frames": [], "supports": [], "load_cases": []});
    let e = Model::from_json(&m.to_string()).unwrap_err().to_string();
    assert!(
        e.contains("material C: weight must be a number >= 0"),
        "{e}"
    );
}
