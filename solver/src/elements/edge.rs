//! Edge constraint: a joint lying on an edge of a shell element (between its two corners) tied to that
//! edge (rule found in the Phase 3b probe, docs/phases/PHASE3B_PLAN.md, *Probe findings, 2a*).
//!
//! The tie is three hidden line elements per tied joint M on edge a-b: Euler beams a-M and M-b and,
//! subtracted, a beam a-b. The result costs no energy when M moves the way a beam along the edge would
//! (cubic Hermite interpolation of the transverse displacements and rotations, linear along the edge and
//! in twist) and stiffly resists any other motion of M ("flexible interpolation", docs/SOURCES.md).
//! The beams take their section from the element whose edge it is, with W = element area / edge length:
//! A = t W, I = t W^3 / 12 in the shell's plane, I = W t^3 / 12 out of it, torsion E * 16.65 W t^3, all
//! with the element's E (no Poisson's ratio, no shear deformation). A Membrane element's beams keep only
//! 1e-6 of that out-of-plane bending and about 1e-8 of that torsion (probe m_L, a Membrane floor with an
//! inside corner on columns: the bending factor is sharp, the torsion factor only roughly known).
//! On walls the frame-to-wall joint term is added too (`wall_joint`).

use crate::axes::{self, Axes, Vec3};
use crate::dof::NDF;
use crate::elements::{self, Props};
use crate::model::FrameKind;

/// Torsion constant of the constraint beams, as a multiple of W t^3 (measured; with E, not G).
pub const TORSION: f64 = 16.65;

/// Parts of the out-of-plane bending and of the torsion that a Membrane element's constraint beams keep.
pub const MEMBRANE_BENDING: f64 = 1e-6;
pub const MEMBRANE_TORSION: f64 = 1e-8;

/// Stiffness (18 x 18, row-major) that ties joint `pm` to the edge `pa`-`pb` of a shell element with
/// unit normal `normal`, Young's modulus `e`, thickness `t` and width `w` (element area / edge length);
/// `plate` = false for a Membrane element (out-of-plane parts nearly zero).
/// DOF order: joint a, joint b, joint m (6 each, global axes).
#[allow(clippy::too_many_arguments)]
pub fn stiffness(
    pa: Vec3,
    pb: Vec3,
    pm: Vec3,
    normal: Vec3,
    e: f64,
    t: f64,
    w: f64,
    plate: bool,
) -> Vec<f64> {
    let (bend, twist) = if plate {
        (1.0, 1.0)
    } else {
        (MEMBRANE_BENDING, MEMBRANE_TORSION)
    };
    let x = axes::normalize(axes::sub(pb, pa));
    let z = axes::normalize(axes::sub(normal, axes::scale(x, axes::dot(normal, x))));
    let y = axes::cross(z, x);
    let ax: Axes = [x, y, z];
    let beam = |p: Vec3, q: Vec3| {
        let d = axes::sub(q, p);
        let props = Props {
            kind: FrameKind::Frame,
            e,
            g: e,
            a: t * w,
            i22: bend * w * t.powi(3) / 12.0,
            i33: t * w.powi(3) / 12.0,
            j: twist * TORSION * w * t.powi(3),
            as2: 0.0,
            as3: 0.0,
            l: axes::dot(d, d).sqrt(),
            ri: 0.0,
            rj: 0.0,
            ri3: 0.0,
            rj3: 0.0,
            oi: 0.0,
            oj: 0.0,
            pd: 0.0,
        };
        axes::mat12_to_global(&ax, &elements::local_stiffness(&props))
    };
    let n = 3 * NDF;
    let mut k = vec![0.0; n * n];
    // Joints 0 = a, 1 = b, 2 = m.
    let mut add = |kb: [[f64; 12]; 12], i: usize, j: usize, s: f64| {
        let at = |r: usize| {
            if r < NDF {
                i * NDF + r
            } else {
                j * NDF + r - NDF
            }
        };
        for r in 0..12 {
            for c in 0..12 {
                k[at(r) * n + at(c)] += s * kb[r][c];
            }
        }
    };
    add(beam(pa, pm), 0, 2, 1.0);
    add(beam(pm, pb), 2, 1, 1.0);
    add(beam(pa, pb), 0, 1, -1.0);
    k
}

/// Width of the constraint beams: area of the polygon `p` (flat) divided by the length of edge `a`-`b`.
pub fn width(p: &[Vec3], pa: Vec3, pb: Vec3) -> f64 {
    let mut s = [0.0; 3];
    for k in 0..p.len() {
        s = axes::add(s, axes::cross(p[k], p[(k + 1) % p.len()]));
    }
    let area = axes::dot(s, s).sqrt() / 2.0;
    let d = axes::sub(pb, pa);
    area / axes::dot(d, d).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(k: &[f64], v: &[f64]) -> Vec<f64> {
        (0..18)
            .map(|r| (0..18).map(|c| k[r * 18 + c] * v[c]).sum())
            .collect()
    }

    /// Floor element 2 x 2, t = 0.2, E = 25e6, joint M at the middle of its edge along +Y (probe case T1).
    fn t1() -> Vec<f64> {
        stiffness(
            [2.0, 0.0, 3.0],
            [2.0, 2.0, 3.0],
            [2.0, 1.0, 3.0],
            [0.0, 0.0, 1.0],
            25e6,
            0.2,
            2.0,
            true,
        )
    }

    #[test]
    fn hanging_joint_block_t1() {
        let k = t1();
        // ux, uy, uz, rx, ry, rz of M (rows 12..18), from the reference probe.
        let want = [8e7, 2e7, 8e5, 266666.666667, 1.332e7, 26666666.666667];
        for (d, w) in want.iter().enumerate() {
            let v = k[(12 + d) * 18 + 12 + d];
            assert!((v - w).abs() < 1e-9 * w, "dof {d}: {v} vs {w}");
        }
    }

    #[test]
    fn rigid_motion_and_beam_interpolation_cost_nothing() {
        let k = t1();
        let scale = k.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let p = [[2.0, 0.0, 3.0], [2.0, 2.0, 3.0], [2.0, 1.0, 3.0]];
        for d in 0..6 {
            let mut v = vec![0.0; 18];
            for (q, pt) in p.iter().enumerate() {
                if d < 3 {
                    v[6 * q + d] = 1.0;
                } else {
                    let mut e = [0.0; 3];
                    e[d - 3] = 1.0;
                    let r = axes::sub(*pt, p[0]);
                    let u = axes::cross(e, r);
                    v[6 * q..6 * q + 3].copy_from_slice(&u);
                    v[6 * q + d] = 1.0;
                }
            }
            assert!(
                times(&k, &v).iter().all(|f| f.abs() < 1e-9 * scale),
                "rigid mode {d}"
            );
        }
        // Out-of-plane bending field w = y^3 (rx = dw/dy) along the edge: Hermite reproduces it.
        let mut v = vec![0.0; 18];
        for (q, pt) in p.iter().enumerate() {
            let y = pt[1];
            v[6 * q + 2] = y.powi(3);
            v[6 * q + 3] = 3.0 * y * y;
        }
        assert!(times(&k, &v).iter().all(|f| f.abs() < 1e-8 * scale));
        // Any other motion of M is resisted.
        let mut v = vec![0.0; 18];
        v[12 + 2] = 1.0;
        assert!(times(&k, &v)[14] > 1e5);
    }

    #[test]
    fn width_is_area_over_edge_length() {
        let p = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 1.4, 0.0],
        ];
        assert!((width(&p, p[1], p[2]) - 1.7).abs() < 1e-12);
    }
}
