//! Element formulations in local coordinates.
//!
//! Local DOF order (both ends): [u1, u2, u3, r1, r2, r3] at end i, then the same at end j.
//! End force vectors hold the forces the nodes apply *to the element*, in local axes.

pub mod edge;
pub mod frame;
pub mod shell;
pub mod shell_thick;
pub mod shell_tri;
pub mod truss;
pub mod wall_joint;

use crate::model::FrameKind;

/// Everything an element formulation needs: material, section and length.
#[derive(Debug, Clone, Copy)]
pub struct Props {
    pub kind: FrameKind,
    pub e: f64,
    pub g: f64,
    pub a: f64,
    pub i22: f64,
    pub i33: f64,
    pub j: f64,
    /// Shear areas; 0 means "no shear deformation" in that plane.
    pub as2: f64,
    pub as3: f64,
    pub l: f64,
    /// Rigid-zone lengths at end i and end j (rigid-zone factor x end offset): rigid in bending
    /// and shear; axial and torsion act over the whole length. 0 for most frames. `ri`, `rj` for bending
    /// in the 1-2 plane (V2, M3), `ri3`, `rj3` in the 1-3 plane (V3, M2): a release makes the whole
    /// offset rigid in its own plane only (B55).
    pub ri: f64,
    pub rj: f64,
    pub ri3: f64,
    pub rj3: f64,
    /// End length offsets at end i and end j: end releases act at the clear-length ends, x = oi and
    /// l - oj. 0 for most frames.
    pub oi: f64,
    pub oj: f64,
    /// Axial force of the P-Delta load (tension positive), 0 without P-Delta: its geometric stiffness
    /// is part of the member's stiffness (see `frame::local_stiffness`, `truss::local_stiffness`).
    pub pd: f64,
}

/// A member load already resolved into one local direction (0 = local 1, 1 = local 2, 2 = local 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LocalLoad {
    Point {
        dir: usize,
        p: f64,
        at: f64,
    },
    /// Force per length varying linearly from `w1` at `from` to `w2` at `to` (uniform when equal).
    Distributed {
        dir: usize,
        w1: f64,
        w2: f64,
        from: f64,
        to: f64,
    },
}

impl LocalLoad {
    pub fn uniform(dir: usize, w: f64, from: f64, to: f64) -> LocalLoad {
        LocalLoad::Distributed {
            dir,
            w1: w,
            w2: w,
            from,
            to,
        }
    }
}

/// Resultant and first moment about s = 0 (integrals of w and s w) of the part between `c` and `d` of
/// a load varying linearly from `w1` at `from` to `w2` at `to`; zero where they do not overlap.
pub fn distributed_part(w1: f64, w2: f64, from: f64, to: f64, c: f64, d: f64) -> (f64, f64) {
    let (c, d) = (c.max(from), d.min(to));
    if d <= c {
        return (0.0, 0.0);
    }
    let at = |s: f64| w1 + (w2 - w1) * (s - from) / (to - from);
    let (wc, wd) = (at(c), at(d));
    (
        (wc + wd) / 2.0 * (d - c),
        (d - c) / 6.0 * (c * (2.0 * wc + wd) + d * (wc + 2.0 * wd)),
    )
}

/// 3-point Gauss-Legendre points and weights on [-1, 1] (exact for polynomials of degree 5).
pub const GAUSS3: [(f64, f64); 3] = [
    (-0.774_596_669_241_483_4, 5.0 / 9.0),
    (0.0, 8.0 / 9.0),
    (0.774_596_669_241_483_4, 5.0 / 9.0),
];

pub fn local_stiffness(p: &Props) -> [[f64; 12]; 12] {
    match p.kind {
        FrameKind::Frame => frame::local_stiffness(p),
        FrameKind::Truss => truss::local_stiffness(p),
    }
}

/// Which side of a point load the internal force is taken on, when a point load sits exactly at x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Just before x (the load at x is not yet included).
    Left,
    /// Just after x (the load at x is included).
    Right,
}

/// Internal forces at distance `x` from end i, the common sign convention:
/// [P, V2, V3, T, M2, M3].
/// - P, V2, V3, T act on the positive-1 face in the positive local directions (P > 0 = tension).
/// - Positive M3 compresses the +2 face (sagging for a beam with 2 up); positive M2 compresses the +3 face.
///
/// `side` only matters when a point load sits exactly at `x` (shear/axial jump there).
pub fn internal_forces(end: &[f64; 12], loads: &[LocalLoad], x: f64, side: Side) -> [f64; 6] {
    let passed = |at: f64| match side {
        Side::Left => at < x,
        Side::Right => at <= x,
    };
    // Equilibrium of the part from end i to the cut. `q_sum[d]` = resultant of loads in direction d
    // left of x; `arm[d]` = their moment about the cut, sum of (x - s) * q.
    let mut q_sum = [0.0; 3];
    let mut arm = [0.0; 3];
    for load in loads {
        match *load {
            LocalLoad::Point { dir, p, at } => {
                if passed(at) {
                    q_sum[dir] += p;
                    arm[dir] += (x - at) * p;
                }
            }
            LocalLoad::Distributed {
                dir,
                w1,
                w2,
                from,
                to,
            } => {
                let (f, m0) = distributed_part(w1, w2, from, to, from, x);
                q_sum[dir] += f;
                arm[dir] += x * f - m0;
            }
        }
    }
    let p = -(end[0] + q_sum[0]);
    let v2 = -(end[1] + q_sum[1]);
    let v3 = -(end[2] + q_sum[2]);
    let t = -end[3];
    // Moment vector on the positive face, local 3 component: compresses +2 face when positive.
    let m3 = -end[5] + x * end[1] + arm[1];
    // Moment vector local 2 component is -(end[4] + x*end[2] + arm[2]); a positive vector about 2
    // compresses the -3 face, so M2 (compression on +3) is its negative.
    let m2 = end[4] + x * end[2] + arm[2];
    [p, v2, v3, t, m2, m3]
}

/// Displacements [u1, u2, u3] of the member axis (local axes) at distances `xs` from end i.
///
/// `ul` holds the local end displacements (both ends, 6 each) and `end` the local end forces.
/// Frames integrate the internal forces from end i: axial strain P/EA, curvature M3/EI33 (1-2 plane)
/// and M2/EI22 (1-3 plane), plus shear strain V/GAs when a shear area is given. Between load
/// positions the integrands are polynomials of degree 4 or less (linearly varying loads), so 3-point
/// Gauss per piece is exact. What is left at end j (round-off only) is spread linearly. Trusses: straight line.
/// With P-Delta the moments carry their P-Delta part (`frame::pdelta_moments`), so the integrands are of a
/// higher degree and a small part is left at end j to spread: the element's own error.
///
/// `hinges`: hinges inside the member (releases of a member with end offsets act at its clear-length
/// ends): its `jumps` are the displacement [u1, u2, u3, r1, r2, r3] gained crossing x = oi towards end j,
/// and crossing x = l - oj towards end i. Zero otherwise (releases at the joints are already in `ul`).
pub fn local_displacements(
    p: &Props,
    ul: &[f64; 12],
    end: &[f64; 12],
    loads: &[LocalLoad],
    hinges: &frame::Hinges,
    xs: &[f64],
) -> Vec<[f64; 3]> {
    if let Some((p12, p13)) = frame::planes(p) {
        let a = local_displacements(&p12, ul, end, loads, hinges, xs);
        let b = local_displacements(&p13, ul, end, loads, hinges, xs);
        return a
            .into_iter()
            .zip(b)
            .map(|(a, b)| [a[0], a[1], b[2]])
            .collect();
    }
    let l = p.l;
    let lerp = |x: f64, a: f64, b: f64| a + (b - a) * x / l;
    if p.kind == FrameKind::Truss {
        return xs
            .iter()
            .map(|&x| {
                [
                    lerp(x, ul[0], ul[6]),
                    lerp(x, ul[1], ul[7]),
                    lerp(x, ul[2], ul[8]),
                ]
            })
            .collect();
    }
    let mut breaks: Vec<f64> = loads
        .iter()
        .flat_map(|ld| match *ld {
            LocalLoad::Point { at, .. } => [at, at],
            LocalLoad::Distributed { from, to, .. } => [from, to],
        })
        .filter(|&b| b > 0.0 && b < l)
        .collect();
    breaks.sort_by(|a, b| a.total_cmp(b));
    breaks.dedup();
    // Rigid zones bend and shear not at all: their ends are breaks too.
    for b in [p.ri, l - p.rj] {
        if b > 0.0 && b < l && !breaks.iter().any(|x| (x - b).abs() <= 1e-12 * l) {
            breaks.push(b);
        }
    }
    breaks.sort_by(|a, b| a.total_cmp(b));
    let (ea, ei33, ei22) = (p.e * p.a, p.e * p.i33, p.e * p.i22);
    let flex = |gas: f64| if gas > 0.0 { 1.0 / gas } else { 0.0 };
    let (s2, s3) = (flex(p.g * p.as2), flex(p.g * p.as3));

    let integrate = |x: f64| -> [f64; 3] {
        let mut pts = vec![0.0];
        pts.extend(breaks.iter().copied().filter(|&b| b < x));
        pts.push(x);
        let mut sum = [0.0; 3];
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if b <= a {
                continue;
            }
            let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
            for (g, wt) in GAUSS3 {
                let s = mid + half * g;
                let mut q = internal_forces(end, loads, s, Side::Right);
                let [m2, m3] = frame::pdelta_moments(p, ul, hinges, s);
                q[4] += m2;
                q[5] += m3;
                let bends = if s < p.ri || s > l - p.rj { 0.0 } else { 1.0 };
                let f = [
                    q[0] / ea,
                    bends * ((x - s) * q[5] / ei33 + q[1] * s2),
                    bends * ((x - s) * q[4] / ei22 + q[2] * s3),
                ];
                for d in 0..3 {
                    sum[d] += wt * half * f[d];
                }
            }
        }
        // v2' = r3 + shear strain, v3' = -r2 + shear strain.
        let mut v = [
            ul[0] + sum[0],
            ul[1] + ul[5] * x + sum[1],
            ul[2] - ul[4] * x + sum[2],
        ];
        let jumps = &hinges.jumps;
        for (j, at, sign) in [(&jumps[0], p.oi, 1.0), (&jumps[1], l - p.oj, -1.0)] {
            let past = if sign > 0.0 {
                x >= at
            } else {
                x > at || x >= l
            };
            if past {
                v[0] += sign * j[0];
                v[1] += sign * (j[1] + j[5] * (x - at));
                v[2] += sign * (j[2] - j[4] * (x - at));
            }
        }
        v
    };
    let at_j = integrate(l);
    let gap = [ul[6] - at_j[0], ul[7] - at_j[1], ul[8] - at_j[2]];
    xs.iter()
        .map(|&x| {
            let v = integrate(x);
            [
                v[0] + gap[0] * x / l,
                v[1] + gap[1] * x / l,
                v[2] + gap[2] * x / l,
            ]
        })
        .collect()
}
