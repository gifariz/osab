//! End releases through the mesh step: a released beam split at a crossing beam's joint keeps its
//! releases at its own two ends only (releases belong to the object's ends), so it gives the same
//! results as the beam drawn as two frames released at the outer ends.

use osab_mesh::api::analyze_value;
use serde_json::{json, Value};

fn grid(beam_a: Value, mesh: bool) -> Value {
    let mut m = json!({
        "materials": [{"id": "S", "E": 200e6, "nu": 0.3}],
        "sections": [{"id": "R", "A": 0.01, "I22": 2e-4, "I33": 2e-4, "J": 1e-4},
                     {"id": "K", "A": 0.02, "I22": 2e-3, "I33": 2e-3, "J": 1e-3}],
        "nodes": [{"id": "1"}, {"id": "2", "x": 6}, {"id": "3", "x": 3}, {"id": "4", "x": 3, "y": -3}, {"id": "5", "x": 3, "y": 3}],
        "frames": beam_a,
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"}, {"node": "4", "fix": "fixed"}, {"node": "5", "fix": "fixed"}],
        "load_cases": [{"id": "C", "frame_loads": [{"type": "uniform", "frame": "B1", "dir": "Z", "w": -10}]}],
    });
    let frames = m["frames"].as_array_mut().unwrap();
    for (id, a, b) in [("B2", "4", "3"), ("B3", "3", "5")] {
        // Stiffer crossing beams (with equal ones the joint happens to settle just enough to cancel the
        // support moment).
        frames.push(
            json!({"id": id, "type": "frame", "nodes": [a, b], "material": "S", "section": "K"}),
        );
    }
    if !mesh {
        m["mesh"] = json!("off");
    }
    m
}

#[test]
fn a_split_frame_keeps_its_releases_at_its_own_ends() {
    let rel = json!({"i": ["M3"], "j": ["M3"]});
    let one = grid(
        json!([{"id": "B1", "type": "frame", "nodes": ["1", "2"], "material": "S", "section": "R", "releases": rel}]),
        true,
    );
    let r1 = analyze_value(one).unwrap();
    let pieces = &r1.mesh.as_ref().unwrap().frames;
    assert_eq!(
        pieces.iter().filter(|p| p.parent.0 == "B1").count(),
        2,
        "B1 is split at joint 3"
    );

    let mut two = grid(
        json!([{"id": "B1", "type": "frame", "nodes": ["1", "3"], "material": "S", "section": "R", "releases": {"i": ["M3"]}},
               {"id": "B4", "type": "frame", "nodes": ["3", "2"], "material": "S", "section": "R", "releases": {"j": ["M3"]}}]),
        false,
    );
    two["load_cases"][0]["frame_loads"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "uniform", "frame": "B4", "dir": "Z", "w": -10}));
    let r2 = analyze_value(two).unwrap();
    let (c1, c2) = (r1.case(&"C".into()).unwrap(), r2.case(&"C".into()).unwrap());
    for n in ["1", "2", "3"] {
        let (a, b) = (
            c1.displacement(&n.into()).unwrap(),
            c2.displacement(&n.into()).unwrap(),
        );
        assert!(
            (a.uz - b.uz).abs() < 1e-12 && (a.ry - b.ry).abs() < 1e-12,
            "joint {n}"
        );
    }
    let r = c1.reaction(&"1".into()).unwrap();
    assert!(r.my.abs() < 1e-9, "released end carries no moment");
    // The beam is continuous over joint 3, so it has a moment there.
    let b1 = c1.frames.iter().find(|f| f.element.0 == "B1").unwrap();
    let mid = b1
        .stations
        .iter()
        .find(|s| (s.x - 3.0).abs() < 1e-9)
        .unwrap();
    assert!(mid.M3.abs() > 1.0);
}
