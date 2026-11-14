//! Load patterns through the mesh step (docs/phases/LOAD_PATTERNS_PLAN.md): a model whose cases apply
//! patterns gives the same results as the same model with the loads on the cases (files written before
//! patterns), for meshed floors, Membrane floors (load transfer and their mass from loads), null objects,
//! self-weight, combinations and mass from patterns.

use osab_solver::{Model, Results};
use serde_json::{json, Value};

fn run(m: &Value) -> Results {
    osab_mesh::run(&serde_json::from_value::<Model>(m.clone()).unwrap()).unwrap()
}

/// `m` with every load case turned into a pattern of the same name that the case applies once, and the
/// mass source naming patterns.
fn with_patterns(m: &Value) -> Value {
    let mut m = m.clone();
    let cases = m["load_cases"].as_array().cloned().unwrap_or_default();
    let mut patterns = Vec::new();
    let mut new_cases = Vec::new();
    for c in &cases {
        let mut p = c.clone();
        p["type"] = json!("other");
        patterns.push(p);
        new_cases.push(json!({"id": c["id"], "loads": [{"pattern": c["id"], "scale": 1}]}));
    }
    m["load_patterns"] = json!(patterns);
    m["load_cases"] = json!(new_cases);
    if let Some(loads) = m
        .get_mut("mass_source")
        .and_then(|s| s.get_mut("loads"))
        .and_then(|l| l.as_array_mut())
    {
        for l in loads {
            let name = l["case"].take();
            l.as_object_mut().unwrap().remove("case");
            l["pattern"] = name;
        }
    }
    m
}

fn same(a: &Results, b: &Results, what: &str) {
    assert_eq!(a.cases.len(), b.cases.len(), "{what}");
    for (x, y) in a.cases.iter().zip(&b.cases) {
        assert_eq!(x.id, y.id, "{what}");
        assert_eq!(x.displacements.len(), y.displacements.len(), "{what}");
        for (p, q) in x.displacements.iter().zip(&y.displacements) {
            let (u, v) = (p.values.as_array(), q.values.as_array());
            for d in 0..6 {
                assert!(
                    (u[d] - v[d]).abs() <= 1e-12 * (1e-9 + v[d].abs()),
                    "{what} {} {}: {u:?} vs {v:?}",
                    x.id,
                    p.node
                );
            }
        }
    }
    match (&a.modal, &b.modal) {
        (Some(x), Some(y)) => {
            assert_eq!(x.masses.len(), y.masses.len(), "{what}");
            for (p, q) in x.masses.iter().zip(&y.masses) {
                let (u, v) = (p.as_array(), q.as_array());
                for d in 0..6 {
                    assert!(
                        (u[d] - v[d]).abs() <= 1e-12 * (1.0 + v[d].abs()),
                        "{what} mass {}",
                        p.node
                    );
                }
            }
            for (p, q) in x.modes.iter().zip(&y.modes) {
                assert!(
                    (p.period - q.period).abs() <= 1e-12 * q.period,
                    "{what} period"
                );
            }
        }
        (None, None) => {}
        _ => panic!("{what}: modal results on one side only"),
    }
}

#[test]
fn benchmarks_give_the_same_results_with_patterns() {
    for b in [
        "b26_load_combinations",
        "b39_membrane_floor",
        "b41_null_objects",
        "b47_self_weight",
        "b51_floor_diaphragms",
        "b53_modal_building",
        "b56_one_way_floors",
    ] {
        let path = format!(
            "{}/../tests/benchmarks/{b}/model.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let m: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        same(&run(&with_patterns(&m)), &run(&m), b);
    }
}
