//! The plane grid (step 9, B37): a flat floor's bending does not depend on its in-plane part, so the
//! plane-grid run (UZ, RX, RY only) of B37 equals the 3D run of the same model, value by value.

use osab_solver::model::Dimension;
use osab_solver::Model;

fn b37() -> Model {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/benchmarks/b37_plane_grid/model.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn close(a: f64, b: f64, scale: f64, what: &str) {
    assert!(
        (a - b).abs() <= 1e-9 * scale + 1e-14,
        "{what}: grid {a} vs 3D {b}"
    );
}

#[test]
fn b37_plane_grid_equals_its_3d_run() {
    let grid = b37();
    assert_eq!(grid.dimension, Dimension::Grid);
    let mut full = grid.clone();
    full.dimension = Dimension::ThreeD;
    let (g, f) = (
        osab_mesh::run(&grid).unwrap(),
        osab_mesh::run(&full).unwrap(),
    );
    assert_eq!(g.cases.len(), 3);
    for (cg, cf) in g.cases.iter().zip(&f.cases) {
        let id = &cg.id.0;
        // Displacements: the same everywhere; the grid holds UX, UY and RZ at zero.
        let largest = cf
            .displacements
            .iter()
            .flat_map(|d| d.values.as_array())
            .fold(0.0f64, |m, v| m.max(v.abs()));
        assert_eq!(cg.displacements.len(), cf.displacements.len());
        for (dg, df) in cg.displacements.iter().zip(&cf.displacements) {
            assert_eq!(dg.node, df.node);
            let (ug, uf) = (dg.values.as_array(), df.values.as_array());
            for k in 0..6 {
                close(
                    ug[k],
                    uf[k],
                    largest,
                    &format!("{id} {} dof {k}", dg.node.0),
                );
            }
            assert_eq!([ug[0], ug[1], ug[5]], [0.0, 0.0, 0.0]);
        }
        // Reactions at the supports (fz, mx, my; the grid's held DOFs carry nothing).
        let rmax = cf
            .reactions
            .iter()
            .map(|r| r.values.fz.abs())
            .fold(1e-9, f64::max);
        for (rg, rf) in cg.reactions.iter().zip(&cf.reactions) {
            assert_eq!(rg.node, rf.node);
            close(
                rg.values.fz,
                rf.values.fz,
                rmax,
                &format!("{id} {} fz", rg.node.0),
            );
            close(
                rg.values.mx,
                rf.values.mx,
                rmax,
                &format!("{id} {} mx", rg.node.0),
            );
            close(
                rg.values.my,
                rf.values.my,
                rmax,
                &format!("{id} {} my", rg.node.0),
            );
        }
        // Shell forces and moments at every element corner.
        let smax = cf
            .shells
            .iter()
            .flat_map(|s| s.corners.iter().flat_map(|c| c.as_array()))
            .fold(1e-9, |m: f64, v| m.max(v.abs()));
        assert_eq!(cg.shells.len(), 64);
        for (sg, sf) in cg.shells.iter().zip(&cf.shells) {
            assert_eq!(sg.element, sf.element);
            for (kg, kf) in sg.corners.iter().zip(&sf.corners) {
                for (vg, vf) in kg.as_array().iter().zip(kf.as_array()) {
                    close(
                        *vg,
                        vf,
                        smax,
                        &format!("{id} {} {}", sg.element.0, kg.node.0),
                    );
                }
            }
        }
        // Frame forces along every beam.
        let fmax = cf
            .frames
            .iter()
            .flat_map(|e| e.stations.iter().flat_map(|s| [s.V2, s.M3, s.T]))
            .fold(1e-9, |m: f64, v| m.max(v.abs()));
        for (eg, ef) in cg.frames.iter().zip(&cf.frames) {
            assert_eq!(eg.element, ef.element);
            assert_eq!(eg.stations.len(), ef.stations.len());
            for (sg, sf) in eg.stations.iter().zip(&ef.stations) {
                for (vg, vf) in [
                    (sg.P, sf.P),
                    (sg.V2, sf.V2),
                    (sg.V3, sf.V3),
                    (sg.T, sf.T),
                    (sg.M2, sf.M2),
                    (sg.M3, sf.M3),
                ] {
                    close(vg, vf, fmax, &format!("{id} {} at {}", eg.element.0, sg.x));
                }
            }
        }
    }
}
