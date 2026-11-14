//! The model file's format version: files without one (older) and with the current one are read; a file from a
//! newer OSAB is refused with a message that says so, by the solver and by the mesh step.

use osab_solver::model::MODEL_FORMAT;
use osab_solver::Model;
use serde_json::json;

fn cantilever(version: Option<u32>) -> String {
    let mut m = json!({
        "materials": [{"id": "S", "E": 200e6, "nu": 0.3}],
        "sections": [{"id": "R", "A": 0.01, "I22": 1e-5, "I33": 2e-5, "J": 1e-5}],
        "nodes": [{"id": "1"}, {"id": "2", "x": 3}],
        "frames": [{"id": "B", "type": "frame", "nodes": ["1", "2"], "material": "S", "section": "R"}],
        "supports": [{"node": "1", "fix": "fixed"}],
        "load_cases": [{"id": "P", "node_loads": [{"node": "2", "fz": -10}]}],
    });
    if let Some(v) = version {
        m["version"] = json!(v);
    }
    m.to_string()
}

#[test]
fn older_and_current_files_are_read() {
    for v in [None, Some(MODEL_FORMAT)] {
        let m = Model::from_json(&cantilever(v)).unwrap();
        assert_eq!(m.version, v);
        osab_solver::analyze(&m).unwrap();
        osab_mesh::run(&m).unwrap();
    }
    // Written back with the version it was read with.
    let m = Model::from_json(&cantilever(Some(MODEL_FORMAT))).unwrap();
    assert_eq!(
        serde_json::to_value(&m).unwrap()["version"],
        json!(MODEL_FORMAT)
    );
}

#[test]
fn a_file_from_a_newer_osab_is_refused() {
    let text = cantilever(Some(MODEL_FORMAT + 1));
    let err = Model::from_json(&text).unwrap_err().to_string();
    assert!(
        err.contains("newer OSAB") && err.contains(&format!("file format {}", MODEL_FORMAT + 1)),
        "{err}"
    );
    let m: Model = serde_json::from_str(&text).unwrap();
    let err = osab_mesh::run(&m).unwrap_err().to_string();
    assert!(err.contains("newer OSAB"), "{err}");
}
