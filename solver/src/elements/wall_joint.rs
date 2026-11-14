//! Frame-to-wall joint term (B25's open problem, found in the Phase 3b probe:
//! `checks/wall_joint_probe.py`).
//!
//! Where a frame meets a 4-node wall element at one of its corners J, a rotational spring is added
//! between J's rotation about the wall's normal and the rotation of the element's edge at J that is
//! nearest the horizontal (local axis 1 before any user angle). It is the stiffness of a beam
//! embedded along that edge, fixed at J and pinned at the edge's other end K (a propped beam 3 E I / L,
//! I = t d^3 / 12), so the energy is k/2 (theta_J - theta_edge)^2 with k = E t d^3 / (4 L), L = |JK| and
//! d = half the mean length of the element's two other edges (J's and K's). When that edge is the
//! element's closing edge (from its last corner back to its first, in corner order: the first
//! lowest corner first), the edge's own length is used in place of K's other edge. One spring per
//! joint and element, the same for any number, direction or stiffness of frames, for Shell-Thin and
//! Membrane walls alike; triangles and floors get none. Exact to round-off on 45 reference cases.
//! With property modifiers E is scaled by the smallest of the wall's f11, f22 and f12 (bending
//! modifiers do nothing; Phase 4, probe group M of `checks/wall_joint_probe.py`).

use crate::axes::{self, Vec3};

/// Springs of one wall quad: for every framed corner J, (J, K, 12 x 12 row-major stiffness over the
/// six DOFs of J then of K, global axes). `p`: corners as analysed; `framed[k]`: a frame meets corner k.
pub fn springs(p: &[Vec3; 4], e: f64, t: f64, framed: [bool; 4]) -> Vec<(usize, usize, Vec<f64>)> {
    let ax = axes::area_axes(p, 0.0);
    let (e1, n) = (ax[0], ax[2]);
    let len = |a: Vec3, b: Vec3| {
        let d = axes::sub(b, a);
        axes::dot(d, d).sqrt()
    };
    let mut out = Vec::new();
    for j in (0..4).filter(|&j| framed[j]) {
        let (next, prev) = ((j + 1) % 4, (j + 3) % 4);
        let along = |q: usize| {
            let d = axes::sub(p[q], p[j]);
            (axes::dot(d, e1) / axes::dot(d, d).sqrt()).abs()
        };
        let (k, m) = if along(next) >= along(prev) {
            (next, prev)
        } else {
            (prev, next)
        };
        // K's other edge, away from J. On the closing edge (between the last and the first corner,
        // in corner order) that edge itself is taken instead (an indexing quirk, measured).
        let nk = if k == next { (k + 1) % 4 } else { (k + 3) % 4 };
        let l = len(p[j], p[k]);
        let closing = (j == 3 && k == 0) || (j == 0 && k == 3);
        let far = if closing { l } else { len(p[k], p[nk]) };
        let d = (len(p[j], p[m]) + far) / 4.0;
        let stiff = e * t * d.powi(3) / (4.0 * l);
        let dir = axes::scale(axes::sub(p[k], p[j]), 1.0 / l);
        let perp = axes::cross(n, dir);
        // theta_J - theta_edge over [u_J, r_J, u_K, r_K].
        let mut v = [0.0; 12];
        for c in 0..3 {
            v[c] = perp[c] / l;
            v[3 + c] = n[c];
            v[6 + c] = -perp[c] / l;
        }
        let mut kk = vec![0.0; 144];
        for r in 0..12 {
            for c in 0..12 {
                kk[r * 12 + c] = stiff * v[r] * v[c];
            }
        }
        out.push((j, k, kk));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// B25's smallest case (the hand-off's thin_x): a 3 x 3 m wall, t = 0.25, E = 25e6, a beam at the top
    /// right corner: the reference adds 195312.5 (uz3 - uz4 + 3 ry3)^2 / 2.
    #[test]
    fn thin_x_reference() {
        let p = [
            [0.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [3.0, 0.0, 3.0],
            [0.0, 0.0, 3.0],
        ];
        let s = springs(&p, 25e6, 0.25, [false, false, true, false]);
        assert_eq!(s.len(), 1);
        let (j, k, kk) = &s[0];
        assert_eq!((*j, *k), (2, 3));
        // Rows/cols: uz3 = 2, ry3 = 4, uz4 = 8.
        let at = |r: usize, c: usize| kk[r * 12 + c];
        assert!((at(2, 2) - 195312.5).abs() < 1e-6);
        assert!((at(2, 4) - 195312.5 * 3.0).abs() < 1e-6);
        assert!((at(2, 8) + 195312.5).abs() < 1e-6);
        assert!((at(4, 4) - 195312.5 * 9.0).abs() < 1e-5);
        // A rigid rotation of the wall costs nothing.
        let mut u = [0.0; 12];
        u[4] = 1.0;
        u[10] = 1.0;
        u[2] = -3.0; // joint 3 at x = 3: uz = -theta x
        u[8] = 0.0; // joint 4 at x = 0
        let f: Vec<f64> = (0..12)
            .map(|r| (0..12).map(|c| at(r, c) * u[c]).sum())
            .collect();
        assert!(f.iter().all(|v| v.abs() < 1e-6));
    }
}
