//! Groups (docs/phases/GROUPS_PLAN.md): kept in the model, checked against its objects, left out of the
//! analysis model (the mesh step's pieces have other names) so a run gives the same results.

use osab_solver::Model;

fn b42() -> serde_json::Value {
    let path = format!(
        "{}/../tests/benchmarks/b42_l_floor_beams/model.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn groups_change_no_result_and_unknown_members_are_refused() {
    let plain = osab_mesh::run(&serde_json::from_value::<Model>(b42()).unwrap()).unwrap();
    let mut v = b42();
    let frames: Vec<String> = v["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_string())
        .collect();
    v["groups"] = serde_json::json!([
        {"id": "BEAMS", "color": "#3f8fd2", "frames": frames.iter().filter(|f| f.starts_with('B')).collect::<Vec<_>>()},
        {"id": "FLOOR", "areas": ["F1"], "nodes": ["a"]}
    ]);
    let model: Model = serde_json::from_value(v.clone()).unwrap();
    model.check_groups().unwrap();
    // The beams are split by the floor's mesh: the groups do not follow, and nothing changes.
    let grouped = osab_mesh::run(&model).unwrap();
    assert_eq!(
        serde_json::to_string(&grouped.cases).unwrap(),
        serde_json::to_string(&plain.cases).unwrap()
    );
    // Kept on a round trip.
    let back: Model = serde_json::from_str(&osab_solver::api::model_to_json(&model)).unwrap();
    assert_eq!(back.groups.len(), 2);
    assert_eq!(back.groups[0].color.as_deref(), Some("#3f8fd2"));

    v["groups"][1]["frames"] = serde_json::json!(["NOPE"]);
    let bad: Model = serde_json::from_value(v.clone()).unwrap();
    // The run refuses it (the mesh step checks the groups before it leaves them out).
    assert!(osab_mesh::run(&bad)
        .unwrap_err()
        .to_string()
        .contains("group FLOOR: unknown object NOPE"));
    v["groups"][1]["frames"] = serde_json::json!([]);
    v["groups"][1]["id"] = "BEAMS".into();
    let twice: Model = serde_json::from_value(v).unwrap();
    assert!(twice
        .check_groups()
        .unwrap_err()
        .to_string()
        .contains("BEAMS"));
}
