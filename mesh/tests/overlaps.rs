//! Overlapping floors in the mesh step (docs/SOURCES.md): only one object gets elements
//! where floors overlap (an object inside another, else Stiff over Drop over Slab, else the smaller area;
//! thickness plays no part; docs/elements/STIFF_AREA.md), every
//! object's load acts on its whole area, and an opening cuts every floor it overlaps.

use std::collections::HashMap;

use osab_solver::Model;
use serde_json::{json, Value};

fn model(extra_areas: Value, extra_sections: Value, loads: Value) -> Model {
    let mut nodes = vec![
        json!({"id": "1", "z": 3}),
        json!({"id": "2", "x": 6, "z": 3}),
        json!({"id": "3", "x": 6, "y": 6, "z": 3}),
        json!({"id": "4", "y": 6, "z": 3}),
    ];
    let mut areas = vec![json!({"id": "A", "nodes": ["1", "2", "3", "4"], "section": "BASE"})];
    for (k, a) in extra_areas.as_array().unwrap().iter().enumerate() {
        let corners = a["corners"].as_array().unwrap();
        let ids: Vec<String> = (0..corners.len()).map(|c| format!("o{k}_{c}")).collect();
        for (id, p) in ids.iter().zip(corners) {
            nodes.push(json!({"id": id, "x": p[0], "y": p[1], "z": 3}));
        }
        let mut obj = json!({"id": a["id"], "nodes": ids});
        if let Some(s) = a.get("section") {
            obj["section"] = s.clone();
        } else {
            obj["opening"] = json!(true);
        }
        areas.push(obj);
    }
    let mut sections =
        vec![json!({"id": "BASE", "type": "shell-thin", "material": "C", "thickness": 0.2})];
    sections.extend(extra_sections.as_array().unwrap().iter().cloned());
    serde_json::from_value(json!({
        "materials": [{"id": "C", "E": 25e6, "nu": 0.2}], "sections": [],
        "area_sections": sections, "nodes": nodes, "areas": areas,
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": "fixed"},
                     {"node": "3", "fix": "fixed"}, {"node": "4", "fix": "fixed"}],
        "mesh": {"floor": "rectangular", "floor_size": 1.0},
        "load_cases": [{"id": "Q", "area_loads": loads}]
    }))
    .unwrap()
}

/// Area kept by each object (sum of its elements' areas).
fn kept(m: &Model) -> HashMap<String, f64> {
    let r = osab_mesh::mesh(m).unwrap();
    let at: HashMap<&str, [f64; 2]> = r
        .model
        .nodes
        .iter()
        .map(|n| (n.id.0.as_str(), [n.x, n.y]))
        .collect();
    let mut out = HashMap::new();
    for a in &r.model.areas {
        let p: Vec<[f64; 2]> = a.nodes.iter().map(|n| at[n.0.as_str()]).collect();
        let area: f64 = (0..p.len())
            .map(|k| p[k][0] * p[(k + 1) % p.len()][1] - p[(k + 1) % p.len()][0] * p[k][1])
            .sum::<f64>()
            / 2.0;
        *out.entry(r.parent[&a.id].0.clone()).or_insert(0.0) += area.abs();
    }
    out
}

fn total_load(m: &Model) -> f64 {
    let r = osab_mesh::run(m).unwrap();
    r.cases[0].reactions.iter().map(|x| x.values.fz).sum()
}

#[test]
fn smaller_slab_wins_whatever_its_thickness() {
    for t in [0.1, 0.4] {
        let m = model(
            json!([{"id": "B", "corners": [[2, 2], [4, 2], [4, 4], [2, 4]], "section": "S"}]),
            json!([{"id": "S", "type": "shell-thin", "material": "C", "thickness": t}]),
            json!([{"area": "A", "dir": "Z", "q": -5}, {"area": "B", "dir": "Z", "q": -3}]),
        );
        let k = kept(&m);
        assert!(
            (k["A"] - 32.0).abs() < 1e-9 && (k["B"] - 4.0).abs() < 1e-9,
            "{k:?}"
        );
        // Loads add up: 5 * 36 + 3 * 4.
        assert!((total_load(&m) - 192.0).abs() < 1e-8);
    }
}

#[test]
fn drop_wins_over_a_smaller_slab_and_openings_cut_both() {
    let m = model(
        json!([{"id": "B", "corners": [[1, 1], [5, 1], [5, 5], [1, 5]], "section": "D"},
               {"id": "C", "corners": [[0, 2], [6, 2], [6, 3], [0, 3]], "section": "S"},
               {"id": "O", "corners": [[2, 4], [3, 4], [3, 5], [2, 5]]}]),
        json!([{"id": "D", "type": "shell-thin", "material": "C", "thickness": 0.3, "slab_type": "drop"},
               {"id": "S", "type": "shell-thin", "material": "C", "thickness": 0.3}]),
        json!([{"area": "A", "dir": "Z", "q": -1}]),
    );
    let k = kept(&m);
    // The drop (16 m^2) keeps its square but the opening (1 m^2); the band (6 m^2) keeps what is outside
    // the drop (2 m^2); the base keeps the rest.
    assert!((k["B"] - 15.0).abs() < 1e-9, "{k:?}");
    assert!((k["C"] - 2.0).abs() < 1e-9, "{k:?}");
    assert!((k["A"] - (36.0 - 16.0 - 2.0)).abs() < 1e-9, "{k:?}");
    // The base's load acts everywhere but in the opening.
    assert!((total_load(&m) - 35.0).abs() < 1e-8);
}

#[test]
fn stiff_wins_partial_overlaps_but_an_object_inside_wins_over_it() {
    let secs = json!([
        {"id": "K", "type": "shell-thin", "material": "C", "thickness": 0.2, "slab_type": "stiff"},
        {"id": "D", "type": "shell-thin", "material": "C", "thickness": 0.3, "slab_type": "drop"},
        {"id": "S", "type": "shell-thin", "material": "C", "thickness": 0.25}]);
    let q = json!([{"area": "A", "dir": "Z", "q": -5}]);
    // Stiff over a Drop of the same size and over a smaller
    // Slab that it only partly covers.
    let m = model(
        json!([{"id": "K1", "corners": [[1, 1], [4, 1], [4, 4], [1, 4]], "section": "K"},
               {"id": "D1", "corners": [[2, 2], [5, 2], [5, 5], [2, 5]], "section": "D"}]),
        secs.clone(),
        q.clone(),
    );
    let k = kept(&m);
    assert!(
        (k["K1"] - 9.0).abs() < 1e-9 && (k["D1"] - 5.0).abs() < 1e-9,
        "{k:?}"
    );
    let m = model(
        json!([{"id": "K1", "corners": [[4, -2], [12, -2], [12, 8], [4, 8]], "section": "K"}]),
        secs.clone(),
        q.clone(),
    );
    let k = kept(&m);
    assert!(
        (k["K1"] - 80.0).abs() < 1e-9 && (k["A"] - 24.0).abs() < 1e-9,
        "{k:?}"
    );
    // o_slab_in_stiff, o_drop_in_stiff, o_slab_in_drop: the inner object wins (also on the edge).
    for (outer, inner, corners) in [
        ("K", "S", json!([[2, 2], [4, 2], [4, 4], [2, 4]])),
        ("K", "D", json!([[2, 2], [4, 2], [4, 4], [2, 4]])),
        ("D", "S", json!([[2, 2], [4, 2], [4, 4], [2, 4]])),
        ("K", "S", json!([[1, 2], [3, 2], [3, 4], [1, 4]])),
    ] {
        let m = model(
            json!([{"id": "O1", "corners": [[1, 1], [5, 1], [5, 5], [1, 5]], "section": outer},
                   {"id": "I1", "corners": corners, "section": inner}]),
            secs.clone(),
            q.clone(),
        );
        let k = kept(&m);
        assert!(
            (k["I1"] - 4.0).abs() < 1e-9 && (k["O1"] - 12.0).abs() < 1e-9,
            "{outer} {inner}: {k:?}"
        );
    }
}
