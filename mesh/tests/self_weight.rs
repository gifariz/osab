//! Self-weight through the mesh step: a Membrane floor's weight goes to its beams like its area loads,
//! and where floors overlap only the meshed object weighs (docs/phases/PHASE4_PLAN.md).

use osab_solver::{Model, Results};
use serde_json::{json, Value};


fn run(m: &Value) -> Results {
    osab_mesh::run(&Model::from_json(&m.to_string()).unwrap()).unwrap()
}


#[test]
fn overlapping_floors_weigh_once() {
    // A 6 x 6 m floor 0.2 m thick on four columns, a 2 x 2 m drop panel 0.4 m thick inside it.
    let m = json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2, "weight": 24}],
        "sections": [{"id": "COL", "A": 0.25, "I22": 5.2e-3, "I33": 5.2e-3, "J": 8.8e-3}],
        "area_sections": [{"id": "S", "type": "shell-thin", "material": "C", "thickness": 0.2},
                          {"id": "D", "type": "shell-thin", "material": "C", "thickness": 0.4, "slab_type": "drop"}],
        "nodes": [{"id": "1", "z": 3}, {"id": "2", "x": 6, "z": 3}, {"id": "3", "x": 6, "y": 6, "z": 3}, {"id": "4", "y": 6, "z": 3},
                  {"id": "5", "x": 2, "y": 2, "z": 3}, {"id": "6", "x": 4, "y": 2, "z": 3}, {"id": "7", "x": 4, "y": 4, "z": 3}, {"id": "8", "x": 2, "y": 4, "z": 3},
                  {"id": "11"}, {"id": "12", "x": 6}, {"id": "13", "x": 6, "y": 6}, {"id": "14", "y": 6}],
        "frames": [{"id": "C1", "type": "frame", "nodes": ["11", "1"], "material": "C", "section": "COL"},
                   {"id": "C2", "type": "frame", "nodes": ["12", "2"], "material": "C", "section": "COL"},
                   {"id": "C3", "type": "frame", "nodes": ["13", "3"], "material": "C", "section": "COL"},
                   {"id": "C4", "type": "frame", "nodes": ["14", "4"], "material": "C", "section": "COL"}],
        "areas": [{"id": "F1", "nodes": ["1", "2", "3", "4"], "section": "S"},
                  {"id": "F2", "nodes": ["5", "6", "7", "8"], "section": "D"}],
        "supports": [{"node": "11", "fix": "fixed"}, {"node": "12", "fix": "fixed"}, {"node": "13", "fix": "fixed"}, {"node": "14", "fix": "fixed"}],
        "load_cases": [{"id": "SW", "self_weight": 1}],
    });
    let r = run(&m);
    let total: f64 = r
        .case(&"SW".into())
        .unwrap()
        .reactions
        .iter()
        .map(|x| x.values.fz)
        .sum();
    let want = 24.0 * (4.0 * 0.25 * 3.0 + 0.2 * 32.0 + 0.4 * 4.0);
    assert!((total - want).abs() < 1e-9 * want, "{total} vs {want}");
}
