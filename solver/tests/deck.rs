//! Deck area sections: the weight per area (concrete per area x the fill's unit weight + the deck's unit
//! weight), the membrane each deck type is analysed as, and the validation messages. Against the reference
//! program: benchmarks B73 (deck floors on beams) and B74 (in-plane stiffness).

use osab_solver::Model;
use serde_json::{json, Value};

fn model(deck_section: Value) -> Value {
    json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2, "weight": 24.0},
                      {"id": "S", "E": 200e6, "nu": 0.3, "weight": 77.0}],
        "sections": [],
        "area_sections": [deck_section],
        "nodes": [{"id": "1"}, {"id": "2", "x": 4}, {"id": "3", "x": 4, "y": 2}, {"id": "4", "y": 2}],
        "areas": [{"id": "A", "nodes": ["1", "2", "3", "4"], "section": "D"}],
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"},
                     {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}],
        "load_cases": [{"id": "SW", "self_weight": 1.0}],
    })
}

fn filled() -> Value {
    json!({"id": "D", "type": "membrane", "material": "C", "thickness": 0.14, "property_type": "deck",
           "one_way": true,
           "deck": {"type": "filled", "steel": "S", "slab_depth": 0.065, "rib_depth": 0.075,
                    "rib_width_top": 0.15, "rib_width_bottom": 0.12, "rib_spacing": 0.3,
                    "shear_thickness": 0.001, "unit_weight": 0.1}})
}

fn read(v: &Value) -> Result<Model, String> {
    Model::from_json(&v.to_string()).map_err(|e| e.to_string())
}

#[test]
fn weight_per_area_follows_the_deck() {
    let m = read(&model(filled())).unwrap();
    let a = &m.areas[0];
    // 0.065 + 0.075 x 0.135 / 0.3 = 0.09875 m of concrete.
    assert!((m.area_weight(a) - (0.09875 * 24.0 + 0.1)).abs() < 1e-12);
    assert!((m.area_mass_weight(a) - (0.09875 * 24.0 + 0.1)).abs() < 1e-12);
    let mut v = filled();
    v["deck"]["type"] = json!("unfilled");
    let m = read(&model(v.clone())).unwrap();
    assert!((m.area_weight(&m.areas[0]) - 0.1).abs() < 1e-12);
    v["deck"]["type"] = json!("solid");
    v["deck"]["slab_depth"] = json!(0.15);
    let m = read(&model(v)).unwrap();
    assert!((m.area_weight(&m.areas[0]) - (0.15 * 24.0 + 0.1)).abs() < 1e-12);
}

#[test]
fn each_deck_type_is_its_own_membrane() {
    let m = read(&model(filled())).unwrap();
    assert_eq!(m.area_sections[0].analysed(), (&"C".into(), 0.065));
    let mut v = filled();
    v["deck"]["type"] = json!("unfilled");
    let m = read(&model(v)).unwrap();
    assert_eq!(m.area_sections[0].analysed(), (&"S".into(), 0.001));
}

#[test]
fn a_deck_must_be_a_one_way_membrane_with_its_data() {
    let mut v = filled();
    v["type"] = json!("shell-thin");
    assert!(read(&model(v)).unwrap_err().contains("always a Membrane"));
    let mut v = filled();
    v["one_way"] = json!(false);
    assert!(read(&model(v)).unwrap_err().contains("always a Membrane"));
    let mut v = filled();
    v.as_object_mut().unwrap().remove("deck");
    assert!(read(&model(v)).unwrap_err().contains("deck data"));
    let mut v = filled();
    v["deck"]["steel"] = json!("X");
    assert!(read(&model(v))
        .unwrap_err()
        .contains("unknown deck material X"));
    let mut v = filled();
    v["deck"]["rib_spacing"] = json!(0);
    assert!(read(&model(v)).unwrap_err().contains("rib spacing"));
    let mut v = filled();
    v["deck"]["rib_depth"] = json!(-0.01);
    assert!(read(&model(v)).unwrap_err().contains("0 or more"));
}
