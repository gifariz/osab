//! Shells against closed-form solutions that an element only reaches as the mesh is refined: the same
//! problem at three meshes (n, 2n, 4n) is extrapolated to zero element size (Richardson). The extrapolated
//! value must be within 0.1% of the closed form, and the observed convergence rate close to 2, so the
//! extrapolation can be trusted (docs/phases/PHASE3A_PLAN.md, "What match means", kind 3).

use osab_solver::{analyze, Model};
use serde_json::{json, Value};

const E: f64 = 25e6;
const NU: f64 = 0.2;

/// Richardson extrapolation of values on meshes h, h/2, h/4: (extrapolated value, observed order).
fn richardson(f: [f64; 3]) -> (f64, f64) {
    let p = ((f[0] - f[1]) / (f[1] - f[2])).log2();
    (f[2] + (f[2] - f[1]) / (2f64.powf(p) - 1.0), p)
}

fn check(name: &str, f: [f64; 3], exact: f64) {
    let (ext, p) = richardson(f);
    let err = (ext - exact).abs() / exact.abs();
    println!("{name}: meshes {f:?}, order {p:.3}, extrapolated {ext:.6e}, exact {exact:.6e}, error {err:.2e}");
    assert!((1.7..=2.3).contains(&p), "{name}: observed order {p}");
    assert!(
        err < 1e-3,
        "{name}: extrapolated {ext} vs exact {exact} ({err:.2e})"
    );
}

/// Simply supported square Kirchhoff plate under uniform load: Navier series at the centre.
/// Returns (deflection, M11) with the load q downwards, deflection positive downwards, sagging M positive.
fn navier_centre(a: f64, t: f64, q: f64) -> (f64, f64) {
    let d = E * t.powi(3) / (12.0 * (1.0 - NU * NU));
    let pi = std::f64::consts::PI;
    let (mut w, mut m) = (0.0, 0.0);
    for i in (1..400).step_by(2) {
        for j in (1..400).step_by(2) {
            let (fi, fj) = (i as f64, j as f64);
            let s = if ((i + j) / 2 - 1) % 2 == 0 {
                1.0
            } else {
                -1.0
            }; // sin(i pi/2) sin(j pi/2)
            let k = (fi * fi + fj * fj) / (a * a);
            w += s / (fi * fj * k * k);
            m += s * (fi * fi + NU * fj * fj) / (a * a) / (fi * fj * k * k);
        }
    }
    let c = 16.0 * q / pi.powi(6);
    (c * w / d, c * m * pi * pi)
}

fn plate_model(n: usize, a: f64, t: f64, q: f64) -> Value {
    let id = |i: usize, j: usize| (j * (n + 1) + i + 1).to_string();
    let mut nodes = Vec::new();
    let mut supports = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            nodes.push(
                json!({"id": id(i, j), "x": a * i as f64 / n as f64, "y": a * j as f64 / n as f64}),
            );
            let edge = i == 0 || j == 0 || i == n || j == n;
            if (i, j) == (0, 0) {
                supports.push(json!({"node": id(i, j), "fix": ["ux", "uy", "uz"]}));
            } else if (i, j) == (n, 0) {
                supports.push(json!({"node": id(i, j), "fix": ["uy", "uz"]}));
            } else if edge {
                supports.push(json!({"node": id(i, j), "fix": ["uz"]}));
            }
        }
    }
    let mut areas = Vec::new();
    let mut loads = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let aid = format!("F{}", j * n + i + 1);
            areas.push(json!({"id": aid, "nodes": [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)], "section": "S"}));
            loads.push(json!({"area": aid, "dir": "Z", "q": -q}));
        }
    }
    json!({
        "materials": [{"id": "c", "E": E, "nu": NU}], "sections": [], "frames": [],
        "area_sections": [{"id": "S", "type": "shell-thin", "material": "c", "thickness": t}],
        "nodes": nodes, "areas": areas, "supports": supports,
        "load_cases": [{"id": "Q", "area_loads": loads}]
    })
}

#[test]
fn simply_supported_plate_converges_to_navier() {
    let (a, t, q) = (4.0, 0.2, 10.0);
    let (w_exact, m_exact) = navier_centre(a, t, q);
    let mut w = [0.0; 3];
    let mut m = [0.0; 3];
    for (k, n) in [8usize, 16, 32].into_iter().enumerate() {
        let res =
            analyze(&Model::from_json(&plate_model(n, a, t, q).to_string()).unwrap()).unwrap();
        let c = &res.cases[0];
        let centre = ((n / 2) * (n + 1) + n / 2 + 1).to_string();
        w[k] = -c.displacement(&centre.as_str().into()).unwrap().uz;
        // M11 at the centre joint, averaged over the four elements that meet there (as contours are drawn).
        let vals: Vec<f64> = c
            .shells
            .iter()
            .flat_map(|s| s.corners.iter())
            .filter(|cr| cr.node.0 == centre)
            .map(|cr| cr.M11)
            .collect();
        assert_eq!(vals.len(), 4);
        m[k] = vals.iter().sum::<f64>() / 4.0;
    }
    check("plate centre deflection", w, w_exact);
    check("plate centre M11", m, m_exact);
}

/// A wall in pure in-plane bending (B22's walls): plane-stress closed form, deflection of the neutral axis
/// at the loaded end = M L^2 / (2 E I) (curvature M / EI). The left end is held as in the exact solution:
/// the whole edge stays vertical (ux = 0) and its mid-height joint is held in uz.
fn wall_model(nx: usize, nz: usize, l: f64, h: f64, t: f64, m: f64) -> (Value, String) {
    let id = |i: usize, j: usize| (j * (nx + 1) + i + 1).to_string();
    let mut nodes = Vec::new();
    let mut supports = Vec::new();
    for j in 0..=nz {
        for i in 0..=nx {
            nodes.push(json!({"id": id(i, j), "x": l * i as f64 / nx as f64, "z": h * j as f64 / nz as f64}));
            let fix = if (i, j) == (0, nz / 2) {
                json!(["ux", "uy", "uz", "rx", "rz"])
            } else if i == 0 {
                json!(["ux", "uy", "rx", "rz"])
            } else {
                json!(["uy", "rx", "rz"])
            };
            supports.push(json!({"node": id(i, j), "fix": fix}));
        }
    }
    let mut areas = Vec::new();
    for j in 0..nz {
        for i in 0..nx {
            areas.push(json!({"id": format!("W{}", j * nx + i + 1),
                "nodes": [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)], "section": "S"}));
        }
    }
    // Linear edge load p(z') = 12 M z' / H^3 per height, as consistent joint forces on each end.
    let p = |j: usize| 12.0 * m * (h * j as f64 / nz as f64 - h / 2.0) / h.powi(3);
    let seg = h / nz as f64;
    let mut f = vec![0.0; nz + 1];
    for j in 0..nz {
        f[j] += seg * (2.0 * p(j) + p(j + 1)) / 6.0;
        f[j + 1] += seg * (p(j) + 2.0 * p(j + 1)) / 6.0;
    }
    let mut loads = Vec::new();
    for (j, fj) in f.iter().enumerate() {
        loads.push(json!({"node": id(nx, j), "fx": fj}));
        loads.push(json!({"node": id(0, j), "fx": -fj}));
    }
    let model = json!({
        "materials": [{"id": "c", "E": E, "nu": NU}], "sections": [], "frames": [],
        "area_sections": [{"id": "S", "type": "membrane", "material": "c", "thickness": t}],
        "nodes": nodes, "areas": areas, "supports": supports,
        "load_cases": [{"id": "M", "node_loads": loads}]
    });
    (model, id(nx, nz / 2))
}

#[test]
fn wall_in_pure_bending_matches_plane_stress() {
    let (l, h, t, m) = (6.0f64, 1.2f64, 0.3f64, 30.0f64);
    let exact = -m / (E * t * h.powi(3) / 12.0) * l * l / 2.0;
    let mut u = [0.0; 3];
    for (k, (nx, nz)) in [(6usize, 2usize), (12, 4), (24, 8)].into_iter().enumerate() {
        let (model, tip) = wall_model(nx, nz, l, h, t, m);
        let res = analyze(&Model::from_json(&model.to_string()).unwrap()).unwrap();
        u[k] = res.cases[0].displacement(&tip.as_str().into()).unwrap().uz;
    }
    if u.iter().all(|v| ((v - exact) / exact).abs() < 1e-6) {
        println!("wall tip deflection exact on every mesh: {u:?}");
    } else {
        check("wall tip deflection", u, exact);
    }
}
