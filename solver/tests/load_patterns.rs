//! Load patterns and load cases (docs/phases/LOAD_PATTERNS_PLAN.md): a case applies patterns times scale
//! factors (self-weight multipliers too), a pattern may be applied twice or not at all, an empty case
//! gives zero, files whose cases hold their own loads still work, the mass source takes patterns, and
//! bad references are refused. The model: one 3 m cantilever column.

use osab_solver::{analyze, Model, Results};
use serde_json::{json, Value};

const W: f64 = 24.0;
const A: f64 = 0.25;

fn base(extra: Value) -> Value {
    let mut m = json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2, "weight": W}],
        "sections": [{"id": "S", "A": A, "I22": 5.2e-3, "I33": 5.2e-3, "J": 8.8e-3}],
        "nodes": [{"id": "1"}, {"id": "2", "z": 3}],
        "frames": [{"id": "C1", "type": "frame", "nodes": ["1", "2"], "material": "C", "section": "S"}],
        "supports": [{"node": "1", "fix": "fixed"}],
        "load_patterns": [
            {"id": "Dead", "type": "dead", "self_weight": 1},
            {"id": "Live", "type": "live", "node_loads": [{"node": "2", "fz": -10}]},
            {"id": "SDL", "type": "super-dead", "node_loads": [{"node": "2", "fz": -4}]},
            {"id": "W", "type": "wind", "node_loads": [{"node": "2", "fx": 5}]},
            {"id": "SW2", "self_weight": 0.5, "node_loads": [{"node": "2", "fy": 2}]}
        ],
        "load_cases": [
            {"id": "Dead", "loads": [{"pattern": "Dead", "scale": 1}]},
            {"id": "C1", "loads": [{"pattern": "Dead", "scale": 2}, {"pattern": "Live", "scale": 0.5},
                                   {"pattern": "SDL", "scale": 1}, {"pattern": "W", "scale": -1}]},
            {"id": "TWICE", "loads": [{"pattern": "Dead", "scale": 1}, {"pattern": "Dead", "scale": 1}]},
            {"id": "EMPTY"},
            {"id": "SW2", "loads": [{"pattern": "SW2"}]},
            {"id": "MIXED", "node_loads": [{"node": "2", "fz": -1}], "loads": [{"pattern": "Live", "scale": 1}]}
        ],
        "load_combinations": [{"id": "COMB1", "type": "linear-add",
                               "items": [{"case": "C1", "scale": 1}, {"case": "Dead", "scale": 1}]}]
    });
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    m
}

fn model(extra: Value) -> Model {
    Model::from_json(&base(extra).to_string()).unwrap()
}

fn reaction(r: &Results, case: &str) -> [f64; 6] {
    let x = r.case(&case.into()).unwrap().reaction(&"1".into()).unwrap();
    [x.fx, x.fy, x.fz, x.mx, x.my, x.mz]
}

fn close(a: [f64; 6], b: [f64; 6]) {
    for d in 0..6 {
        assert!((a[d] - b[d]).abs() < 1e-9, "{a:?} != {b:?}");
    }
}

#[test]
fn a_case_applies_its_patterns_times_the_scale_factors() {
    let r = analyze(&model(json!({}))).unwrap();
    let w = W * A * 3.0;
    close(reaction(&r, "Dead"), [0.0, 0.0, w, 0.0, 0.0, 0.0]);
    // 2 Dead + 0.5 Live + SDL - W: as the probe (2 x 17.672 + 5 + 4 kN there).
    close(
        reaction(&r, "C1"),
        [5.0, 0.0, 2.0 * w + 9.0, 0.0, 15.0, 0.0],
    );
    close(reaction(&r, "TWICE"), [0.0, 0.0, 2.0 * w, 0.0, 0.0, 0.0]);
    close(reaction(&r, "EMPTY"), [0.0; 6]);
    // A second self-weight pattern (multiplier 0.5) with a joint load in Y.
    close(reaction(&r, "SW2"), [0.0, -2.0, 0.5 * w, 6.0, 0.0, 0.0]);
    // A case with loads of its own and a pattern: both act.
    close(reaction(&r, "MIXED"), [0.0, 0.0, 11.0, 0.0, 0.0, 0.0]);
    close(
        reaction(&r, "COMB1"),
        [5.0, 0.0, 3.0 * w + 9.0, 0.0, 15.0, 0.0],
    );
    let ids: Vec<String> = r.cases.iter().map(|c| c.id.0.clone()).collect();
    assert_eq!(
        ids,
        ["Dead", "C1", "TWICE", "EMPTY", "SW2", "MIXED", "COMB1"]
    );
}

#[test]
fn a_file_with_loads_on_its_cases_gives_the_same_results() {
    let legacy = Model::from_json(
        &base(json!({
            "load_patterns": [],
            "load_cases": [
                {"id": "Dead", "self_weight": 1},
                {"id": "C1", "self_weight": 2, "node_loads": [{"node": "2", "fz": -5}, {"node": "2", "fz": -4}, {"node": "2", "fx": -5}]}
            ],
            "load_combinations": []
        }))
        .to_string(),
    )
    .unwrap();
    let new = analyze(&model(json!({}))).unwrap();
    let old = analyze(&legacy).unwrap();
    for c in ["Dead", "C1"] {
        close(reaction(&new, c), reaction(&old, c));
    }
}

#[test]
fn the_mass_source_takes_patterns() {
    let modal = |source: Value, cases: Value, patterns: Value| {
        let m = model(json!({
            "load_patterns": patterns,
            "load_cases": cases,
            "load_combinations": [],
            "mass_source": source,
            "modal": {"id": "Modal", "modes": 2}
        }));
        let r = analyze(&m).unwrap();
        let ms = r.modal.unwrap().masses;
        let top = ms.iter().find(|j| j.node.0 == "2").unwrap();
        [top.ux, top.uy, top.uz]
    };
    let g = 9.80665;
    let live = json!([{"id": "Live", "node_loads": [{"node": "2", "fz": -10}]},
                      {"id": "Dead", "self_weight": 1}]);
    let by_pattern = modal(
        json!({"elements": false, "loads": [{"pattern": "Live", "scale": 0.5}, {"pattern": "Dead"}]}),
        json!([]),
        live,
    );
    let w = W * A * 3.0;
    let want = 0.5 * 10.0 / g + w / 2.0 / g;
    assert!((by_pattern[0] - want).abs() < 1e-12 && (by_pattern[1] - want).abs() < 1e-12);
    // The same from load cases (files without patterns).
    let by_case = modal(
        json!({"elements": false, "loads": [{"case": "L", "scale": 0.5}, {"case": "D"}]}),
        json!([{"id": "L", "node_loads": [{"node": "2", "fz": -10}]}, {"id": "D", "self_weight": 1}]),
        json!([]),
    );
    assert_eq!(by_pattern, by_case);
}

#[test]
fn bad_patterns_are_refused() {
    let err = |extra: Value| {
        Model::from_json(&base(extra).to_string())
            .unwrap_err()
            .to_string()
    };
    assert!(
        err(json!({"load_cases": [{"id": "X", "loads": [{"pattern": "NOPE"}]}]}))
            .contains("load case X: unknown load pattern NOPE")
    );
    assert!(err(json!({"load_patterns": [{"id": "P"}, {"id": "P"}]}))
        .contains("duplicate load pattern id P"));
    assert!(err(
        json!({"load_patterns": [{"id": "P", "node_loads": [{"node": "9", "fz": 1}]}],
                       "load_cases": [], "load_combinations": []})
    )
    .contains("load pattern P: unknown node 9"));
    assert!(
        err(json!({"mass_source": {"loads": [{"pattern": "NOPE"}]}}))
            .contains("mass source: unknown load pattern NOPE")
    );
    assert!(
        err(json!({"mass_source": {"loads": [{"pattern": "Live", "case": "C1"}]}}))
            .contains("each item names one load pattern")
    );
    assert!(
        err(json!({"load_patterns": [{"id": "P", "type": "earthquake"}]}))
            .contains("unknown variant")
    );
}

#[test]
fn pattern_types_and_round_trip() {
    let types = [
        "dead",
        "super-dead",
        "live",
        "reducible-live",
        "roof-live",
        "snow",
        "seismic",
        "wind",
        "temperature",
        "hydrostatic",
        "construction",
        "notional",
        "other",
    ];
    let patterns: Vec<Value> = types.iter().map(|t| json!({"id": t, "type": t})).collect();
    let m = model(json!({"load_patterns": patterns, "load_cases": [], "load_combinations": []}));
    let back = serde_json::to_value(&m).unwrap();
    for (p, t) in back["load_patterns"].as_array().unwrap().iter().zip(types) {
        assert_eq!(p["type"], t);
    }
    // Missing type = other; the expanded model has no patterns and the same cases.
    let m = model(json!({}));
    let e = m.with_patterns().unwrap();
    assert!(e.load_patterns.is_empty() && e.load_cases.iter().all(|c| c.loads.is_empty()));
    assert_eq!(
        serde_json::to_value(&m).unwrap()["load_patterns"][4]["type"],
        "other"
    );
    assert!(matches!(
        e.with_patterns().unwrap(),
        std::borrow::Cow::Borrowed(_)
    ));
}
