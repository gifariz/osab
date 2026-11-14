//! Plate part of the Shell-Thick element, identified from black-box runs of single elements
//! (docs/elements/SHELL_THICK.md; Python reference for quads
//! checks/thick_q9_law.py).
//!
//! The element is a thin *bending part* in series with a *shear part* that moves w only (like the
//! shear deformation of a frame): nodal w = w_bending + w_shear, rotations from the bending part only.
//!
//! - Quadrilateral bending: bilinear corner rotations plus four hierarchical midside vectors and
//!   an interior vector bubble (Q9). All ten internal curvature columns are mean corrected with
//!   the eight-point degree-five rule. Full curl energy is scaled by 1000 trace(D), and the four
//!   oriented integrated edge residuals share one exact circulation (three KKT constraints).
//!   Moments use the same energy field at the four diagonal sqrt(7/9) points, extrapolated
//!   bilinearly to the corners. One law applies to every quad; see docs/elements/SHELL_THICK.md.
//! - Shear part (per kappa G t, kappa = 5/6): one-point Laplacian on w plus one hourglass term with the
//!   bilinear hourglass energy evaluated by a positive eight-point, degree-five square rule.
//! - Triangles: quadratic rotations with mean-corrected bending and full curl energy. The three
//!   oriented edge residuals share one exact common circulation; the internal variables are that
//!   circulation and three normal midside increments. Shear is the exact linear Laplacian.

/// Shear correction factor.
const KAPPA: f64 = 5.0 / 6.0;
const CURL_PENALTY: f64 = 1000.0;
const G: f64 = 0.577_350_269_189_625_8;
const GAUSS: [[f64; 2]; 4] = [[-G, -G], [G, -G], [G, G], [-G, G]];
const CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
// Diagonal radius sqrt(7/9), axis radius sqrt(7/15); weights integrate the reference square.
const SHEAR_DIAGONAL: f64 = 0.881_917_103_688_196_9;
const SHEAR_AXIS: f64 = 0.683_130_051_063_973_2;
const SHEAR_POINTS: [(f64, f64, f64); 8] = [
    (-SHEAR_DIAGONAL, -SHEAR_DIAGONAL, 9.0 / 49.0),
    (SHEAR_DIAGONAL, -SHEAR_DIAGONAL, 9.0 / 49.0),
    (SHEAR_DIAGONAL, SHEAR_DIAGONAL, 9.0 / 49.0),
    (-SHEAR_DIAGONAL, SHEAR_DIAGONAL, 9.0 / 49.0),
    (-SHEAR_AXIS, 0.0, 40.0 / 49.0),
    (SHEAR_AXIS, 0.0, 40.0 / 49.0),
    (0.0, -SHEAR_AXIS, 40.0 / 49.0),
    (0.0, SHEAR_AXIS, 40.0 / 49.0),
];
const SIDES4: [(usize, usize); 4] = [(0, 1), (1, 2), (2, 3), (3, 0)];
const SIDES3: [(usize, usize); 3] = [(0, 1), (1, 2), (2, 0)];
/// Interior 3-point triangle rule (each point weighs a third of the area), area coordinates (xi, eta).
const TRI_POINTS: [[f64; 2]; 3] = [
    [1.0 / 6.0, 1.0 / 6.0],
    [2.0 / 3.0, 1.0 / 6.0],
    [1.0 / 6.0, 2.0 / 3.0],
];
// ---------------------------------------------------------------- small dense algebra

/// Row-major dense matrix.
#[derive(Clone)]
struct Mat {
    r: usize,
    c: usize,
    a: Vec<f64>,
}

impl Mat {
    fn zeros(r: usize, c: usize) -> Mat {
        Mat {
            r,
            c,
            a: vec![0.0; r * c],
        }
    }
    fn at(&self, i: usize, j: usize) -> f64 {
        self.a[i * self.c + j]
    }
    fn add(&mut self, i: usize, j: usize, v: f64) {
        self.a[i * self.c + j] += v;
    }
    /// self += f * u u^T (u has length r = c).
    fn add_outer(&mut self, f: f64, u: &[f64]) {
        for i in 0..self.r {
            if u[i] == 0.0 {
                continue;
            }
            for j in 0..self.c {
                self.a[i * self.c + j] += f * u[i] * u[j];
            }
        }
    }
    fn block(&self, r0: usize, r1: usize, c0: usize, c1: usize) -> Mat {
        let mut m = Mat::zeros(r1 - r0, c1 - c0);
        for i in r0..r1 {
            for j in c0..c1 {
                m.a[(i - r0) * m.c + j - c0] = self.at(i, j);
            }
        }
        m
    }
    fn mul(&self, o: &Mat) -> Mat {
        let mut m = Mat::zeros(self.r, o.c);
        for i in 0..self.r {
            for k in 0..self.c {
                let v = self.at(i, k);
                if v == 0.0 {
                    continue;
                }
                for j in 0..o.c {
                    m.a[i * o.c + j] += v * o.at(k, j);
                }
            }
        }
        m
    }
    fn t(&self) -> Mat {
        let mut m = Mat::zeros(self.c, self.r);
        for i in 0..self.r {
            for j in 0..self.c {
                m.a[j * self.r + i] = self.at(i, j);
            }
        }
        m
    }
    /// Solves self X = B (Gaussian elimination with partial pivoting); self is square.
    fn solve(&self, b: &Mat) -> Mat {
        let n = self.r;
        let mut a = self.clone();
        let mut x = b.clone();
        for col in 0..n {
            let p = (col..n)
                .max_by(|&i, &j| a.at(i, col).abs().total_cmp(&a.at(j, col).abs()))
                .expect("non-empty");
            if p != col {
                for j in 0..n {
                    a.a.swap(col * n + j, p * n + j);
                }
                for j in 0..x.c {
                    x.a.swap(col * x.c + j, p * x.c + j);
                }
            }
            let piv = a.at(col, col);
            for i in col + 1..n {
                let f = a.at(i, col) / piv;
                if f == 0.0 {
                    continue;
                }
                for j in col..n {
                    a.a[i * n + j] -= f * a.at(col, j);
                }
                for j in 0..x.c {
                    x.a[i * x.c + j] -= f * x.at(col, j);
                }
            }
        }
        for col in (0..n).rev() {
            let piv = a.at(col, col);
            for j in 0..x.c {
                let mut v = x.at(col, j);
                for k in col + 1..n {
                    v -= a.at(col, k) * x.at(k, j);
                }
                x.a[col * x.c + j] = v / piv;
            }
        }
        x
    }
}

// ---------------------------------------------------------------- geometry helpers

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

/// Jacobian rows (x,xi y,xi), (x,eta y,eta); determinant; inverse (d/dx = inv[0] . (d/dxi, d/deta)).
fn jacobian(xy: &[[f64; 2]; 4], xi: f64, eta: f64) -> ([[f64; 2]; 2], f64, [[f64; 2]; 2]) {
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
    (j, det, inv)
}

fn cart(inv: &[[f64; 2]; 2], a: f64, b: f64) -> [f64; 2] {
    [inv[0][0] * a + inv[0][1] * b, inv[1][0] * a + inv[1][1] * b]
}

/// Hierarchical midside functions on sides (1,2), (2,3), (3,4), (4,1) and natural derivatives.
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

/// Side length, unit tangent s and normal n = (s_y, -s_x).
fn side(p: [f64; 2], q: [f64; 2]) -> (f64, [f64; 2], [f64; 2]) {
    let d = [q[0] - p[0], q[1] - p[1]];
    let l = d[0].hypot(d[1]);
    let s = [d[0] / l, d[1] / l];
    (l, s, [s[1], -s[0]])
}

/// Wilson's edge shear row (chord slope + mean tangential rotation + 2/3 of the tangential increment).
fn edge_row(
    n: usize,
    i: usize,
    j: usize,
    k: usize,
    first_inc: usize,
    l: f64,
    s: [f64; 2],
) -> Vec<f64> {
    let mut r = vec![0.0; n];
    r[3 * j] += 1.0 / l;
    r[3 * i] -= 1.0 / l;
    for a in [i, j] {
        r[3 * a + 2] += 0.5 * s[0];
        r[3 * a + 1] -= 0.5 * s[1];
    }
    r[first_inc + k] += 2.0 / 3.0;
    r
}

/// Rotations and their x, y derivatives (2 x n each, flattened as rows) at a point, given the nodal
/// shape functions with Cartesian derivatives and the midside functions with Cartesian derivatives.
struct Beta {
    b: [Vec<f64>; 2],
    bx: [Vec<f64>; 2],
    by: [Vec<f64>; 2],
}

fn beta_from(
    n: usize,
    nodal: &[(f64, [f64; 2])],
    mids: &[(f64, [f64; 2])],
    dirs: &[([f64; 2], [f64; 2])],
) -> Beta {
    let z = || vec![0.0; n];
    let mut b = [z(), z()];
    let mut bx = [z(), z()];
    let mut by = [z(), z()];
    for (a, (na, d)) in nodal.iter().enumerate() {
        b[0][3 * a + 2] = *na;
        b[1][3 * a + 1] = -na;
        bx[0][3 * a + 2] = d[0];
        bx[1][3 * a + 1] = -d[0];
        by[0][3 * a + 2] = d[1];
        by[1][3 * a + 1] = -d[1];
    }
    let m = mids.len();
    let first = 3 * nodal.len();
    for (k, ((h, dh), (s, nn))) in mids.iter().zip(dirs).enumerate() {
        for (col, d) in [(first + k, s), (first + m + k, nn)] {
            for r in 0..2 {
                b[r][col] = h * d[r];
                bx[r][col] = dh[0] * d[r];
                by[r][col] = dh[1] * d[r];
            }
        }
    }
    Beta { b, bx, by }
}

fn curvature(be: &Beta) -> [Vec<f64>; 3] {
    let n = be.b[0].len();
    [
        be.bx[0].clone(),
        be.by[1].clone(),
        (0..n).map(|q| be.by[0][q] + be.bx[1][q]).collect(),
    ]
}

/// Mean-corrects the increment columns (from `first`) of curvature operators at weighted points and
/// adds sum B^T D B w to `k`; returns the corrected operators.
fn bending_energy(
    k: &mut Mat,
    pts: Vec<([Vec<f64>; 3], f64)>,
    d: &[[f64; 3]; 3],
    first: usize,
) -> Vec<[Vec<f64>; 3]> {
    let n = k.r;
    let area: f64 = pts.iter().map(|(_, w)| w).sum();
    let mut mean = [vec![0.0; n], vec![0.0; n], vec![0.0; n]];
    for (c, w) in &pts {
        for r in 0..3 {
            for q in first..n {
                mean[r][q] += c[r][q] * w / area;
            }
        }
    }
    let mut out = Vec::new();
    for (mut c, w) in pts {
        for r in 0..3 {
            for q in first..n {
                c[r][q] -= mean[r][q];
            }
        }
        for i in 0..n {
            for j in 0..n {
                let mut v = 0.0;
                for r in 0..3 {
                    if c[r][i] == 0.0 {
                        continue;
                    }
                    for s in 0..3 {
                        v += c[r][i] * d[r][s] * c[s][j];
                    }
                }
                k.add(i, j, v * w);
            }
        }
        out.push(c);
    }
    out
}

/// What the force recovery needs from a thick plate.
pub struct ThickPlate {
    /// Plate stiffness on (w, rx, ry) per corner, 3n x 3n.
    pub k: Vec<f64>,
    n: usize,
    /// Recovery curvature operators (3 x (3n + internal)), sampled from the energy field.
    b_points: Vec<[Vec<f64>; 3]>,
    /// Internal increments from bending-part displacements: q = cond * u_b.
    cond: Mat,
    /// Shear deflection at corners 2..n from total displacements: w_s = shear_map * u.
    shear_map: Mat,
    /// Reported gradient (d/dx, d/dy) of the shear deflection per corner value.
    grad0: [Vec<f64>; 2],
    /// Bending stiffness and kappa G t for shear in the local 1 and 2 directions (times v13, v23).
    d: [[f64; 3]; 3],
    kgt: [f64; 2],
}

impl ThickPlate {
    /// Moments (M = -D kappa, positive = tension at the bottom) at the recovery points and the constant shears
    /// [V13, V23] = kappa G t grad(w_shear) from the plate displacements (w, rx, ry per corner).
    pub fn forces(&self, u: &[f64]) -> (Vec<[f64; 3]>, [f64; 2]) {
        let n = self.n;
        let ws: Vec<f64> = (0..n - 1)
            .map(|i| (0..3 * n).map(|j| self.shear_map.at(i, j) * u[j]).sum())
            .collect();
        let mut ub = u.to_vec();
        for (i, w) in ws.iter().enumerate() {
            ub[3 * (i + 1)] -= w;
        }
        let ni = self.cond.r;
        let q: Vec<f64> = (0..ni)
            .map(|i| (0..3 * n).map(|j| self.cond.at(i, j) * ub[j]).sum())
            .collect();
        let v: Vec<f64> = ub.iter().chain(q.iter()).copied().collect();
        let m = self
            .b_points
            .iter()
            .map(|b| {
                let kappa: [f64; 3] =
                    std::array::from_fn(|r| b[r].iter().zip(&v).map(|(x, y)| x * y).sum());
                std::array::from_fn(|r| -(0..3).map(|s| self.d[r][s] * kappa[s]).sum::<f64>())
            })
            .collect();
        let wsf: Vec<f64> = std::iter::once(0.0).chain(ws).collect();
        let vs = std::array::from_fn(|c| {
            self.kgt[c]
                * self.grad0[c]
                    .iter()
                    .zip(&wsf)
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
        });
        (m, vs)
    }
}

#[allow(clippy::too_many_arguments)] // Keep the shared triangle/quad series assembly unchanged.
fn assemble_parts(
    n: usize,
    kb: Mat,
    cond: Mat,
    ks: &Mat,
    b_points: Vec<[Vec<f64>; 3]>,
    grad0: [Vec<f64>; 2],
    d: [[f64; 3]; 3],
    kgt: [f64; 2],
) -> ThickPlate {
    let m = 3 * n;
    // Series: w = w_b + w_s, w_s(corner 1) = 0 (a uniform w_s is a rigid motion of both parts).
    let mut p = Mat::zeros(m, n - 1);
    for i in 0..n - 1 {
        p.a[3 * (i + 1) * (n - 1) + i] = 1.0;
    }
    let kbp = kb.mul(&p);
    let mut s = p.t().mul(&kbp);
    let mut shear_k = Mat::zeros(n - 1, n - 1);
    for i in 0..n - 1 {
        for j in 0..n - 1 {
            // `ks` is already times kappa G t.
            let value = ks.at(i + 1, j + 1);
            s.add(i, j, value);
            shear_k.add(i, j, value);
        }
    }
    let shear_map = s.solve(&kbp.t());
    // Evaluate the minimized bending + shear energy directly. Subtracting the
    // Schur correction from Kb cancels large entries in thick, long elements.
    // R = I - P S maps total displacements to the bending-part displacements.
    let mut bending_map = p.mul(&shear_map);
    for value in &mut bending_map.a {
        *value = -*value;
    }
    for i in 0..m {
        bending_map.add(i, i, 1.0);
    }
    let mut energy = bending_map.t().mul(&kb).mul(&bending_map);
    let shear_energy = shear_map.t().mul(&shear_k).mul(&shear_map);
    for (value, shear) in energy.a.iter_mut().zip(&shear_energy.a) {
        *value += shear;
    }
    ThickPlate {
        k: energy.a,
        n,
        b_points,
        cond,
        shear_map,
        grad0,
        d,
        kgt,
    }
}

fn trace(d: &[[f64; 3]; 3]) -> f64 {
    d[0][0] + d[1][1] + d[2][2]
}

// ---------------------------------------------------------------- quad

fn quad_beta(
    xy: &[[f64; 2]; 4],
    dirs: &[([f64; 2], [f64; 2]); 4],
    xi: f64,
    eta: f64,
) -> (Beta, f64) {
    let (n, dn) = bilinear(xi, eta);
    let (_, det, inv) = jacobian(xy, xi, eta);
    let (h, dh) = midside(xi, eta);
    let nodal: Vec<(f64, [f64; 2])> = (0..4)
        .map(|a| (n[a], cart(&inv, dn[0][a], dn[1][a])))
        .collect();
    let mids: Vec<(f64, [f64; 2])> = (0..4)
        .map(|k| (h[k], cart(&inv, dh[0][k], dh[1][k])))
        .collect();
    // Internal order: tangential4, normal4, bubble(x,y). The research reference
    // interleaves edge components; this permutation does not change the field.
    let mut be = beta_from(22, &nodal, &mids, dirs);
    let bubble = (1.0 - xi * xi) * (1.0 - eta * eta);
    let db = cart(
        &inv,
        -2.0 * xi * (1.0 - eta * eta),
        -2.0 * eta * (1.0 - xi * xi),
    );
    for r in 0..2 {
        be.b[r][20 + r] = bubble;
        be.bx[r][20 + r] = db[0];
        be.by[r][20 + r] = db[1];
    }
    (be, det)
}

/// Shear stiffness (4 x 4 on w) and the separately recovered shape gradients. `kgt`: kappa G t for
/// gradients along local 1 and 2 (they differ only with v13, v23 modifiers).
fn quad_shear(xy: &[[f64; 2]; 4], kgt: [f64; 2]) -> (Mat, [Vec<f64>; 2]) {
    let (_, dn) = bilinear(0.0, 0.0);
    let (_, det0, inv0) = jacobian(xy, 0.0, 0.0);
    let b: Vec<[f64; 2]> = (0..4).map(|a| cart(&inv0, dn[0][a], dn[1][a])).collect();
    let area = 4.0 * det0;
    let h = [1.0, -1.0, 1.0, -1.0];
    let hx: f64 = (0..4).map(|a| h[a] * xy[a][0]).sum();
    let hy: f64 = (0..4).map(|a| h[a] * xy[a][1]).sum();
    let g: Vec<f64> = (0..4).map(|a| h[a] - hx * b[a][0] - hy * b[a][1]).collect();
    // This degree-five square cubature reproduces the measured reference hourglass
    // coefficient; the exact integral of the distorted-quad energy density does not.
    let mut c = 0.0;
    for &(xi, eta, weight) in &SHEAR_POINTS {
        let (_, det, inv) = jacobian(xy, xi, eta);
        let gr = cart(&inv, eta, xi);
        c += (kgt[0] * gr[0] * gr[0] + kgt[1] * gr[1] * gr[1]) * det * weight / 16.0;
    }
    let mut k = Mat::zeros(4, 4);
    for i in 0..4 {
        for j in 0..4 {
            k.a[i * 4 + j] =
                area * (kgt[0] * b[i][0] * b[j][0] + kgt[1] * b[i][1] * b[j][1]) + c * g[i] * g[j];
        }
    }
    // Reported shear uses the mean physical gradient at the four diagonal cubature points.
    let mut grad = [vec![0.0; 4], vec![0.0; 4]];
    for xi in [-SHEAR_DIAGONAL, SHEAR_DIAGONAL] {
        for eta in [-SHEAR_DIAGONAL, SHEAR_DIAGONAL] {
            let (_, dn) = bilinear(xi, eta);
            let (_, _, inv) = jacobian(xy, xi, eta);
            for a in 0..4 {
                let derivative = cart(&inv, dn[0][a], dn[1][a]);
                for r in 0..2 {
                    grad[r][a] += 0.25 * derivative[r];
                }
            }
        }
    }
    (k, grad)
}

/// Q9 bending stiffness, internal stationary map, and recovery operators at the
/// usual 2x2 Gauss points. The caller's bilinear extrapolation then gives the
/// corners of the energy field sampled at the degree-five rule's diagonal points.
fn quad_bending(xy: &[[f64; 2]; 4], d: &[[f64; 3]; 3]) -> (Mat, Mat, Vec<[Vec<f64>; 3]>) {
    let sides: Vec<_> = SIDES4.iter().map(|&(i, j)| side(xy[i], xy[j])).collect();
    let dirs = std::array::from_fn(|e| (sides[e].1, sides[e].2));
    let fields: Vec<_> = SHEAR_POINTS
        .iter()
        .map(|&(xi, eta, w)| {
            let (be, det) = quad_beta(xy, &dirs, xi, eta);
            (be, det * w)
        })
        .collect();
    let mut h = Mat::zeros(22, 22);
    let pts = fields.iter().map(|(be, w)| (curvature(be), *w)).collect();
    let corrected = bending_energy(&mut h, pts, d, 12);
    for (be, w) in &fields {
        let curl: Vec<_> = (0..22).map(|q| be.bx[1][q] - be.by[0][q]).collect();
        h.add_outer(CURL_PENALTY * trace(d) * w, &curl);
    }
    // R_e = w_j-w_i + L/2 (beta_i+beta_j).s + 2L/3 t_e.
    // Three exact differences enforce equal integrated residuals, including
    // their common circulation. No penalty or projected curl is used.
    let mut edges = Mat::zeros(4, 22);
    for (e, &(i, j)) in SIDES4.iter().enumerate() {
        let (l, tangent, _) = sides[e];
        for (q, value) in edge_row(22, i, j, e, 12, l, tangent).iter().enumerate() {
            edges.add(e, q, l * value);
        }
    }
    let mut kkt = Mat::zeros(13, 13);
    let mut rhs = Mat::zeros(13, 12);
    for i in 0..10 {
        for j in 0..10 {
            kkt.add(i, j, 0.5 * (h.at(12 + i, 12 + j) + h.at(12 + j, 12 + i)));
        }
        for j in 0..12 {
            rhs.add(i, j, -0.5 * (h.at(12 + i, j) + h.at(j, 12 + i)));
        }
    }
    for e in 0..3 {
        for q in 0..22 {
            let value = edges.at(e, q) - edges.at(e + 1, q);
            if q < 12 {
                rhs.add(10 + e, q, -value);
            } else {
                kkt.add(10 + e, q - 12, value);
                kkt.add(q - 12, 10 + e, value);
            }
        }
    }
    let cond = kkt.solve(&rhs).block(0, 10, 0, 12);
    let mut stationary = Mat::zeros(22, 12);
    for q in 0..12 {
        stationary.add(q, q, 1.0);
    }
    for i in 0..10 {
        for q in 0..12 {
            stationary.add(12 + i, q, cond.at(i, q));
        }
    }
    // Direct stationary energy avoids subtracting large curl terms in a Schur
    // complement. Symmetrize only the final roundoff, as in the reference.
    let energy = stationary.t().mul(&h).mul(&stationary);
    let mut kb = Mat::zeros(12, 12);
    for i in 0..12 {
        for j in 0..12 {
            kb.add(i, j, 0.5 * (energy.at(i, j) + energy.at(j, i)));
        }
    }
    // Interpolation between the diagonal samples and the usual Gauss points:
    // N_a(xi/r,eta/r), r=sqrt(7/9). Extrapolating these with sqrt(3)
    // in shell.rs is precisely N_a(corner_x/r,corner_y/r).
    let b_points = GAUSS
        .iter()
        .map(|&[xi, eta]| {
            let (weights, _) = bilinear(xi / SHEAR_DIAGONAL, eta / SHEAR_DIAGONAL);
            std::array::from_fn(|r| {
                (0..22)
                    .map(|q| (0..4).map(|p| weights[p] * corrected[p][r][q]).sum())
                    .collect()
            })
        })
        .collect();
    (kb, cond, b_points)
}

/// Quad Shell-Thick plate part. `d` = bending stiffness (E t^3 / 12 plane stress), `kgt` = kappa G t for
/// shear along local 1 and 2 (equal unless the v13, v23 modifiers differ).
pub fn quad(xy: &[[f64; 2]; 4], d: &[[f64; 3]; 3], kgt: [f64; 2]) -> ThickPlate {
    let (kb, cond, b_points) = quad_bending(xy, d);
    let (ks, grad0) = quad_shear(xy, kgt);
    assemble_parts(4, kb, cond, &ks, b_points, grad0, *d, kgt)
}

// ---------------------------------------------------------------- triangle

/// Area and Cartesian derivatives of the area coordinates.
fn tri_geometry(xy: &[[f64; 2]; 3]) -> (f64, [[f64; 2]; 3]) {
    let j = [
        [xy[1][0] - xy[0][0], xy[1][1] - xy[0][1]],
        [xy[2][0] - xy[0][0], xy[2][1] - xy[0][1]],
    ];
    let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
    let inv = [
        [j[1][1] / det, -j[0][1] / det],
        [-j[1][0] / det, j[0][0] / det],
    ];
    let dl_nat = [[-1.0, 1.0, 0.0], [-1.0, 0.0, 1.0]];
    let dl = std::array::from_fn(|a| cart(&inv, dl_nat[0][a], dl_nat[1][a]));
    (det / 2.0, dl)
}

fn tri_beta(xy: &[[f64; 2]; 3], dirs: &[([f64; 2], [f64; 2]); 3], l: [f64; 3]) -> Beta {
    let (_, dl) = tri_geometry(xy);
    let nodal: Vec<(f64, [f64; 2])> = (0..3).map(|a| (l[a], dl[a])).collect();
    let mids: Vec<(f64, [f64; 2])> = SIDES3
        .iter()
        .map(|&(i, j)| {
            (
                4.0 * l[i] * l[j],
                [
                    4.0 * (dl[i][0] * l[j] + l[i] * dl[j][0]),
                    4.0 * (dl[i][1] * l[j] + l[i] * dl[j][1]),
                ],
            )
        })
        .collect();
    beta_from(15, &nodal, &mids, dirs)
}

/// Triangle Shell-Thick plate part (recovery at the corners).
pub fn tri(xy: &[[f64; 2]; 3], d: &[[f64; 3]; 3], kgt: [f64; 2]) -> ThickPlate {
    let trd = trace(d);
    let (area, dl) = tri_geometry(xy);
    let sides: Vec<(f64, [f64; 2], [f64; 2])> =
        SIDES3.iter().map(|&(i, j)| side(xy[i], xy[j])).collect();
    let dirs: [([f64; 2], [f64; 2]); 3] = std::array::from_fn(|k| (sides[k].1, sides[k].2));
    let lc = |p: &[f64; 2]| [1.0 - p[0] - p[1], p[0], p[1]];
    let mut k = Mat::zeros(15, 15);
    let pts: Vec<([Vec<f64>; 3], f64)> = TRI_POINTS
        .iter()
        .map(|p| (curvature(&tri_beta(xy, &dirs, lc(p))), area / 3.0))
        .collect();
    // Mean of the increment curvatures (the same correction is applied at the corners).
    let mut mean = [vec![0.0; 15], vec![0.0; 15], vec![0.0; 15]];
    for (c, _) in &pts {
        for r in 0..3 {
            for q in 9..15 {
                mean[r][q] += c[r][q] / 3.0;
            }
        }
    }
    bending_energy(&mut k, pts, d, 9);
    // Full curl, including its mean. Stokes gives integral(curl beta)=sum(R_e),
    // R_e=L_e*gamma_e. With R_e=rho its mean energy is 9000 trD/A * rho^2.
    for p in &TRI_POINTS {
        let be = tri_beta(xy, &dirs, lc(p));
        let curl: Vec<f64> = (0..15).map(|q| be.bx[1][q] - be.by[0][q]).collect();
        k.add_outer(CURL_PENALTY * trd * area / 3.0, &curl);
    }
    // Exact reduction: keep external9, common circulation rho, and normal3.
    // t_e=(rho-R_external,e*u)/((2/3)L_e), hence all R_e=rho identically.
    // There is no separate edge penalty and no large approximate constraint.
    let mut reduction = Mat::zeros(15, 13);
    for q in 0..9 {
        reduction.add(q, q, 1.0);
    }
    for (e, &(i, j)) in SIDES3.iter().enumerate() {
        let (l, s, _) = sides[e];
        let row = edge_row(15, i, j, e, 9, l, s);
        for (q, value) in row[..9].iter().enumerate() {
            reduction.add(9 + e, q, -1.5 * value);
        }
        reduction.add(9 + e, 9, 1.5 / l);
        reduction.add(12 + e, 10 + e, 1.0);
    }
    let reduced = reduction.t().mul(&k).mul(&reduction);
    // Recovery at the corners (linear curvature field), corrected like the energy.
    let b_points = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        .iter()
        .map(|l| {
            let mut c = curvature(&tri_beta(xy, &dirs, *l));
            for r in 0..3 {
                for q in 9..15 {
                    c[r][q] -= mean[r][q];
                }
            }
            let mut full = Mat::zeros(3, 15);
            for r in 0..3 {
                for q in 0..15 {
                    full.add(r, q, c[r][q]);
                }
            }
            let mapped = full.mul(&reduction);
            std::array::from_fn(|r| (0..13).map(|q| mapped.at(r, q)).collect())
        })
        .collect();
    let mut ks = Mat::zeros(3, 3);
    for i in 0..3 {
        for j in 0..3 {
            ks.a[i * 3 + j] = area * (kgt[0] * dl[i][0] * dl[j][0] + kgt[1] * dl[i][1] * dl[j][1]);
        }
    }
    let grad0 = [
        dl.iter().map(|v| v[0]).collect(),
        dl.iter().map(|v| v[1]).collect(),
    ];
    let mut cond = reduced
        .block(9, 13, 9, 13)
        .solve(&reduced.block(9, 13, 0, 9));
    for value in &mut cond.a {
        *value = -*value;
    }
    // Evaluate minimized bending energy directly instead of subtracting its Schur term.
    let mut stationary = Mat::zeros(13, 9);
    for q in 0..9 {
        stationary.add(q, q, 1.0);
    }
    for i in 0..4 {
        for q in 0..9 {
            stationary.add(9 + i, q, cond.at(i, q));
        }
    }
    let kb = stationary.t().mul(&reduced).mul(&stationary);
    assemble_parts(3, kb, cond, &ks, b_points, grad0, *d, kgt)
}

/// kappa G t of an isotropic material.
pub fn shear_rigidity(e: f64, nu: f64, t: f64) -> f64 {
    KAPPA * e / (2.0 * (1.0 + nu)) * t
}

#[cfg(test)]
mod triangle_compatibility_tests {
    use super::*;

    const D: [[f64; 3]; 3] = [[2.0, 0.4, 0.0], [0.4, 2.0, 0.0], [0.0, 0.0, 0.8]];

    #[test]
    fn triangle_compatibility_patches_covariance_and_positive_modes() {
        for xy in [
            [[0.0, 0.0], [2.0, 0.0], [0.0, 1.5]],
            [[0.0, 0.0], [2.0, 0.0], [1.0, 1.732]],
        ] {
            let reference = tri(&xy, &D, [7.0; 2]);
            for mode in 0..6 {
                let mut u = Vec::new();
                for [x, y] in xy {
                    u.extend(match mode {
                        0 => [1.0, 0.0, 0.0],
                        1 => [y, 1.0, 0.0],
                        2 => [-x, 0.0, 1.0],
                        3 => [0.5 * x * x, 0.0, -x],
                        4 => [0.5 * y * y, y, 0.0],
                        _ => [x * y, x, -y],
                    });
                }
                let expected = match mode {
                    3 => [2.0, 0.4, 0.0],
                    4 => [0.4, 2.0, 0.0],
                    5 => [0.0, 0.0, 1.6],
                    _ => [0.0; 3],
                };
                let (moments, shear) = reference.forces(&u);
                for a in 0..3 {
                    // Constant-moment boundary virtual work, independent of the interpolation.
                    let previous = xy[(a + 2) % 3];
                    let next = xy[(a + 1) % 3];
                    let gx = 0.5 * (next[1] - previous[1]);
                    let gy = 0.5 * (previous[0] - next[0]);
                    let force = [
                        0.0,
                        gy * expected[1] + gx * expected[2],
                        -gx * expected[0] - gy * expected[2],
                    ];
                    for r in 0..3 {
                        let actual: f64 = (0..9)
                            .map(|q| reference.k[(3 * a + r) * 9 + q] * u[q])
                            .sum();
                        assert!(
                            (actual - force[r]).abs() < 2e-9,
                            "triangle force patch mode {mode}"
                        );
                        assert!(
                            (moments[a][r] - expected[r]).abs() < 2e-9,
                            "triangle moment patch mode {mode}"
                        );
                    }
                }
                assert!(shear.iter().all(|v| v.abs() < 2e-9), "triangle patch shear");
            }
            let peak = reference.k.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
            // A non-patch field exercises the condensed recovery and shear state.
            let u: Vec<f64> = (0..9)
                .map(|q| 0.03 * ((q * q + 3 * q + 1) % 11) as f64 - 0.1)
                .collect();
            let (reference_m, reference_v) = reference.forces(&u);
            let moment_peak = reference_m
                .iter()
                .flatten()
                .fold(0.0_f64, |m, v| m.max(v.abs()));
            for shift in 0..3 {
                for angle in [0.0_f64, 0.61] {
                    for scale in [0.1_f64, 1.0, 10.0] {
                        let (sin, cos) = angle.sin_cos();
                        let moved = std::array::from_fn(|a| {
                            let p = xy[(a + shift) % 3];
                            [
                                3.0 + scale * (cos * p[0] - sin * p[1]),
                                -4.0 + scale * (sin * p[0] + cos * p[1]),
                            ]
                        });
                        let d = D.map(|row| row.map(|v| v * scale.powi(3)));
                        let plate = tri(&moved, &d, [7.0 * scale; 2]);
                        let g = [[scale, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]];
                        let mut moved_u = vec![0.0; 9];
                        for a in 0..3 {
                            for r in 0..3 {
                                for t in 0..3 {
                                    moved_u[3 * a + r] += g[r][t] * u[3 * ((a + shift) % 3) + t];
                                }
                            }
                        }
                        let (moments, shear) = plate.forces(&moved_u);
                        for (a, moment) in moments.iter().enumerate() {
                            let m = reference_m[(a + shift) % 3];
                            let expected = [
                                cos * cos * m[0] + sin * sin * m[1] - 2.0 * cos * sin * m[2],
                                sin * sin * m[0] + cos * cos * m[1] + 2.0 * cos * sin * m[2],
                                cos * sin * (m[0] - m[1]) + (cos * cos - sin * sin) * m[2],
                            ];
                            for r in 0..3 {
                                assert!((moment[r]/scale.powi(2)-expected[r]).abs() < 1e-9*moment_peak,
                                    "triangle moment covariance shift{shift}, angle{angle}, scale{scale}");
                            }
                        }
                        let expected_v = [
                            cos * reference_v[0] - sin * reference_v[1],
                            sin * reference_v[0] + cos * reference_v[1],
                        ];
                        for r in 0..2 {
                            assert!((shear[r]/scale-expected_v[r]).abs() < 1e-9*(1.0+expected_v[r].abs()),
                                "triangle shear covariance shift{shift}, angle{angle}, scale{scale}");
                        }
                        let mut transformed = [0.0_f64; 81];
                        for a in 0..3 {
                            for b in 0..3 {
                                for i in 0..3 {
                                    for j in 0..3 {
                                        for r in 0..3 {
                                            for t in 0..3 {
                                                transformed[(3 * ((a + shift) % 3) + i) * 9
                                                    + 3 * ((b + shift) % 3)
                                                    + j] += g[r][i]
                                                    * plate.k[(3 * a + r) * 9 + 3 * b + t]
                                                    * g[t][j]
                                                    / scale.powi(3);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let error = reference
                            .k
                            .iter()
                            .zip(transformed)
                            .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
                        assert!(
                            error / peak < 1e-9,
                            "triangle covariance shift{shift}, angle{angle}, scale{scale}"
                        );
                        let asymmetry = (0..9)
                            .flat_map(|i| (0..9).map(move |j| (i, j)))
                            .fold(0.0_f64, |m, (i, j)| {
                                m.max((plate.k[i * 9 + j] - plate.k[j * 9 + i]).abs())
                            });
                        let moved_peak = plate.k.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
                        assert!(asymmetry / moved_peak < 1e-11, "triangle symmetry");
                        // Cholesky on the free block checks all six deformational modes.
                        let mut lower = [[0.0_f64; 6]; 6];
                        for i in 0..6 {
                            for j in 0..=i {
                                let mut value = 0.5
                                    * (plate.k[(i + 3) * 9 + j + 3] + plate.k[(j + 3) * 9 + i + 3]);
                                for p in 0..j {
                                    value -= lower[i][p] * lower[j][p];
                                }
                                if i == j {
                                    assert!(value > 0.0, "triangle positive mode {i}");
                                    lower[i][j] = value.sqrt();
                                } else {
                                    lower[i][j] = value / lower[j][j];
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}


#[cfg(test)]
mod quad_compatibility_tests {
    use super::*;

    const D: [[f64; 3]; 3] = [[2.0, 0.4, 0.0], [0.4, 2.0, 0.0], [0.0, 0.0, 0.8]];

    #[test]
    fn q9_preserves_rigid_and_constant_curvature_force_patches() {
        for shape in [
            [[0.0, 0.0], [2.0, 0.0], [2.003, 1.0], [0.003, 1.0]],
            [[0.0, 0.0], [2.0, 0.0], [1.998, 1.0], [0.002, 1.0]],
            [[0.0, 0.0], [2.0, 0.0], [2.001, 1.002], [0.0, 1.0]],
            [[0.0, 0.0], [2.0, 0.0], [2.6, 1.0], [0.6, 1.0]],
            [[0.0, 0.0], [2.0, 0.0], [3.4, 1.0], [1.4, 1.0]],
            [[0.0, 0.0], [2.0, 0.0], [2.603, 1.0], [0.6, 1.0]],
        ] {
            for angle in [0.0_f64, 0.61] {
                for scale in [0.1, 1.0, 10.0] {
                    let xy = shape.map(|p| {
                        [
                            3.0 + scale * (angle.cos() * p[0] - angle.sin() * p[1]),
                            -4.0 + scale * (angle.sin() * p[0] + angle.cos() * p[1]),
                        ]
                    });
                    let plate = quad(&xy, &D, [7.0; 2]);
                    for mode in 0..6 {
                        let mut u = Vec::new();
                        for [x, y] in xy {
                            u.extend(match mode {
                                0 => [1.0, 0.0, 0.0],
                                1 => [y, 1.0, 0.0],
                                2 => [-x, 0.0, 1.0],
                                3 => [0.5 * x * x, 0.0, -x],
                                4 => [0.5 * y * y, y, 0.0],
                                _ => [x * y, x, -y],
                            });
                        }
                        let expected = match mode {
                            3 => [2.0, 0.4, 0.0],
                            4 => [0.4, 2.0, 0.0],
                            5 => [0.0, 0.0, 1.6],
                            _ => [0.0; 3],
                        };
                        let (moments, shear) = plate.forces(&u);
                        for a in 0..4 {
                            let prev = xy[(a + 3) % 4];
                            let next = xy[(a + 1) % 4];
                            // Boundary integral of grad(N_a), independent of the element's
                            // internal condensation; constant moment virtual work.
                            let gx = 0.5 * (next[1] - prev[1]);
                            let gy = 0.5 * (prev[0] - next[0]);
                            let force = [
                                0.0,
                                gy * expected[1] + gx * expected[2],
                                -gx * expected[0] - gy * expected[2],
                            ];
                            for r in 0..3 {
                                let actual: f64 =
                                    (0..12).map(|q| plate.k[(3 * a + r) * 12 + q] * u[q]).sum();
                                assert!(
                                    (actual - force[r]).abs() < 2e-8 * (1.0 + force[r].abs()),
                                    "quadratic force patch: {actual} vs {}",
                                    force[r]
                                );
                            }
                        }
                        for m in moments {
                            for c in 0..3 {
                                assert!((m[c]-expected[c]).abs()<2e-8,
                                    "mode {mode}, scale {scale}, angle {angle}: {m:?} vs {expected:?}");
                            }
                        }
                        assert!(
                            shear.iter().all(|v| v.abs() < 2e-8),
                            "patch shear: {shear:?}"
                        );
                    }
                }
            }
        }
    }

    fn closure_quad(skew: f64, closure: f64) -> [[f64; 2]; 4] {
        [
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0 + skew + closure, 1.0],
            [skew, 1.0],
        ]
    }

    fn closure_quad_moment_operator(skew: f64, closure: f64) -> Vec<f64> {
        let plate = quad(&closure_quad(skew, closure), &D, [7.0; 2]);
        let mut operator = Vec::new();
        for col in 0..12 {
            let mut u = vec![0.0; 12];
            u[col] = 1.0;
            operator.extend(plate.forces(&u).0.into_iter().flatten());
        }
        operator
    }

    #[test]
    fn q9_stiffness_and_moments_are_continuous_at_parallel_edges() {
        // Perturb actual closure about both a rectangle and a substantial oblique quad.
        for skew in [0.0, 0.6] {
            let reference_k = quad(&closure_quad(skew, 0.0), &D, [7.0; 2]).k;
            let reference_m = closure_quad_moment_operator(skew, 0.0);
            for closure in [-1e-6, 1e-6] {
                let near_k = quad(&closure_quad(skew, closure), &D, [7.0; 2]).k;
                let half_k = quad(&closure_quad(skew, closure / 2.0), &D, [7.0; 2]).k;
                let near_m = closure_quad_moment_operator(skew, closure);
                let half_m = closure_quad_moment_operator(skew, closure / 2.0);
                // The change from exactly parallel to closure 1e-6 must shrink
                // linearly when the perturbation is halved, without a jump.
                for (name, reference, half, near) in [
                    ("stiffness", &reference_k, &half_k, &near_k),
                    ("moments", &reference_m, &half_m, &near_m),
                ] {
                    let peak = reference.iter().fold(0.0_f64, |p, v| p.max(v.abs()));
                    let remainder = reference
                        .iter()
                        .zip(half)
                        .zip(near)
                        .fold(0.0_f64, |p, ((r, h), n)| {
                            p.max(((n - r) - 2.0 * (h - r)).abs())
                        });
                    assert!(
                        remainder / peak < 1e-9,
                        "parallel {name}, closure {closure}"
                    );
                }
            }
            for centre in [-0.003, -1e-6, 0.0, 1e-6, 0.003] {
                let reference = closure_quad_moment_operator(skew, centre);
                let peak = reference.iter().fold(0.0_f64, |p, v| p.max(v.abs()));
                for h in [-1e-8, 1e-8] {
                    let moved = closure_quad_moment_operator(skew, centre + h);
                    let error = reference
                        .iter()
                        .zip(moved)
                        .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
                    assert!(
                        error / peak < 1e-6,
                        "Q9 recovery at {centre}, delta {h}: {}",
                        error / peak
                    );
                    let stiffness = |s| quad(&closure_quad(skew, s), &D, [7.0; 2]).k;
                    let reference_k = stiffness(centre);
                    let moved_k = stiffness(centre + h);
                    let peak_k = reference_k.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
                    let error_k = reference_k
                        .iter()
                        .zip(moved_k)
                        .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
                    assert!(
                        error_k / peak_k < 1e-6,
                        "Q9 stiffness at {centre}, delta {h}"
                    );
                }
            }
        }
    }

    #[test]
    fn q9_nine_modes_are_positive_symmetric_and_covariant() {
        for s in [0.0, 0.6, 1.4] {
            for closure in [0.0, 1e-6, 0.003, 0.02, -0.4, 0.4] {
                let xy = closure_quad(s, closure);
                let reference_plate = quad(&xy, &D, [7.0; 2]);
                let reference = reference_plate.k.clone();
                let u: Vec<f64> = (0..12)
                    .map(|q| 0.03 * ((q * q + 3 * q + 1) % 11) as f64 - 0.1)
                    .collect();
                let (reference_m, reference_v) = reference_plate.forces(&u);
                let peak = reference.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
                for shift in 0..4 {
                    for angle in [0.0_f64, 0.61] {
                        for scale in [0.1_f64, 1.0, 10.0] {
                            let (sin, cos) = angle.sin_cos();
                            let moved = std::array::from_fn(|a| {
                                let p = xy[(a + shift) % 4];
                                [
                                    3.0 + scale * (cos * p[0] - sin * p[1]),
                                    -4.0 + scale * (sin * p[0] + cos * p[1]),
                                ]
                            });
                            // Scale geometry and thickness together: D~L^3, kGt~L.
                            let d = D.map(|row| row.map(|v| v * scale.powi(3)));
                            let plate = quad(&moved, &d, [7.0 * scale; 2]);
                            let k = plate.k.clone();
                            let g = [[scale, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]];
                            let mut moved_u = vec![0.0; 12];
                            for a in 0..4 {
                                for r in 0..3 {
                                    for t in 0..3 {
                                        moved_u[3 * a + r] +=
                                            g[r][t] * u[3 * ((a + shift) % 4) + t];
                                    }
                                }
                            }
                            let (moments, shear) = plate.forces(&moved_u);
                            for (p, m) in moments.iter().enumerate() {
                                let reference = reference_m[(p + shift) % 4];
                                let expected = [
                                    cos * cos * reference[0] + sin * sin * reference[1]
                                        - 2.0 * cos * sin * reference[2],
                                    sin * sin * reference[0]
                                        + cos * cos * reference[1]
                                        + 2.0 * cos * sin * reference[2],
                                    cos * sin * (reference[0] - reference[1])
                                        + (cos * cos - sin * sin) * reference[2],
                                ];
                                let peak_m = reference_m
                                    .iter()
                                    .flatten()
                                    .fold(0.0_f64, |m, v| m.max(v.abs()));
                                for r in 0..3 {
                                    assert!((m[r]/scale.powi(2)-expected[r]).abs()<1e-8*peak_m,
                                "moment covariance s={s}, closure={closure},shift={shift},angle={angle},scale={scale}");
                                }
                            }
                            let expected_v = [
                                cos * reference_v[0] - sin * reference_v[1],
                                sin * reference_v[0] + cos * reference_v[1],
                            ];
                            for r in 0..2 {
                                assert!(
                                    (shear[r] / scale - expected_v[r]).abs()
                                        < 1e-8 * (1.0 + expected_v[r].abs())
                                );
                            }
                            let mut transformed = vec![0.0; 144];
                            for a in 0..4 {
                                for b in 0..4 {
                                    for i in 0..3 {
                                        for j in 0..3 {
                                            for r in 0..3 {
                                                for t in 0..3 {
                                                    transformed[(3 * ((a + shift) % 4) + i)
                                                        * 12
                                                        + 3 * ((b + shift) % 4)
                                                        + j] += g[r][i]
                                                        * k[(3 * a + r) * 12 + 3 * b + t]
                                                        * g[t][j]
                                                        / scale.powi(3);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            let error = reference
                                .iter()
                                .zip(&transformed)
                                .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
                            assert!(
                                error / peak < 1e-9,
                                "covariance s={s}, shift={shift}, angle={angle}, scale={scale}: {}",
                                error / peak
                            );
                            let mut l = [[0.0; 9]; 9];
                            for i in 0..9 {
                                for j in 0..=i {
                                    let mut value =
                                        0.5 * (k[(i + 3) * 12 + j + 3] + k[(j + 3) * 12 + i + 3]);
                                    for p in 0..j {
                                        value -= l[i][p] * l[j][p];
                                    }
                                    if i == j {
                                        assert!(value > 0.0, "positive modes s={s}");
                                        l[i][j] = value.sqrt();
                                    } else {
                                        l[i][j] = value / l[j][j];
                                    }
                                }
                            }
                            let asymmetry = (0..12)
                                .flat_map(|i| (0..12).map(move |j| (i, j)))
                                .fold(0.0_f64, |m, (i, j)| {
                                    m.max((k[i * 12 + j] - k[j * 12 + i]).abs())
                                });
                            let peak_k = k.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
                            assert!(asymmetry / peak_k < 1e-11, "symmetry s={s}");
                        }
                    }
                }
            }
        }
    }
}

