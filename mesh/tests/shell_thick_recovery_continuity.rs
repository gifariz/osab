//! Mesh-coordinate roundoff must not switch Shell-Thick moment recovery abruptly.
use osab_solver::{Model, Results};
use serde_json::Value;

fn b21(offset: f64, axis: &str) -> Results {
    let mut input: Value = serde_json::from_str(include_str!(
        "../../tests/benchmarks/b21_ss_plate_thick/model.json"
    ))
    .unwrap();
    let node = input["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "41")
        .unwrap();
    node[axis] = (node[axis].as_f64().unwrap() + offset).into();
    osab_mesh::run(&Model::from_json(&input.to_string()).unwrap()).unwrap()
}

#[test]
fn b21_moments_are_continuous_at_rectangular_geometry() {
    let baseline = b21(0.0, "x");
    for axis in ["x", "y"] {
        for delta in [-1e-8, -1e-10, -1e-12, 1e-12, 1e-10, 1e-8] {
            let moved = b21(delta, axis);
            let mut worst: f64 = 0.0;
            let mut f37_twist: f64 = 0.0;
            for (reference, actual) in baseline.cases.iter().zip(&moved.cases) {
                assert_eq!(reference.id, actual.id);
                let peak = reference
                    .shells
                    .iter()
                    .flat_map(|s| &s.corners)
                    .flat_map(|c| c.as_array()[3..6].to_vec())
                    .fold(0.0_f64, |p, v| p.max(v.abs()));
                assert_eq!(reference.shells.len(), actual.shells.len());
                for (r, a) in reference.shells.iter().zip(&actual.shells) {
                    assert_eq!(r.element, a.element);
                    for (rc, ac) in r.corners.iter().zip(&a.corners) {
                        assert_eq!(rc.node, ac.node);
                        let rv = rc.as_array();
                        let av = ac.as_array();
                        for k in 3..6 {
                            worst = worst.max((rv[k] - av[k]).abs() / peak);
                        }
                        if r.area.to_string() == "F37" {
                            f37_twist =
                                f37_twist.max((rv[5] - av[5]).abs() / rv[5].abs().max(1e-5 * peak));
                        }
                    }
                }
            }
            // Physical changes must shrink with the perturbation, down to numerical
            // solve noise. The former hard switch fails this bound by orders of magnitude.
            println!("node 41 {axis} {delta:+.1e}: moment change / case peak = {worst:.6e}; F37 M12 relative = {f37_twist:.6e}");
            assert!(
                worst < 10.0 * delta.abs() / 0.5 + 5e-9,
                "node 41 {axis} {delta:+.1e}: discontinuous moment change {worst:.6e}"
            );
            assert!(
                f37_twist < 50.0 * delta.abs() / 0.5 + 1e-7,
                "F37 M12 jump: {f37_twist:.6e}"
            );
        }
    }
}
