//! Pier and spandrel labels in the model and forces from the solver alone (no mesh step: every area its
//! own element). The rules: docs/phases/PIER_SPANDREL_PLAN.md.

use osab_solver::model::Id;
use osab_solver::Model;
use serde_json::json;

/// A 4 x 3 m wall on the X-Z plane with a column at its left edge, both labelled P (and the wall S),
/// loaded at its top right corner.
fn wall() -> serde_json::Value {
    json!({
        "units": {"force": "kN", "length": "m"},
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}],
        "sections": [{"id": "R", "A": 0.12, "I22": 0.0009, "I33": 0.0016, "J": 0.0019}],
        "area_sections": [{"id": "W", "type": "shell-thin", "material": "C", "thickness": 0.2}],
        "nodes": [
            {"id": "1", "x": 0, "y": 0, "z": 0}, {"id": "2", "x": 4, "y": 0, "z": 0},
            {"id": "3", "x": 4, "y": 0, "z": 3}, {"id": "4", "x": 0, "y": 0, "z": 3}
        ],
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"}],
        "frames": [{"id": "F", "type": "frame", "nodes": ["1", "4"], "material": "C", "section": "R", "pier": "P"}],
        "areas": [{"id": "A", "nodes": ["1", "2", "3", "4"], "section": "W", "pier": "P", "spandrel": "S"}],
        "pier_labels": [{"id": "P"}, {"id": "Q", "color": "#ff0000"}],
        "spandrel_labels": [{"id": "S", "multi_story": true}],
        "stories": [{"id": "Base", "z": 0}, {"id": "L1", "z": 3}],
        "load_cases": [{"id": "L", "node_loads": [{"node": "3", "fx": 11, "fy": 13, "fz": -17, "mx": 19, "my": 23, "mz": 29}]}]
    })
}

#[test]
fn labels_are_kept_and_checked() {
    let model = Model::from_json(&wall().to_string()).unwrap();
    let back: Model = serde_json::from_str(&osab_solver::api::model_to_json(&model)).unwrap();
    assert_eq!(back.pier_labels.len(), 2);
    assert_eq!(back.pier_labels[1].color.as_deref(), Some("#ff0000"));
    assert!(back.spandrel_labels[0].multi_story);
    assert_eq!(back.areas[0].spandrel, Some(Id::from("S")));
    assert_eq!(back.frames[0].pier, Some(Id::from("P")));

    let mut v = wall();
    v["frames"][0]["pier"] = "NOPE".into();
    let err = Model::from_json(&v.to_string()).unwrap_err().to_string();
    assert!(err.contains("frame F: unknown pier label NOPE"), "{err}");
    let mut v = wall();
    v["pier_labels"][1]["id"] = "P".into();
    assert!(Model::from_json(&v.to_string()).is_err());
}

/// The pier holds the whole structure, so its forces are the load moved to the cuts' centroids (the
/// wall's 4 x 0.2 strip and the column's 0.12 at x = 0: x = 0.8 / 0.92 x 2 = 1.73913).
#[test]
fn pier_forces_are_statics() {
    let model = Model::from_json(&wall().to_string()).unwrap();
    let r = osab_solver::analyze(&model).unwrap();
    assert_eq!(r.pier_sections.len(), 1);
    let c = 0.8 * 2.0 / 0.92;
    assert!((r.pier_sections[0].bottom[0] - c).abs() < 1e-12);
    assert_eq!(r.pier_sections[0].angle, 0.0);
    let case = r.case(&Id::from("L")).unwrap();
    let p = &case.pier_forces[0];
    assert_eq!((p.story.as_str(), p.pier.0.as_str()), ("L1", "P"));
    let arm = 4.0 - c;
    // Moment of the load about the top centroid: m + r x f, r = (arm, 0, 0); about the bottom one,
    // r = (arm, 0, 3).
    let top = [
        -17.0,
        11.0,
        13.0,
        29.0 + arm * 13.0,
        -19.0,
        23.0 + arm * 17.0,
    ];
    let mx_bottom = 19.0 - 3.0 * 13.0;
    let my_bottom = 23.0 + 3.0 * 11.0 + arm * 17.0;
    let bottom = [-17.0, 11.0, 13.0, 29.0 + arm * 13.0, -mx_bottom, my_bottom];
    for k in 0..6 {
        assert!((p.top[k] - top[k]).abs() < 1e-9, "top {k}: {:?}", p.top);
        assert!(
            (p.bottom[k] - bottom[k]).abs() < 1e-9,
            "bottom {k}: {:?}",
            p.bottom
        );
    }
    // The spandrel (the wall alone; the column is vertical, so its spandrel label would not count) is
    // multi-story: one, at the first story.
    assert_eq!(case.spandrel_forces.len(), 1);
    assert_eq!(case.spandrel_forces[0].story, "L1");
}
