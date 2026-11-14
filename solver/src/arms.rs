//! Rigid arms from joints to element ends: an insertion point that changes the stiffness puts a frame's
//! ends at its section's centroid, or a shell's corners on its mid-plane, away from the joints. Each end
//! at `e` from its joint moves with the joint as a rigid body: u_end = u_joint + θ × e, θ_end = θ.
//! So an element's end displacements are T u, its forces at the joints Tᵀ f (the moment gains e × f) and
//! its stiffness at the joints Tᵀ K T. Vectors hold 6 values per node (UX..RZ, FX..MZ); `arms` has one
//! offset per node of the element.

use crate::axes;

/// The transformation T (row-major, 6n x 6n) for these arms.
fn matrix(arms: &[[f64; 3]]) -> Vec<f64> {
    let n = 6 * arms.len();
    let mut t = vec![0.0; n * n];
    for (k, e) in arms.iter().enumerate() {
        let o = 6 * k;
        for d in 0..6 {
            t[(o + d) * n + o + d] = 1.0;
        }
        // θ × e = -[e]x θ: rows UX..UZ, columns RX..RZ.
        let c = [[0.0, e[2], -e[1]], [-e[2], 0.0, e[0]], [e[1], -e[0], 0.0]];
        for r in 0..3 {
            for s in 0..3 {
                t[(o + r) * n + o + 3 + s] = c[r][s];
            }
        }
    }
    t
}

/// Joint displacements to element-end displacements (T u).
pub fn to_ends(u: &mut [f64], arms: &[[f64; 3]]) {
    for (k, e) in arms.iter().enumerate() {
        let o = 6 * k;
        let d = axes::cross([u[o + 3], u[o + 4], u[o + 5]], *e);
        for c in 0..3 {
            u[o + c] += d[c];
        }
    }
}

/// Element-end forces to the forces they put on the joints (Tᵀ f).
pub fn to_joints(f: &mut [f64], arms: &[[f64; 3]]) {
    for (k, e) in arms.iter().enumerate() {
        let o = 6 * k;
        let m = axes::cross(*e, [f[o], f[o + 1], f[o + 2]]);
        for c in 0..3 {
            f[o + 3 + c] += m[c];
        }
    }
}

/// A stiffness at the element ends (row-major, 6n x 6n) as a stiffness at the joints (Tᵀ K T).
pub fn stiffness(k: &[f64], arms: &[[f64; 3]]) -> Vec<f64> {
    let n = 6 * arms.len();
    let t = matrix(arms);
    let mut kt = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            kt[i * n + j] = (0..n).map(|m| k[i * n + m] * t[m * n + j]).sum();
        }
    }
    let mut out = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            out[i * n + j] = (0..n).map(|m| t[m * n + i] * kt[m * n + j]).sum();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_forms_agree() {
        // Tᵀ K T u = Tᵀ (K (T u)) for any K and u.
        let arms = [[0.1, -0.3, 0.2], [0.0, 0.25, -0.4]];
        let k: Vec<f64> = (0..144)
            .map(|i| ((i * 37 % 101) as f64) / 10.0 - 5.0)
            .collect();
        let u: Vec<f64> = (0..12).map(|i| (i as f64 * 0.7).sin()).collect();
        let kj = stiffness(&k, &arms);
        let direct: Vec<f64> = (0..12)
            .map(|i| (0..12).map(|j| kj[i * 12 + j] * u[j]).sum())
            .collect();
        let mut ue = u.clone();
        to_ends(&mut ue, &arms);
        let mut f: Vec<f64> = (0..12)
            .map(|i| (0..12).map(|j| k[i * 12 + j] * ue[j]).sum())
            .collect();
        to_joints(&mut f, &arms);
        for (a, b) in direct.iter().zip(&f) {
            assert!((a - b).abs() < 1e-12 * (1.0 + a.abs()), "{a} vs {b}");
        }
    }

    #[test]
    fn a_rotation_moves_the_arm_end() {
        // A joint turning 0.01 about Z moves an end 1 m along X by 0.01 along Y.
        let mut u = [0.0, 0.0, 0.0, 0.0, 0.0, 0.01];
        to_ends(&mut u, &[[1.0, 0.0, 0.0]]);
        assert!((u[1] - 0.01).abs() < 1e-15 && u[0] == 0.0);
        // A force along Y at that end is a moment about Z at the joint.
        let mut f = [0.0, 2.0, 0.0, 0.0, 0.0, 0.0];
        to_joints(&mut f, &[[1.0, 0.0, 0.0]]);
        assert!((f[5] - 2.0).abs() < 1e-15);
    }
}
