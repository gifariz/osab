//! Station displacements must equal the node displacements of the same model with every frame
//! member split at its stations: the stiffness method is exact at nodes, so this checks the
//! deflected shape along each member (including member-load and shear-deformation parts).

use std::path::Path;

use osab_solver::model::{FrameKind, FrameLoad, LoadCase, LoadDir, Model, Node, NodeLoad};
use osab_solver::results::CaseResults;
use osab_solver::{analyze, Id};

/// The model (one load case, as analysed) with every frame member split at its stations.
/// Returns the split model and the checks: (new node, original element, x along the member).
fn split(m: &Model, case: &LoadCase, res: &CaseResults) -> (Model, Vec<(Id, Id, f64)>) {
    let mut out = m.clone();
    out.frames.clear();
    let mut node_loads = case.node_loads.clone();
    let mut frame_loads = Vec::new();
    let mut checks = Vec::new();
    for e in &m.frames {
        let er = res.frame(&e.id).unwrap();
        let l = er.length;
        let tol = 1e-9 * l;
        // Trusses, members with end offsets (their stations cover the clear length only), members on
        // line springs (split, the pieces' joints would get spring shares too) and members on rigid arms
        // (an insertion point with the transform: stations follow the member's own axis, split joints
        // would lie on the joint line) stay whole.
        if e.kind == FrameKind::Truss
            || e.end_offsets.is_some()
            || e.line_spring.is_some()
            || e.insertion.as_ref().is_some_and(|i| i.transform)
        {
            out.frames.push(e.clone());
            frame_loads.extend(
                case.frame_loads
                    .iter()
                    .filter(|ml| ml.frame() == &e.id)
                    .cloned(),
            );
            continue;
        }
        let mut xs: Vec<f64> = er
            .stations
            .iter()
            .map(|s| s.x)
            .filter(|&x| x > tol && x < l - tol)
            .collect();
        xs.dedup_by(|a, b| (*a - *b).abs() <= tol);
        let start = m.node(&e.nodes[0]).unwrap().clone();
        let axis = er.axes[0];
        // Chain of sub-elements: (id, from x, to x).
        let mut pieces = Vec::new();
        let mut prev = e.nodes[0].clone();
        let mut prev_x = 0.0;
        for (k, &x) in xs.iter().enumerate() {
            let id = Id(format!("{}~{k}", e.id));
            out.nodes.push(Node {
                id: id.clone(),
                x: start.x + axis[0] * x,
                y: start.y + axis[1] * x,
                z: start.z + axis[2] * x,
                in_mesh: None,
                diaphragm: None,
            });
            checks.push((id.clone(), e.id.clone(), x));
            pieces.push((Id(format!("{}~e{k}", e.id)), [prev, id.clone()], prev_x, x));
            prev = id;
            prev_x = x;
        }
        pieces.push((
            Id(format!("{}~e{}", e.id, xs.len())),
            [prev, e.nodes[1].clone()],
            prev_x,
            l,
        ));
        let last = pieces.len() - 1;
        for (k, (id, nodes, _, _)) in pieces.iter().enumerate() {
            let mut sub = e.clone();
            sub.id = id.clone();
            sub.nodes = nodes.clone();
            // End releases stay at the member's own ends.
            if k > 0 {
                sub.releases.i.clear();
            }
            if k < last {
                sub.releases.j.clear();
            }
            out.frames.push(sub);
        }
        for ml in case.frame_loads.iter().filter(|ml| ml.frame() == &e.id) {
            match ml {
                FrameLoad::Uniform { .. } | FrameLoad::Trapezoidal { .. } => {
                    for (id, _, a, b) in &pieces {
                        frame_loads.extend(ml.cut(id, l, *a, *b, tol));
                    }
                }
                FrameLoad::Point { dir, p, at, .. } => {
                    // Every point load inside the frame sits on a split node.
                    let node = if *at <= tol {
                        e.nodes[0].clone()
                    } else if *at >= l - tol {
                        e.nodes[1].clone()
                    } else {
                        let k = xs.iter().position(|x| (x - at).abs() <= tol).unwrap();
                        Id(format!("{}~{k}", e.id))
                    };
                    let v = match dir {
                        LoadDir::X => [*p, 0.0, 0.0],
                        LoadDir::Y => [0.0, *p, 0.0],
                        LoadDir::Z => [0.0, 0.0, *p],
                        LoadDir::L1 => er.axes[0].map(|a| a * p),
                        LoadDir::L2 => er.axes[1].map(|a| a * p),
                        LoadDir::L3 => er.axes[2].map(|a| a * p),
                    };
                    node_loads.push(NodeLoad {
                        node,
                        fx: v[0],
                        fy: v[1],
                        fz: v[2],
                        ..Default::default()
                    });
                }
            }
        }
    }
    out.load_cases = vec![LoadCase {
        id: case.id.clone(),
        node_loads,
        frame_loads,
        area_loads: case.area_loads.clone(),
        self_weight: case.self_weight,
        ..Default::default()
    }];
    (out, checks)
}

#[test]
fn station_displacements_match_split_members() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/benchmarks");
    let mut dirs: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|d| d.unwrap().path())
        .collect();
    dirs.sort();
    let mut compared = 0;
    for dir in dirs {
        let text = std::fs::read_to_string(dir.join("model.json")).unwrap();
        // Meshed benchmarks (Phase 3b) need the mesh step first; this test is about the solver alone.
        let raw: Model = serde_json::from_str(&text).unwrap();
        if !matches!(raw.mesh, Some(osab_solver::model::MeshSetting::Word(_))) {
            continue;
        }
        // Load patterns written into the cases, so that each case holds its loads (cases with user wind
        // loads, which the analysis turns into joint loads, are left out).
        let (model, _) = Model::from_json(&text)
            .unwrap()
            .with_patterns()
            .unwrap()
            .into_owned()
            .oriented();
        for case in model.load_cases.iter().filter(|c| c.wind_loads.is_empty()) {
            let mut one = model.clone();
            one.load_cases = vec![case.clone()];
            one.load_combinations.clear();
            one.modal = None;
            one.mass_source = None;
            one.spectrum_cases.clear();
            // (Split members would have another geometric stiffness; see the P-Delta test below.)
            one.p_delta = None;
            let res = analyze(&one).unwrap();
            let res = &res.cases[0];
            let (split_model, checks) = split(&one, case, res);
            let split_res = analyze(&split_model).unwrap();
            let split_res = &split_res.cases[0];
            // Largest translation anywhere (a propped cantilever has none at its nodes).
            let scale = split_res
                .displacements
                .iter()
                .flat_map(|d| d.values.as_array()[..3].to_vec())
                .fold(1e-30f64, |m, v| m.max(v.abs()));
            for (node, element, x) in checks {
                let s = res
                    .frame(&element)
                    .unwrap()
                    .stations
                    .iter()
                    .find(|s| (s.x - x).abs() <= 1e-9 * x.max(1.0))
                    .unwrap();
                let d = split_res.displacement(&node).unwrap();
                for (got, want) in [(s.ux, d.ux), (s.uy, d.uy), (s.uz, d.uz)] {
                    assert!(
                        (got - want).abs() <= 1e-9 * scale,
                        "{} case {} element {element} x={x}: station {got:e}, split node {want:e}",
                        dir.display(),
                        case.id
                    );
                    compared += 1;
                }
            }
        }
    }
    assert!(compared > 500, "only {compared} values compared");
}

const MAT_SEC: &str = r#""materials": [{"id": "m", "E": 200e6, "G": 80e6}],
    "sections": [{"id": "s", "A": 0.01, "I22": 5e-5, "I33": 2e-4, "J": 1e-5, "As2": 0.004, "As3": 0.006}],
    "nodes": [{"id": "1", "x": 0}, {"id": "2", "x": 6}],
    "frames": [{"id": "1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"}]"#;

fn station(json_rest: &str, x: f64) -> osab_solver::results::Station {
    let model = Model::from_json(&format!("{{{MAT_SEC}, {json_rest}}}")).unwrap();
    let res = analyze(&model).unwrap();
    *res.cases[0].frames[0]
        .stations
        .iter()
        .find(|s| (s.x - x).abs() < 1e-12)
        .unwrap()
}

#[test]
fn simple_beam_midspan_deflection_one_element() {
    // 5wL^4/384EI bending + wL^2/8GAs shear, with the beam as a single element.
    let (w, l, e, i, g, as2) = (-10.0, 6.0f64, 200e6, 2e-4, 80e6, 0.004);
    let s = station(
        r#""supports": [{"node": "1", "fix": ["ux", "uy", "uz", "rx"]}, {"node": "2", "fix": ["uy", "uz"]}],
        "load_cases": [{"id": "D", "frame_loads": [{"type": "uniform", "frame": "1", "dir": "Z", "w": -10}]}]"#,
        3.0,
    );
    let want = 5.0 * w * l.powi(4) / (384.0 * e * i) + w * l * l / (8.0 * g * as2);
    assert!(
        (s.uz - want).abs() < 1e-12 * want.abs(),
        "{} vs {want}",
        s.uz
    );
    assert!(s.ux.abs() < 1e-15 && s.uy.abs() < 1e-15);
}

#[test]
fn simple_beam_triangular_load_midspan_deflection() {
    // Load rising from 0 to w: 5wL^4/768EI bending + M(L/2)/GAs = wL^2/16GAs shear.
    let (w, l, e, i, g, as2) = (-10.0, 6.0f64, 200e6, 2e-4, 80e6, 0.004);
    let s = station(
        r#""supports": [{"node": "1", "fix": ["ux", "uy", "uz", "rx"]}, {"node": "2", "fix": ["uy", "uz"]}],
        "load_cases": [{"id": "D", "frame_loads": [{"type": "trapezoidal", "frame": "1", "dir": "Z", "w1": 0, "w2": -10}]}]"#,
        3.0,
    );
    let want = 5.0 * w * l.powi(4) / (768.0 * e * i) + w * l * l / (16.0 * g * as2);
    assert!(
        (s.uz - want).abs() < 1e-12 * want.abs(),
        "{} vs {want}",
        s.uz
    );
}

#[test]
fn trapezoidal_loads_match_split_members() {
    // Partial trapezoidal loads in global and local directions on a sloped, reversed frame.
    let text = r#"{"materials": [{"id": "m", "E": 200e6, "G": 80e6}],
        "sections": [{"id": "s", "A": 0.01, "I22": 5e-5, "I33": 2e-4, "J": 1e-5, "As2": 0.004, "As3": 0.006}],
        "nodes": [{"id": "1", "x": 6, "z": 2}, {"id": "2", "y": 1}],
        "frames": [{"id": "1", "type": "frame", "nodes": ["1", "2"], "material": "m", "section": "s"}],
        "supports": [{"node": "1", "fix": "fixed"}, {"node": "2", "fix": ["ux", "uy", "uz"]}],
        "load_cases": [{"id": "D", "frame_loads": [
            {"type": "trapezoidal", "frame": "1", "dir": "Z", "w1": -3, "w2": -8, "from": 1, "to": 4.5},
            {"type": "trapezoidal", "frame": "1", "dir": "2", "w1": 4, "w2": 0, "from": 2},
            {"type": "trapezoidal", "frame": "1", "dir": "1", "w1": 1, "w2": 2}]}]}"#;
    let (model, _) = Model::from_json(text).unwrap().oriented();
    let case = &model.load_cases[0];
    let res = analyze(&model).unwrap();
    let res = &res.cases[0];
    assert!(
        res.equilibrium.as_ref().unwrap().is_ok(),
        "{:?}",
        res.equilibrium
    );
    let (split_model, checks) = split(&model, case, res);
    assert!(!checks.is_empty());
    let split_res = analyze(&split_model).unwrap();
    let split_res = &split_res.cases[0];
    for (node, element, x) in checks {
        let s = res
            .frame(&element)
            .unwrap()
            .stations
            .iter()
            .find(|s| (s.x - x).abs() <= 1e-9 * x.max(1.0))
            .unwrap();
        let d = split_res.displacement(&node).unwrap();
        for (got, want) in [(s.ux, d.ux), (s.uy, d.uy), (s.uz, d.uz)] {
            assert!(
                (got - want).abs() <= 1e-9 * want.abs().max(1e-6),
                "x={x}: {got:e} vs {want:e}"
            );
        }
    }
    // Same reactions as the split model.
    for r in &res.reactions {
        let q = split_res.reaction(&r.node).unwrap();
        for (a, b) in r.values.as_array().iter().zip(q.as_array()) {
            assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
        }
    }
}

#[test]
fn cantilever_deflection_along_member_with_shear() {
    // Tip load P in +Y (local 3 for a beam along X): v(x) = P/EI22 (L x^2/2 - x^3/6) + P x / G As3.
    let (p, l, e, i, g, as3) = (5.0, 6.0f64, 200e6, 5e-5, 80e6, 0.006);
    for x in [1.2f64, 3.0, 4.8] {
        let s = station(
            r#""supports": [{"node": "1", "fix": "fixed"}],
            "load_cases": [{"id": "T", "node_loads": [{"node": "2", "fy": 5}]}]"#,
            x,
        );
        let want = p / (e * i) * (l * x * x / 2.0 - x.powi(3) / 6.0) + p * x / (g * as3);
        assert!(
            (s.uy - want).abs() < 1e-12 * want.abs(),
            "x={x}: {} vs {want}",
            s.uy
        );
    }
}

/// Second reference: OpenSees with every frame member split at the stations
/// (tests/benchmarks/*/opensees_stations.json, made by checks/opensees_run.py --stations).
#[test]
fn station_displacements_match_opensees() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/benchmarks");
    let mut compared = 0;
    for dir in std::fs::read_dir(&root).unwrap().map(|d| d.unwrap().path()) {
        let Ok(text) = std::fs::read_to_string(dir.join("opensees_stations.json")) else {
            continue;
        };
        let reference: serde_json::Value = serde_json::from_str(&text).unwrap();
        let values = reference["values"].as_array().unwrap();
        let model =
            Model::from_json(&std::fs::read_to_string(dir.join("model.json")).unwrap()).unwrap();
        let res = analyze(&model).unwrap();
        let largest = values
            .iter()
            .fold(0.0f64, |m, v| m.max(v["value"].as_f64().unwrap().abs()));
        for v in values {
            let case = res.case(&Id(v["case"].as_str().unwrap().into())).unwrap();
            let er = case
                .frame(&Id(v["element"].as_str().unwrap().into()))
                .unwrap();
            let x = v["x"].as_f64().unwrap();
            let s = er
                .stations
                .iter()
                .find(|s| (s.x - x).abs() <= 1e-9 * er.length)
                .unwrap_or_else(|| panic!("{}: no station at x = {x}", dir.display()));
            let got = match v["dof"].as_str().unwrap() {
                "ux" => s.ux,
                "uy" => s.uy,
                _ => s.uz,
            };
            let want = v["value"].as_f64().unwrap();
            // Same rule as the benchmark comparison: 0.1% plus a small floor for near-zero values.
            let tol = 1e-3 * want.abs() + 1e-5 * largest + 1e-12;
            assert!(
                (got - want).abs() <= tol,
                "{} case {} element {} x={x} {}: OSAB {got:e}, OpenSees {want:e}",
                dir.display(),
                v["case"],
                v["element"],
                v["dof"]
            );
            compared += 1;
        }
    }
    assert!(compared > 800, "only {compared} values compared");
}

/// With P-Delta from loads the deflection along a member follows its moments with their P-Delta part: one
/// element's station against the joint of a finely divided column (its tip within the element's own
/// error, about 1e-3 at this load).
#[test]
fn p_delta_deflection_along_a_member_follows_its_moments() {
    let column = |pieces: usize| -> Model {
        let nodes: Vec<String> = (0..=pieces)
            .map(|k| {
                format!(
                    r#"{{"id": "n{k}", "x": 0, "y": 0, "z": {}}}"#,
                    3.0 * k as f64 / pieces as f64
                )
            })
            .collect();
        let frames: Vec<String> = (0..pieces)
            .map(|k| {
                format!(
                    r#"{{"id": "f{k}", "type": "frame", "nodes": ["n{k}", "n{}"], "material": "c", "section": "s"}}"#,
                    k + 1
                )
            })
            .collect();
        Model::from_json(&format!(
            r#"{{"dimension": "3D", "units": {{"force": "kN", "length": "m"}},
                "materials": [{{"id": "c", "E": 25e6, "nu": 0.2}}],
                "sections": [{{"id": "s", "A": 0.18, "I22": 0.00135, "I33": 0.0054, "J": 0.001}}],
                "nodes": [{}], "frames": [{}], "supports": [{{"node": "n0", "fix": "fixed"}}],
                "load_patterns": [
                    {{"id": "PD", "node_loads": [{{"node": "n{pieces}", "fz": -3000}}]}},
                    {{"id": "LAT", "node_loads": [{{"node": "n{pieces}", "fx": 10, "fy": 5}}]}}],
                "load_cases": [{{"id": "PD", "loads": [{{"pattern": "PD"}}]}}, {{"id": "LAT", "loads": [{{"pattern": "LAT"}}]}}],
                "p_delta": {{"method": "loads", "loads": [{{"pattern": "PD"}}]}}}}"#,
            nodes.join(","),
            frames.join(",")
        ))
        .unwrap()
    };
    let one = analyze(&column(1)).unwrap();
    let one = one.cases.iter().find(|c| c.id.0 == "LAT").unwrap();
    let many = analyze(&column(40)).unwrap();
    let many = many.cases.iter().find(|c| c.id.0 == "LAT").unwrap();
    let f = one.frame(&osab_solver::model::Id("f0".into())).unwrap();
    for s in &f.stations {
        let at = (s.x / 3.0 * 40.0).round() as usize;
        if ((at as f64) * 3.0 / 40.0 - s.x).abs() > 1e-9 {
            continue;
        }
        let d = many
            .displacements
            .iter()
            .find(|d| d.node.0 == format!("n{at}"))
            .unwrap();
        let v = d.values.as_array();
        let tip = many
            .displacements
            .iter()
            .find(|d| d.node.0 == "n40")
            .unwrap()
            .values
            .as_array();
        assert!(
            (s.ux - v[0]).abs() <= 2e-3 * tip[0].abs(),
            "ux at {}: {} against {}",
            s.x,
            s.ux,
            v[0]
        );
        assert!(
            (s.uy - v[1]).abs() <= 2e-3 * tip[1].abs(),
            "uy at {}: {} against {}",
            s.x,
            s.uy,
            v[1]
        );
    }
}
