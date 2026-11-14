//! The check before factorizing that the analysis fits in memory (`osab_solver::memory`): it stops
//! with a message of its own when the estimate is more than the free memory less a reserve, goes on
//! when forced, and changes nothing when there is room. One test: the free memory given here is a
//! setting of the whole process.

use osab_solver::{analyze, memory, Error, Model};
use serde_json::json;

#[test]
fn stops_with_a_message_when_the_analysis_will_not_fit() {
    let model = Model::from_json(
        &json!({
            "materials": [{"id": "S", "E": 200e6, "nu": 0.3}],
            "sections": [{"id": "R", "A": 0.01, "I22": 1e-4, "I33": 2e-4, "J": 1e-4}],
            "nodes": [{"id": "1"}, {"id": "2", "z": 4}, {"id": "3", "x": 6, "z": 4}, {"id": "4", "x": 6}],
            "frames": [
                {"id": "C1", "type": "frame", "nodes": ["1", "2"], "material": "S", "section": "R"},
                {"id": "G", "type": "frame", "nodes": ["2", "3"], "material": "S", "section": "R"},
                {"id": "C2", "type": "frame", "nodes": ["4", "3"], "material": "S", "section": "R"}],
            "supports": [{"node": "1", "fix": "fixed"}, {"node": "4", "fix": "fixed"}],
            "load_cases": [{"id": "A", "node_loads": [{"node": "2", "fx": 20}]},
                           {"id": "B", "node_loads": [{"node": "3", "fz": -30}]}],
            "load_combinations": [
                {"id": "ENV", "type": "envelope", "items": [{"case": "A", "scale": 1}, {"case": "B", "scale": 1}]}],
        })
        .to_string(),
    )
    .unwrap();
    let json = |m: &Model| serde_json::to_string(&analyze(m).unwrap()).unwrap();

    // Room enough (whatever this computer has free, or a given 64 GB): the results.
    let free = json(&model);
    memory::set_available(Some(64 << 30));
    assert_eq!(json(&model), free);

    // 1 GB free: nothing may be used of it (the reserve), so even this model is stopped.
    memory::set_available(Some(1 << 30));
    let err = analyze(&model).expect_err("stopped");
    match &err {
        Error::TooBigForMemory {
            needed,
            free,
            equations,
            ..
        } => {
            assert!(*needed > 0.0 && (*free - 1.0).abs() < 1e-12);
            assert_eq!(*equations, 12);
        }
        other => panic!("{other}"),
    }
    let text = err.to_string();
    assert!(
        text.contains("GB of memory") && text.contains("coarser mesh"),
        "{text}"
    );
    assert_eq!(
        osab_solver::api::ErrorReport::from(err).kind,
        "too_big_for_memory"
    );

    // Forced: the same results as with room.
    memory::set_force(true);
    assert_eq!(json(&model), free);
    memory::set_force(false);
    assert!(analyze(&model).is_err());

    // Back to asking the system.
    memory::set_available(None);
    assert_eq!(json(&model), free);
}
