//! Two-node truss (axial force only).

use super::Props;

/// Axial stiffness; with a P-Delta axial force `p.pd` (tension positive) also the string stiffness pd / l
/// sideways (u2, u3): the geometric stiffness of a bar whose ends are free to turn, as a frame released at
/// both ends gets it (docs/phases/P_DELTA_PLAN.md).
pub fn local_stiffness(p: &Props) -> [[f64; 12]; 12] {
    let mut k = [[0.0; 12]; 12];
    for (d, v) in [(0, p.e * p.a / p.l), (1, p.pd / p.l), (2, p.pd / p.l)] {
        k[d][d] = v;
        k[d + 6][d + 6] = v;
        k[d][d + 6] = -v;
        k[d + 6][d] = -v;
    }
    k
}
