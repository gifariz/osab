//! Flat shell element (Shell-Thin, Shell-Thick and Membrane): the 4-node quad here,
//! the 3-node triangle in `shell_tri.rs`, the Shell-Thick plate part in `shell_thick.rs`; [`Shell`]
//! handles all of them.
//!
//! - Membrane: bilinear quad + Allman-type drilling field + two condensed displacement bubbles, 2 x 2
//!   Gauss, mean correction of the drilling strain columns, one-point Hughes-Brezzi penalty with
//!   gamma = 0.4 G, and a small alternating-rotation term.
//! - Thin-plate bending: DKQ (Batoz and Tahar 1982), 2 x 2 Gauss.
//! - Thick-plate bending: a bending part in series with a shear part (shell_thick.rs).
//! - Forces at the 2 x 2 Gauss points, extrapolated bilinearly to the corners, with the common signs
//!   (docs/SOURCES.md): F11 F22 F12, M11 M22 M12 (positive = tension at the bottom),
//!   V13 V23 from moment equilibrium of the bilinear moment field through the Gauss points.
//!
//! Results match the reference results to round-off on the checked single elements, Shell-Thick too
//! (docs/elements/SHELL_THICK.md).
//! Local DOFs per corner: u, v, w, rx, ry, rz in the area's local axes.

use super::{shell_thick, shell_tri};
use crate::axes::{Axes, Vec3};
use crate::model::ShellType;

/// 1 / sqrt(3).
const G: f64 = 0.577_350_269_189_625_8;
/// Gauss points in corner order: (-,-), (+,-), (+,+), (-,+).
const GAUSS: [[f64; 2]; 4] = [[-G, -G], [G, -G], [G, G], [-G, G]];
const CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
/// Drilling penalty factor (gamma = 0.4 G) and alternating-rotation factor, measured (docs/SOURCES.md).
const DRILLING_FACTOR: f64 = 0.4;
const ALTERNATING_FACTOR: f64 = 5.0e-5;
/// Membrane DOFs (u, v, rz) and plate DOFs (w, rx, ry) within a corner's six local DOFs.
const MEM: [usize; 3] = [0, 1, 5];
const PLATE: [usize; 3] = [2, 3, 4];

type Xy = [[f64; 2]; 4];

fn bilinear(xi: f64, eta: f64) -> ([f64; 4], [[f64; 4]; 2]) {
    let mut n = [0.0; 4];
    let mut d = [[0.0; 4]; 2];
    for (a, c) in CORNERS.iter().enumerate() {
        n[a] = 0.25 * (1.0 + c[0] * xi) * (1.0 + c[1] * eta);
        d[0][a] = 0.25 * c[0] * (1.0 + c[1] * eta);
        d[1][a] = 0.25 * c[1] * (1.0 + c[0] * xi);
    }
    (n, d)
}

/// Jacobian rows (x,xi y,xi) and (x,eta y,eta), its determinant and inverse.
fn jacobian(xy: &Xy, xi: f64, eta: f64) -> (f64, [[f64; 2]; 2]) {
    let (_, d) = bilinear(xi, eta);
    let mut j = [[0.0; 2]; 2];
    for r in 0..2 {
        for c in 0..2 {
            j[r][c] = (0..4).map(|a| d[r][a] * xy[a][c]).sum();
        }
    }
    let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
    let inv = [
        [j[1][1] / det, -j[0][1] / det],
        [-j[1][0] / det, j[0][0] / det],
    ];
    (det, inv)
}

/// Natural derivatives -> Cartesian (d/dx, d/dy).
fn cart(inv: &[[f64; 2]; 2], dxi: f64, deta: f64) -> [f64; 2] {
    [
        inv[0][0] * dxi + inv[0][1] * deta,
        inv[1][0] * dxi + inv[1][1] * deta,
    ]
}

/// Midside (hierarchical / serendipity) functions on sides (1,2), (2,3), (3,4), (4,1): values and
/// natural derivatives.
fn midside(xi: f64, eta: f64) -> ([f64; 4], [[f64; 4]; 2]) {
    let n = [
        0.5 * (1.0 - xi * xi) * (1.0 - eta),
        0.5 * (1.0 + xi) * (1.0 - eta * eta),
        0.5 * (1.0 - xi * xi) * (1.0 + eta),
        0.5 * (1.0 - xi) * (1.0 - eta * eta),
    ];
    let d = [
        [
            -xi * (1.0 - eta),
            0.5 * (1.0 - eta * eta),
            -xi * (1.0 + eta),
            -0.5 * (1.0 - eta * eta),
        ],
        [
            -0.5 * (1.0 - xi * xi),
            -eta * (1.0 + xi),
            0.5 * (1.0 - xi * xi),
            -eta * (1.0 - xi),
        ],
    ];
    (n, d)
}

/// Plane-stress matrix times `s` (t for membranes, t^3/12 for bending).
fn plane_stress(e: f64, nu: f64, s: f64) -> [[f64; 3]; 3] {
    let f = e / (1.0 - nu * nu) * s;
    [
        [f, f * nu, 0.0],
        [f * nu, f, 0.0],
        [0.0, 0.0, f * (1.0 - nu) / 2.0],
    ]
}

/// `c` with the property modifiers on its terms: c11 x a11, c22 x a22, the Poisson coupling
/// c12 x sqrt(a11 a22) and c33 x a12 (docs/phases/PHASE4_PLAN.md, probe findings 1b).
fn modified(c: [[f64; 3]; 3], a11: f64, a22: f64, a12: f64) -> [[f64; 3]; 3] {
    let k = (a11 * a22).sqrt();
    [
        [c[0][0] * a11, c[0][1] * k, c[0][2]],
        [c[1][0] * k, c[1][1] * a22, c[1][2]],
        [c[2][0], c[2][1], c[2][2] * a12],
    ]
}

/// Bilinear extrapolation of values at the Gauss points (corner order) to the corners.
fn extrapolate<const N: usize>(g: &[[f64; N]; 4]) -> [[f64; N]; 4] {
    let s = 3.0f64.sqrt();
    let mut out = [[0.0; N]; 4];
    for (c, corner) in CORNERS.iter().enumerate() {
        let (n, _) = bilinear(corner[0] * s, corner[1] * s);
        for a in 0..4 {
            for k in 0..N {
                out[c][k] += n[a] * g[a][k];
            }
        }
    }
    out
}

/// In-plane rigidity (plane stress times t) with the f11, f22, f12 modifiers.
fn membrane_c(e: f64, nu: f64, t: f64, mods: &[f64; 8]) -> [[f64; 3]; 3] {
    modified(plane_stress(e, nu, t), mods[0], mods[1], mods[2])
}

/// Plate-bending rigidity (plane stress times t^3 / 12) with the m11, m22, m12 modifiers.
fn plate_d(e: f64, nu: f64, t: f64, mods: &[f64; 8]) -> [[f64; 3]; 3] {
    modified(
        plane_stress(e, nu, t.powi(3) / 12.0),
        mods[3],
        mods[4],
        mods[5],
    )
}

// ---------------------------------------------------------------- membrane

/// Strain operator (3 x 14: u, v, rz per corner, then the two bubbles) and the rotation constraint row
/// (continuum rotation minus interpolated drilling rotation) at one point.
fn membrane_fields(xy: &Xy, xi: f64, eta: f64) -> ([[f64; 14]; 3], [f64; 14], f64) {
    let (n4, d4n) = bilinear(xi, eta);
    let (det, inv) = jacobian(xy, xi, eta);
    let (_, dmn) = midside(xi, eta);
    let mut b = [[0.0; 14]; 3];
    let mut rot = [0.0; 14];
    for a in 0..4 {
        let d = cart(&inv, d4n[0][a], d4n[1][a]);
        b[0][3 * a] = d[0];
        b[1][3 * a + 1] = d[1];
        b[2][3 * a] = d[1];
        b[2][3 * a + 1] = d[0];
        rot[3 * a] = -0.5 * d[1];
        rot[3 * a + 1] = 0.5 * d[0];
    }
    for k in 0..4 {
        let (i, j) = (k, (k + 1) % 4);
        // Midside offset (l/8)(r_j - r_i) n, with l n = (y_j - y_i, x_i - x_j) for counterclockwise corners.
        let cx = (xy[j][1] - xy[i][1]) / 8.0;
        let cy = (xy[i][0] - xy[j][0]) / 8.0;
        let dm = cart(&inv, dmn[0][k], dmn[1][k]);
        for (node, sgn) in [(j, 1.0), (i, -1.0)] {
            let c = 3 * node + 2;
            b[0][c] += sgn * cx * dm[0];
            b[1][c] += sgn * cy * dm[1];
            b[2][c] += sgn * (cx * dm[1] + cy * dm[0]);
            rot[c] += sgn * 0.5 * (cy * dm[0] - cx * dm[1]);
        }
    }
    for a in 0..4 {
        rot[3 * a + 2] -= n4[a];
    }
    // Displacement bubbles (1 - xi^2)(1 - eta^2) in u and v.
    let bub = cart(
        &inv,
        -2.0 * xi * (1.0 - eta * eta),
        -2.0 * eta * (1.0 - xi * xi),
    );
    b[0][12] = bub[0];
    b[2][12] = bub[1];
    b[1][13] = bub[1];
    b[2][13] = bub[0];
    rot[12] = -0.5 * bub[1];
    rot[13] = 0.5 * bub[0];
    (b, rot, det)
}

/// Membrane stiffness (12 x 12, DOFs u, v, rz per corner) and the corrected strain operators at the
/// Gauss points (first 12 columns), used for the reported forces. `c`: the in-plane rigidity (plane
/// stress times t, with any modifiers); `gt`: G t times the f12 modifier, for the drilling terms.
fn membrane(xy: &Xy, c: &[[f64; 3]; 3], gt: f64) -> ([[f64; 12]; 12], [[[f64; 12]; 3]; 4]) {
    let mut pts: Vec<([[f64; 14]; 3], f64)> = GAUSS
        .iter()
        .map(|p| {
            let (b, _, det) = membrane_fields(xy, p[0], p[1]);
            (b, det)
        })
        .collect();
    let area: f64 = pts.iter().map(|(_, d)| d).sum();
    // Mean correction of the four drilling columns only.
    for col in [2, 5, 8, 11] {
        for r in 0..3 {
            let mean = pts.iter().map(|(b, d)| b[r][col] * d).sum::<f64>() / area;
            for (b, _) in pts.iter_mut() {
                b[r][col] -= mean;
            }
        }
    }
    let mut k = [[0.0; 14]; 14];
    for (b, det) in &pts {
        let mut cb = [[0.0; 14]; 3];
        for r in 0..3 {
            for q in 0..14 {
                cb[r][q] = (0..3).map(|s| c[r][s] * b[s][q]).sum();
            }
        }
        for p in 0..14 {
            for q in 0..14 {
                k[p][q] += (0..3).map(|r| b[r][p] * cb[r][q]).sum::<f64>() * det;
            }
        }
    }
    let (_, b0, det0) = membrane_fields(xy, 0.0, 0.0);
    let pen = DRILLING_FACTOR * gt * 4.0 * det0;
    let alt = ALTERNATING_FACTOR * gt * area;
    let h = [1.0, -1.0, 1.0, -1.0];
    for p in 0..14 {
        for q in 0..14 {
            k[p][q] += pen * b0[p] * b0[q];
        }
    }
    for (i, hi) in h.iter().enumerate() {
        for (j, hj) in h.iter().enumerate() {
            k[3 * i + 2][3 * j + 2] += alt * hi * hj;
        }
    }
    // Condense the two bubbles.
    let (a, bb, cc) = (k[12][12], k[12][13], k[13][13]);
    let det = a * cc - bb * bb;
    let kinv = [[cc / det, -bb / det], [-bb / det, a / det]];
    let mut out = [[0.0; 12]; 12];
    for p in 0..12 {
        for q in 0..12 {
            let mut v = k[p][q];
            for r in 0..2 {
                for s in 0..2 {
                    v -= k[p][12 + r] * kinv[r][s] * k[12 + s][q];
                }
            }
            out[p][q] = v;
        }
    }
    let mut corrected = [[[0.0; 12]; 3]; 4];
    for (gp, (b, _)) in pts.iter().enumerate() {
        for r in 0..3 {
            corrected[gp][r].copy_from_slice(&b[r][..12]);
        }
    }
    (out, corrected)
}

// ---------------------------------------------------------------- DKQ

/// Serendipity corner and midside functions (values or one natural derivative), 8 entries.
fn serendipity(xi: f64, eta: f64, which: usize) -> [f64; 8] {
    let mut out = [0.0; 8];
    for (a, c) in CORNERS.iter().enumerate() {
        let (xa, ya) = (c[0], c[1]);
        out[a] = match which {
            0 => 0.25 * (1.0 + xi * xa) * (1.0 + eta * ya) * (xi * xa + eta * ya - 1.0),
            1 => 0.25 * xa * (1.0 + eta * ya) * (2.0 * xi * xa + eta * ya),
            _ => 0.25 * ya * (1.0 + xi * xa) * (xi * xa + 2.0 * eta * ya),
        };
    }
    let (n, d) = midside(xi, eta);
    for k in 0..4 {
        out[4 + k] = match which {
            0 => n[k],
            1 => d[0][k],
            _ => d[1][k],
        };
    }
    out
}

/// beta_x = Hx . U and beta_y = Hy . U (U = w, rx, ry per corner), Batoz and Tahar 1982.
fn dkq_h(xy: &Xy, n: &[f64; 8]) -> ([f64; 12], [f64; 12]) {
    let mut co = [[0.0; 5]; 4];
    for k in 0..4 {
        let (i, j) = (k, (k + 1) % 4);
        let (x, y) = (xy[i][0] - xy[j][0], xy[i][1] - xy[j][1]);
        let l2 = x * x + y * y;
        co[k] = [
            -x / l2,
            0.75 * x * y / l2,
            (0.25 * x * x - 0.5 * y * y) / l2,
            -y / l2,
            (0.25 * y * y - 0.5 * x * x) / l2,
        ];
    }
    let mut hx = [0.0; 12];
    let mut hy = [0.0; 12];
    for a in 0..4 {
        let (k, m) = (a, (a + 3) % 4);
        let (nk, nm) = (n[4 + k], n[4 + m]);
        let (ck, cm) = (co[k], co[m]);
        hx[3 * a] = 1.5 * (ck[0] * nk - cm[0] * nm);
        hx[3 * a + 1] = ck[1] * nk + cm[1] * nm;
        hx[3 * a + 2] = n[a] - ck[2] * nk - cm[2] * nm;
        hy[3 * a] = 1.5 * (ck[3] * nk - cm[3] * nm);
        hy[3 * a + 1] = -n[a] + ck[4] * nk + cm[4] * nm;
        hy[3 * a + 2] = -hx[3 * a + 1];
    }
    (hx, hy)
}

/// Curvature operator [beta_x,x; beta_y,y; beta_x,y + beta_y,x] (3 x 12) and det J.
fn dkq_b(xy: &Xy, xi: f64, eta: f64) -> ([[f64; 12]; 3], f64) {
    let (det, inv) = jacobian(xy, xi, eta);
    let (hxi, hyi) = dkq_h(xy, &serendipity(xi, eta, 1));
    let (hxe, hye) = dkq_h(xy, &serendipity(xi, eta, 2));
    let mut b = [[0.0; 12]; 3];
    for q in 0..12 {
        let hx = cart(&inv, hxi[q], hxe[q]);
        let hy = cart(&inv, hyi[q], hye[q]);
        b[0][q] = hx[0];
        b[1][q] = hy[1];
        b[2][q] = hx[1] + hy[0];
    }
    (b, det)
}

fn dkq(xy: &Xy, d: &[[f64; 3]; 3]) -> [[f64; 12]; 12] {
    let mut k = [[0.0; 12]; 12];
    for p in &GAUSS {
        let (b, det) = dkq_b(xy, p[0], p[1]);
        for i in 0..12 {
            for j in 0..12 {
                let mut v = 0.0;
                for r in 0..3 {
                    for s in 0..3 {
                        v += b[r][i] * d[r][s] * b[s][j];
                    }
                }
                k[i][j] += v * det;
            }
        }
    }
    k
}

// ---------------------------------------------------------------- element

/// The element's shape and what force recovery needs from it.
enum Shape {
    /// Quad: local corner coordinates and the corrected membrane strain operators at the Gauss points.
    Quad {
        xy: Xy,
        mem_b: Box<[[[f64; 12]; 3]; 4]>,
    },
    /// Triangle: local corner coordinates and the mean of the drilling strain columns.
    Tri {
        xy: shell_tri::Xy3,
        mean: [[f64; 3]; 3],
    },
}

/// Thick-plate data for the force recovery (Shell-Thick only).
type Thick = Option<Box<shell_thick::ThickPlate>>;

/// One shell element (4 or 3 corners), ready to assemble and to recover forces.
pub struct Shell {
    pub kind: ShellType,
    pub axes: Axes,
    shape: Shape,
    e: f64,
    nu: f64,
    t: f64,
    /// Stiffness modifiers f11 f22 f12 m11 m22 m12 v13 v23 (all 1 for none; the Stiff slab type
    /// has m11 = m22 = m12 = 100).
    mods: [f64; 8],
    thick: Thick,
    /// Local stiffness, 6n x 6n row-major (u, v, w, rx, ry, rz per corner).
    k_local: Vec<f64>,
}

impl Shell {
    /// `p`: corners in analysed order (4 or 3); `axes`: the area's local axes.
    pub fn new(kind: ShellType, p: &[Vec3], axes: Axes, e: f64, nu: f64, t: f64) -> Shell {
        Shell::with_modifiers(kind, p, axes, e, nu, t, [1.0; 8])
    }

    /// As [`Shell::new`], with the stiffness modifiers `mods` = f11 f22 f12 m11 m22 m12 v13 v23
    /// (see [`crate::model::AreaModifiers`]).
    pub fn with_modifiers(
        kind: ShellType,
        p: &[Vec3],
        axes: Axes,
        e: f64,
        nu: f64,
        t: f64,
        mods: [f64; 8],
    ) -> Shell {
        let xy = crate::area::local_xy(p, &axes);
        let n = xy.len();
        let size = 6 * n;
        let mut k_local = vec![0.0; size * size];
        let mut put = |k: &[f64], dofs: [usize; 3]| {
            let m = 3 * n;
            for i in 0..m {
                for j in 0..m {
                    let gi = 6 * (i / 3) + dofs[i % 3];
                    let gj = 6 * (j / 3) + dofs[j % 3];
                    k_local[gi * size + gj] += k[i * m + j];
                }
            }
        };
        let c = membrane_c(e, nu, t, &mods);
        let gt = e / (2.0 * (1.0 + nu)) * t * mods[2];
        let d = plate_d(e, nu, t, &mods);
        let kgt = shell_thick::shear_rigidity(e, nu, t);
        let kgt = [kgt * mods[6], kgt * mods[7]];
        let mut thick: Thick = None;
        let shape = if n == 4 {
            let xy: Xy = xy.try_into().expect("4 corners");
            let (km, mem_b) = membrane(&xy, &c, gt);
            put(km.as_flattened(), MEM);
            match kind {
                ShellType::ShellThin => put(dkq(&xy, &d).as_flattened(), PLATE),
                ShellType::ShellThick => {
                    let plate = shell_thick::quad(&xy, &d, kgt);
                    put(&plate.k, PLATE);
                    thick = Some(Box::new(plate));
                }
                ShellType::Membrane => {}
            }
            Shape::Quad {
                xy,
                mem_b: Box::new(mem_b),
            }
        } else {
            let xy: shell_tri::Xy3 = xy.try_into().expect("3 corners");
            let (km, mean) = shell_tri::membrane(&xy, &c, gt / t, t);
            put(km.as_flattened(), MEM);
            match kind {
                ShellType::ShellThin => put(shell_tri::dkt(&xy, &d).as_flattened(), PLATE),
                ShellType::ShellThick => {
                    let plate = shell_thick::tri(&xy, &d, kgt);
                    put(&plate.k, PLATE);
                    thick = Some(Box::new(plate));
                }
                ShellType::Membrane => {}
            }
            Shape::Tri { xy, mean }
        };
        Shell {
            kind,
            axes,
            shape,
            e,
            nu,
            t,
            mods,
            thick,
            k_local,
        }
    }

    /// Number of corners (4 or 3).
    pub fn corners(&self) -> usize {
        match self.shape {
            Shape::Quad { .. } => 4,
            Shape::Tri { .. } => 3,
        }
    }

    /// Global stiffness, 6n x 6n row-major: T^T k T with T = blockdiag(R) over 2n vectors of 3.
    pub fn k_global(&self) -> Vec<f64> {
        self.to_global(&self.k_local)
    }

    /// For a Membrane element, the plate-bending stiffness of a Shell-Thin element of the same
    /// thickness (global, 6n x 6n). Membrane elements get this stiffness times a vanishing
    /// factor (probes m_cant, m_tri: joints held only by Membrane floors follow it); the solver uses it
    /// only to place such joints, which it otherwise holds at zero.
    pub fn membrane_plate_global(&self) -> Option<Vec<f64>> {
        if self.kind != ShellType::Membrane {
            return None;
        }
        let n = self.corners();
        let size = 6 * n;
        let d = plane_stress(self.e, self.nu, self.t.powi(3) / 12.0);
        let kp: Vec<f64> = match &self.shape {
            Shape::Quad { xy, .. } => dkq(xy, &d).as_flattened().to_vec(),
            Shape::Tri { xy, .. } => shell_tri::dkt(xy, &d).as_flattened().to_vec(),
        };
        let mut k = vec![0.0; size * size];
        let m = 3 * n;
        for i in 0..m {
            for j in 0..m {
                let gi = 6 * (i / 3) + PLATE[i % 3];
                let gj = 6 * (j / 3) + PLATE[j % 3];
                k[gi * size + gj] += kp[i * m + j];
            }
        }
        Some(self.to_global(&k))
    }

    /// T^T k T for a local 6n x 6n matrix.
    fn to_global(&self, k_local: &[f64]) -> Vec<f64> {
        let r = &self.axes;
        let n = 6 * self.corners();
        let blocks = 2 * self.corners();
        // k T: each 3-column block multiplied by R from the right.
        let mut kt = vec![0.0; n * n];
        for i in 0..n {
            for bj in 0..blocks {
                for c in 0..3 {
                    kt[i * n + 3 * bj + c] =
                        (0..3).map(|m| k_local[i * n + 3 * bj + m] * r[m][c]).sum();
                }
            }
        }
        let mut out = vec![0.0; n * n];
        for bi in 0..blocks {
            for c in 0..3 {
                for j in 0..n {
                    out[(3 * bi + c) * n + j] =
                        (0..3).map(|m| r[m][c] * kt[(3 * bi + m) * n + j]).sum();
                }
            }
        }
        out
    }

    /// Global displacements (6n) -> local (6n).
    fn to_local(&self, u: &[f64]) -> Vec<f64> {
        let mut l = vec![0.0; 6 * self.corners()];
        for b in 0..2 * self.corners() {
            for r in 0..3 {
                l[3 * b + r] = (0..3).map(|c| self.axes[r][c] * u[3 * b + c]).sum();
            }
        }
        l
    }

    /// Global joint forces (translations only, one per corner) of a uniform load `q` (global vector
    /// per area): f_a = q * int(N_a dA).
    pub fn area_load(&self, q: Vec3) -> Vec<Vec3> {
        match &self.shape {
            Shape::Quad { xy, .. } => {
                let mut f = vec![[0.0; 3]; 4];
                for p in &GAUSS {
                    let (n, _) = bilinear(p[0], p[1]);
                    let (det, _) = jacobian(xy, p[0], p[1]);
                    for a in 0..4 {
                        for d in 0..3 {
                            f[a][d] += n[a] * det * q[d];
                        }
                    }
                }
                f
            }
            Shape::Tri { xy, .. } => shell_tri::area_load(xy, q).to_vec(),
        }
    }

    /// Geometric stiffness (global, 6n x 6n) of the membrane forces [F11, F22, F12] the global corner
    /// displacements `u_global` give (P-Delta from loads): ∫ ∇Nᵀ S ∇N dA on each of the three
    /// translations, N the corners' bilinear shape functions (a triangle's linear ones),
    /// S = [[F11, F12], [F12, F22]] at each of the quad's 2 x 2 integration points (a triangle: its mean
    /// forces, three quarters of the result): the initial-stress stiffness of plane stress (e.g. Cook, Malkus & Plesha, "Concepts and
    /// Applications of Finite Element Analysis", the chapter on buckling). Nothing on the rotations.
    pub fn geometric(&self, u_global: &[f64]) -> Vec<f64> {
        let n = self.corners();
        let size = 6 * n;
        let mut k = vec![0.0; size * size];
        let mut add = |grads: &[[f64; 2]], s: [f64; 3], w: f64| {
            for a in 0..n {
                for b in 0..n {
                    let ga = grads[a];
                    let gb = grads[b];
                    let v = w
                        * (ga[0] * (s[0] * gb[0] + s[2] * gb[1])
                            + ga[1] * (s[2] * gb[0] + s[1] * gb[1]));
                    for c in 0..3 {
                        k[(6 * a + c) * size + 6 * b + c] += v;
                    }
                }
            }
        };
        match &self.shape {
            Shape::Quad { xy, mem_b } => {
                let ul = self.to_local(u_global);
                let um: Vec<f64> = (0..4).flat_map(|a| MEM.map(|d| ul[6 * a + d])).collect();
                let c = membrane_c(self.e, self.nu, self.t, &self.mods);
                for (gp, p) in GAUSS.iter().enumerate() {
                    let b = &mem_b[gp];
                    let e: [f64; 3] =
                        std::array::from_fn(|r| (0..12).map(|q| b[r][q] * um[q]).sum());
                    let s: [f64; 3] = std::array::from_fn(|r| (0..3).map(|q| c[r][q] * e[q]).sum());
                    let (_, dn) = bilinear(p[0], p[1]);
                    let (det, inv) = jacobian(xy, p[0], p[1]);
                    let grads: Vec<[f64; 2]> =
                        (0..4).map(|a| cart(&inv, dn[0][a], dn[1][a])).collect();
                    add(&grads, s, det);
                }
            }
            Shape::Tri { xy, .. } => {
                // A triangle: the mean of its membrane forces, and three quarters of the stiffness this
                // gives (measured, docs/SOURCES.md).
                let (area, dl) = shell_tri::geometry(xy);
                let forces = self.corner_forces(u_global);
                let s: [f64; 3] =
                    std::array::from_fn(|r| forces.iter().map(|f| f[r]).sum::<f64>() / 3.0);
                let grads: Vec<[f64; 2]> = (0..3).map(|a| [dl[0][a], dl[1][a]]).collect();
                add(&grads, s, 0.75 * area);
            }
        }
        self.to_global(&k)
    }

    /// Corner forces [F11, F22, F12, M11, M22, M12, V13, V23] (per unit length, positive M = tension at the bottom) from the
    /// global corner displacements (6 per corner).
    pub fn corner_forces(&self, u_global: &[f64]) -> Vec<[f64; 8]> {
        let ul = self.to_local(u_global);
        let n = self.corners();
        let um: Vec<f64> = (0..n).flat_map(|a| MEM.map(|d| ul[6 * a + d])).collect();
        let up: Vec<f64> = (0..n).flat_map(|a| PLATE.map(|d| ul[6 * a + d])).collect();
        let c = membrane_c(self.e, self.nu, self.t, &self.mods);
        let (xy, mem_b) = match &self.shape {
            Shape::Quad { xy, mem_b } => (xy, mem_b),
            Shape::Tri { xy, mean } => {
                let d = plate_d(self.e, self.nu, self.t, &self.mods);
                let um: [f64; 9] = um.try_into().expect("9 membrane DOFs");
                let up9: [f64; 9] = up.clone().try_into().expect("9 plate DOFs");
                let plate = (self.kind == ShellType::ShellThin).then_some((&d, &up9));
                let mut out = shell_tri::corner_forces(xy, mean, &c, &um, plate).to_vec();
                if let Some(th) = &self.thick {
                    let (m, v) = th.forces(&up);
                    for (k, row) in out.iter_mut().enumerate() {
                        row[3..6].copy_from_slice(&m[k]);
                        row[6..8].copy_from_slice(&v);
                    }
                }
                return out;
            }
        };
        let mut f = [[0.0; 3]; 4];
        for (gp, b) in mem_b.iter().enumerate() {
            let strain: [f64; 3] = std::array::from_fn(|r| (0..12).map(|q| b[r][q] * um[q]).sum());
            f[gp] = std::array::from_fn(|r| (0..3).map(|s| c[r][s] * strain[s]).sum());
        }
        let fc = extrapolate(&f);
        let mut out = vec![[0.0; 8]; 4];
        for k in 0..4 {
            out[k][..3].copy_from_slice(&fc[k]);
        }
        if let Some(th) = &self.thick {
            // Thick plates: moments from the bending part at the Gauss points, extrapolated; shears
            // kappa G t times the centre gradient of the shear deflection (the reported shears).
            let (m, v) = th.forces(&up);
            let m4: [[f64; 3]; 4] = std::array::from_fn(|g| m[g]);
            let mc = extrapolate(&m4);
            for k in 0..4 {
                out[k][3..6].copy_from_slice(&mc[k]);
                out[k][6..8].copy_from_slice(&v);
            }
        }
        if self.kind == ShellType::ShellThin {
            let d = plate_d(self.e, self.nu, self.t, &self.mods);
            let mut m = [[0.0; 3]; 4];
            for (gp, p) in GAUSS.iter().enumerate() {
                let (b, _) = dkq_b(xy, p[0], p[1]);
                let kappa: [f64; 3] =
                    std::array::from_fn(|r| (0..12).map(|q| b[r][q] * up[q]).sum());
                // Sign: M11 = -int(x3 s11), so positive moments give tension at the bottom.
                m[gp] = std::array::from_fn(|r| -(0..3).map(|s| d[r][s] * kappa[s]).sum::<f64>());
            }
            let mc = extrapolate(&m);
            // Thin plates: shears from equilibrium of the bilinear moment field through the Gauss values.
            let mut v = [[0.0; 2]; 4];
            for (gp, p) in GAUSS.iter().enumerate() {
                let (_, dn) = bilinear(p[0], p[1]);
                let (_, inv) = jacobian(xy, p[0], p[1]);
                let mut dmx = [0.0; 3];
                let mut dmy = [0.0; 3];
                for a in 0..4 {
                    let dd = cart(&inv, dn[0][a], dn[1][a]);
                    for r in 0..3 {
                        dmx[r] += dd[0] * mc[a][r];
                        dmy[r] += dd[1] * mc[a][r];
                    }
                }
                v[gp] = [-(dmx[0] + dmy[2]), -(dmx[2] + dmy[1])];
            }
            let vc = extrapolate(&v);
            for k in 0..4 {
                out[k][3..6].copy_from_slice(&mc[k]);
                out[k][6..8].copy_from_slice(&vc[k]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axes::area_axes;

    fn rigid_modes(p: &[Vec3]) -> Vec<[f64; 24]> {
        let c = [
            p.iter().map(|q| q[0]).sum::<f64>() / 4.0,
            p.iter().map(|q| q[1]).sum::<f64>() / 4.0,
            p.iter().map(|q| q[2]).sum::<f64>() / 4.0,
        ];
        let mut modes = Vec::new();
        for d in 0..3 {
            let mut m = [0.0; 24];
            for a in 0..4 {
                m[6 * a + d] = 1.0;
            }
            modes.push(m);
        }
        for d in 0..3 {
            let mut w = [0.0; 3];
            w[d] = 1.0;
            let mut m = [0.0; 24];
            for a in 0..4 {
                let r = crate::axes::sub(p[a], c);
                let u = crate::axes::cross(w, r);
                m[6 * a..6 * a + 3].copy_from_slice(&u);
                m[6 * a + 3 + d] = 1.0;
            }
            modes.push(m);
        }
        modes
    }

    #[test]
    fn rigid_body_motion_gives_no_force_and_stiffness_is_symmetric() {
        let p = [
            [0.0, 0.0, 0.0],
            [2.0, 0.5, 0.5],
            [2.3, 2.0, 1.3],
            [0.1, 1.6, 1.0],
        ];
        for kind in [ShellType::ShellThin, ShellType::Membrane] {
            let ax = area_axes(&p, 20.0);
            let s = Shell::new(kind, &p, ax, 25e6, 0.2, 0.2);
            let k = s.k_global();
            let big = k.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            for i in 0..24 {
                for j in 0..24 {
                    assert!((k[i * 24 + j] - k[j * 24 + i]).abs() < 1e-9 * big);
                }
            }
            let flat: Vec<Vec3> = {
                // The element works in its local plane; rigid motions of those flat corners.
                let xy = crate::area::local_xy(&p, &ax);
                xy.iter()
                    .map(|q| {
                        crate::axes::add(
                            crate::axes::scale(ax[0], q[0]),
                            crate::axes::scale(ax[1], q[1]),
                        )
                    })
                    .collect()
            };
            for m in rigid_modes(&flat) {
                for i in 0..24 {
                    let f: f64 = (0..24).map(|j| k[i * 24 + j] * m[j]).sum();
                    assert!(f.abs() < 1e-8 * big, "{kind:?} row {i}: {f}");
                }
                let forces = s.corner_forces(&m);
                for c in forces {
                    for v in c {
                        assert!(v.abs() < 1e-6, "{kind:?}: rigid motion gives force {v}");
                    }
                }
            }
        }
    }

    #[test]
    fn stiff_bending_factor_scales_only_the_plate_part() {
        // Stiff slab: m11 = m22 = m12 = 100, membrane unchanged (docs/elements/STIFF_AREA.md).
        let quad = vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.3, 0.0],
            [2.2, 1.5, 0.0],
            [0.1, 1.2, 0.0],
        ];
        let tri = vec![[0.0, 0.0, 0.0], [2.0, 0.5, 0.0], [0.6, 1.8, 0.0]];
        for p in [quad, tri] {
            let ax = area_axes(&p, 0.0);
            let a = Shell::new(ShellType::ShellThin, &p, ax, 25e6, 0.2, 0.2);
            let stiff = [1.0, 1.0, 1.0, 100.0, 100.0, 100.0, 1.0, 1.0];
            let b = Shell::with_modifiers(ShellType::ShellThin, &p, ax, 25e6, 0.2, 0.2, stiff);
            let n = 6 * a.corners();
            let plate = |i: usize| PLATE.contains(&(i % 6));
            for i in 0..n {
                for j in 0..n {
                    let f = if plate(i) && plate(j) { 100.0 } else { 1.0 };
                    let (ka, kb) = (a.k_local[i * n + j], b.k_local[i * n + j]);
                    assert!((kb - f * ka).abs() <= 1e-9 * kb.abs().max(1.0), "{i} {j}");
                }
            }
            // Same displacements: moments 100 times, membrane forces the same.
            let u: Vec<f64> = (0..n).map(|k| 1e-4 * ((k * 7 % 5) as f64 - 2.0)).collect();
            for (fa, fb) in a.corner_forces(&u).iter().zip(b.corner_forces(&u)) {
                for c in 0..8 {
                    let f = if (3..8).contains(&c) { 100.0 } else { 1.0 };
                    assert!((fb[c] - f * fa[c]).abs() <= 1e-9 * fb[c].abs().max(1.0));
                }
            }
        }
    }

    #[test]
    fn triangle_rigid_body_motion_gives_no_force_and_stiffness_is_symmetric() {
        let p = [[0.0, 0.0, 0.0], [2.0, 0.5, 0.5], [0.6, 1.8, 1.2]];
        for kind in [ShellType::ShellThin, ShellType::Membrane] {
            let ax = area_axes(&p, 35.0);
            let s = Shell::new(kind, &p, ax, 25e6, 0.2, 0.2);
            assert_eq!(s.corners(), 3);
            let k = s.k_global();
            let big = k.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            for i in 0..18 {
                for j in 0..18 {
                    assert!((k[i * 18 + j] - k[j * 18 + i]).abs() < 1e-9 * big);
                }
            }
            let c = [
                p.iter().map(|q| q[0]).sum::<f64>() / 3.0,
                p.iter().map(|q| q[1]).sum::<f64>() / 3.0,
                p.iter().map(|q| q[2]).sum::<f64>() / 3.0,
            ];
            for d in 0..6 {
                let mut m = [0.0; 18];
                for a in 0..3 {
                    if d < 3 {
                        m[6 * a + d] = 1.0;
                    } else {
                        let mut w = [0.0; 3];
                        w[d - 3] = 1.0;
                        let u = crate::axes::cross(w, crate::axes::sub(p[a], c));
                        m[6 * a..6 * a + 3].copy_from_slice(&u);
                        m[6 * a + d] = 1.0;
                    }
                }
                for i in 0..18 {
                    let f: f64 = (0..18).map(|j| k[i * 18 + j] * m[j]).sum();
                    assert!(f.abs() < 1e-8 * big, "{kind:?} mode {d} row {i}: {f}");
                }
                for corner in s.corner_forces(&m) {
                    for v in corner {
                        assert!(v.abs() < 1e-6, "{kind:?}: rigid motion gives force {v}");
                    }
                }
            }
        }
    }
}
