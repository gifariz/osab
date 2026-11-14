//! Frame end releases: closed-form cases (a beam pinned at both ends between fixed joints, a
//! Gerber beam with an inner hinge), the deflected shape along a released member, the end-order swap and
//! the unstable sets refused (docs/phases/PHASE4_PLAN.md).

use osab_solver::{analyze, Model};
use serde_json::{json, Value};

const E: f64 = 200e6;
const I: f64 = 2e-4;
const L: f64 = 6.0;

fn model(frames: Value, supports: Value, nodes: Value, loads: Value) -> Model {
    Model::from_json(
        &json!({
            "materials": [{"id": "S", "E": E, "nu": 0.3}],
            "sections": [{"id": "R", "A": 0.01, "I22": I, "I33": I, "J": 1e-4}],
            "nodes": nodes, "frames": frames, "supports": supports,
            "load_cases": [{"id": "C", "frame_loads": loads}],
        })
        .to_string(),
    )
    .unwrap()
}

fn close(a: f64, b: f64, what: &str) {
    assert!(
        (a - b).abs() <= 1e-9 * b.abs().max(1e-6),
        "{what}: {a:e} vs {b:e}"
    );
}

#[test]
fn beam_pinned_at_both_ends_is_simply_supported() {
    // Fixed joints at both ends, M3 (and M2) released at both ends: w L^2 / 8 at midspan, 5 w L^4 / 384 E I.
    let w = 12.0;
    let m = model(
        json!([{"id": "B", "type": "frame", "nodes": ["1", "2"], "material": "S", "section": "R",
                "releases": {"i": ["M2", "M3"], "j": ["M2", "M3"]}}]),
        json!([{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"}]),
        json!([{"id": "1"}, {"id": "2", "x": L}]),
        json!([{"type": "uniform", "frame": "B", "dir": "Z", "w": -w}]),
    );
    let r = analyze(&m).unwrap();
    let c = r.case(&"C".into()).unwrap();
    let b = &c.frames[0];
    let mid = b
        .stations
        .iter()
        .find(|s| (s.x - L / 2.0).abs() < 1e-9)
        .unwrap();
    close(mid.M3, w * L * L / 8.0, "midspan M3");
    close(
        mid.uz,
        -5.0 * w * L.powi(4) / (384.0 * E * I),
        "midspan deflection",
    );
    for s in [&b.stations[0], b.stations.last().unwrap()] {
        assert!(s.M3.abs() < 1e-9, "end moment {}", s.M3);
    }
    // The fixed joints do not turn; the beam's own end rotation is w L^3 / 24 E I.
    let rot = w * L.powi(3) / (24.0 * E * I);
    close(b.end_displacements[5].abs(), rot, "end i rotation");
    close(b.end_displacements[11].abs(), rot, "end j rotation");
    let reac = c.reaction(&"1".into()).unwrap();
    assert!(reac.my.abs() < 1e-9 && (reac.fz - w * L / 2.0).abs() < 1e-9);
    assert!(c.equilibrium_ok());
}

#[test]
fn gerber_beam_with_an_inner_hinge() {
    // Cantilever 1-2 (fixed at 1) hinged at 2 to a beam 2-3 on a roller at 3; P at 1 m from 3.
    let p = 30.0;
    let m = model(
        json!([{"id": "B1", "type": "frame", "nodes": ["1", "2"], "material": "S", "section": "R",
                "releases": {"j": ["M3"]}},
               {"id": "B2", "type": "frame", "nodes": ["2", "3"], "material": "S", "section": "R"}]),
        json!([{"node": "1", "fix": "fixed"}, {"node": "3", "fix": ["uy", "uz", "rx"]}]),
        json!([{"id": "1"}, {"id": "2", "x": 4}, {"id": "3", "x": 8}]),
        json!([{"type": "point", "frame": "B2", "dir": "Z", "p": -p, "at": 3.0}]),
    );
    let r = analyze(&m).unwrap();
    let c = r.case(&"C".into()).unwrap();
    // B2 simply supported over 4 m: the hinge carries p/4, the roller 3p/4.
    let v = p / 4.0;
    close(c.reaction(&"3".into()).unwrap().fz, 3.0 * p / 4.0, "roller");
    close(c.reaction(&"1".into()).unwrap().my, -v * 4.0, "base moment");
    close(
        -c.displacement(&"2".into()).unwrap().uz,
        v * 64.0 / (3.0 * E * I),
        "hinge deflection",
    );
    let b1 = &c.frames[0];
    assert!(b1.stations.last().unwrap().M3.abs() < 1e-9);
}

#[test]
fn releases_follow_the_end_order() {
    // Drawn from right to left: the release given at "i" (node 2, x = L) stays at x = L.
    let w = 10.0;
    let m = model(
        json!([{"id": "B", "type": "frame", "nodes": ["2", "1"], "material": "S", "section": "R",
                "releases": {"i": ["M3"]}}]),
        json!([{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"}]),
        json!([{"id": "1"}, {"id": "2", "x": L}]),
        json!([{"type": "uniform", "frame": "B", "dir": "Z", "w": -w}]),
    );
    let r = analyze(&m).unwrap();
    let c = r.case(&"C".into()).unwrap();
    // Propped cantilever (fixed at x = 0, pinned at x = L): base moment w L^2 / 8.
    close(
        c.reaction(&"1".into()).unwrap().my.abs(),
        w * L * L / 8.0,
        "fixed-end moment",
    );
    assert!(c.reaction(&"2".into()).unwrap().my.abs() < 1e-9);
}

#[test]
fn unstable_releases_and_trusses_are_refused() {
    let refused = |rel: Value, kind: &str| {
        Model::from_json(
            &json!({
                "materials": [{"id": "S", "E": E, "nu": 0.3}],
                "sections": [{"id": "R", "A": 0.01, "I22": I, "I33": I, "J": 1e-4}],
                "nodes": [{"id": "1"}, {"id": "2", "x": L}],
                "frames": [{"id": "B", "type": kind, "nodes": ["1", "2"], "material": "S", "section": "R", "releases": rel}],
                "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"}],
                "load_cases": [],
            })
            .to_string(),
        )
        .unwrap_err()
        .to_string()
    };
    assert!(
        refused(json!({"i": ["T"], "j": ["T"]}), "frame").contains("T is released at both ends")
    );
    assert!(
        refused(json!({"i": ["P"], "j": ["P"]}), "frame").contains("P is released at both ends")
    );
    assert!(refused(json!({"i": ["V2"], "j": ["V2"]}), "frame").contains("V2 is released"));
    assert!(refused(json!({"i": ["M3", "V2"], "j": ["M3"]}), "frame")
        .contains("M3 is released at both ends and V2"));
    assert!(refused(json!({"i": ["M2"], "j": ["M2", "V3"]}), "frame")
        .contains("M2 is released at both ends and V3"));
    assert!(refused(json!({"i": ["M3"]}), "truss").contains("truss"));
}
