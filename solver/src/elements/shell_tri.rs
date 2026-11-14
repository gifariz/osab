//! Flat 3-node shell element (Shell-Thin and Membrane triangles). Found by the
//! triangle probe (checks/tri_probe.py); it matches the reference results
//! to round-off on single elements.
//!
//! - Membrane: linear triangle + Allman drilling field (midside normal offsets (l/8)(rz_j - rz_i) on the
//!   hierarchical functions 4 L_i L_j), a condensed cubic bubble 27 L1 L2 L3 in u and v, 3-point
//!   integration, mean correction of the drilling strain columns, a one-point penalty 0.4 G t A at the
//!   centroid tying the drilling rotation to the in-plane rotation, and 5e-5 G t A (rz1 - rz2)^2: the
//!   same parts as the quad (shell.rs), with the quad's alternating term as if corner 4 sat on
//!   corner 3.
//! - Thin-plate bending: DKT (Batoz, Bathe and Ho 1980), 3-point integration (exact).
//! - Forces (all fields linear): F11 F22 F12 from the corrected membrane strains (no bubble, no
//!   penalty), M11 M22 M12 from DKT at the corners, V13 V23 constant from the moment gradient.
//!
//! Area coordinates L1, L2, L3; natural coordinates xi = L2, eta = L3. Local DOFs per corner: u, v, w,
//! rx, ry, rz.

pub type Xy3 = [[f64; 2]; 3];

/// Interior 3-point rule (each point weighs a third of the area).
const POINTS: [[f64; 2]; 3] = [
    [1.0 / 6.0, 1.0 / 6.0],
    [2.0 / 3.0, 1.0 / 6.0],
    [1.0 / 6.0, 2.0 / 3.0],
];
const CORNERS: [[f64; 2]; 3] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
/// Side k joins corners SIDES[k] (counterclockwise).
const SIDES: [(usize, usize); 3] = [(0, 1), (1, 2), (2, 0)];
const DRILLING_FACTOR: f64 = 0.4;
const ALTERNATING_FACTOR: f64 = 5.0e-5;

/// Natural derivatives of L1, L2, L3: rows d/dxi, d/deta.
const DL_NAT: [[f64; 3]; 2] = [[-1.0, 1.0, 0.0], [-1.0, 0.0, 1.0]];

/// Area and the Cartesian derivatives of the area coordinates (rows d/dx, d/dy; constant).
pub(crate) fn geometry(xy: &Xy3) -> (f64, [[f64; 3]; 2]) {
    let j = [
        [xy[1][0] - xy[0][0], xy[1][1] - xy[0][1]],
        [xy[2][0] - xy[0][0], xy[2][1] - xy[0][1]],
    ];
    let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
    let inv = [
        [j[1][1] / det, -j[0][1] / det],
        [-j[1][0] / det, j[0][0] / det],
    ];
    let mut dl = [[0.0; 3]; 2];
    for a in 0..3 {
        dl[0][a] = inv[0][0] * DL_NAT[0][a] + inv[0][1] * DL_NAT[1][a];
        dl[1][a] = inv[1][0] * DL_NAT[0][a] + inv[1][1] * DL_NAT[1][a];
    }
    (det / 2.0, dl)
}

fn area_coords(xi: f64, eta: f64) -> [f64; 3] {
    [1.0 - xi - eta, xi, eta]
}

/// Strain operator (3 x 11: u, v, rz per corner, then the bubble in u and v) and the rotation
/// constraint row (continuum rotation minus interpolated drilling rotation) at one point.
fn membrane_fields(xy: &Xy3, xi: f64, eta: f64) -> ([[f64; 11]; 3], [f64; 11]) {
    let l = area_coords(xi, eta);
    let (_, dl) = geometry(xy);
    let mut b = [[0.0; 11]; 3];
    let mut rot = [0.0; 11];
    for a in 0..3 {
        b[0][3 * a] = dl[0][a];
        b[1][3 * a + 1] = dl[1][a];
        b[2][3 * a] = dl[1][a];
        b[2][3 * a + 1] = dl[0][a];
        rot[3 * a] = -0.5 * dl[1][a];
        rot[3 * a + 1] = 0.5 * dl[0][a];
    }
    for &(i, j) in &SIDES {
        // d(4 L_i L_j)/dx, /dy; midside offset (l/8)(r_j - r_i) n with l n = (y_j - y_i, x_i - x_j).
        let dm = [
            4.0 * (dl[0][i] * l[j] + l[i] * dl[0][j]),
            4.0 * (dl[1][i] * l[j] + l[i] * dl[1][j]),
        ];
        let cx = (xy[j][1] - xy[i][1]) / 8.0;
        let cy = (xy[i][0] - xy[j][0]) / 8.0;
        for (node, sgn) in [(j, 1.0), (i, -1.0)] {
            let c = 3 * node + 2;
            b[0][c] += sgn * cx * dm[0];
            b[1][c] += sgn * cy * dm[1];
            b[2][c] += sgn * (cx * dm[1] + cy * dm[0]);
            rot[c] += sgn * 0.5 * (cy * dm[0] - cx * dm[1]);
        }
    }
    for a in 0..3 {
        rot[3 * a + 2] -= l[a];
    }
    // Cubic bubble 27 L1 L2 L3 in u (column 9) and v (column 10).
    let g: [f64; 2] = std::array::from_fn(|d| {
        27.0 * (dl[d][0] * l[1] * l[2] + l[0] * dl[d][1] * l[2] + l[0] * l[1] * dl[d][2])
    });
    b[0][9] = g[0];
    b[2][9] = g[1];
    b[1][10] = g[1];
    b[2][10] = g[0];
    rot[9] = -0.5 * g[1];
    rot[10] = 0.5 * g[0];
    (b, rot)
}

/// Membrane stiffness (9 x 9, DOFs u, v, rz per corner) and the area mean of the drilling strain
/// columns (3 strains x 3 corners), which is subtracted from them everywhere.
pub fn membrane(xy: &Xy3, c: &[[f64; 3]; 3], g: f64, t: f64) -> ([[f64; 9]; 9], [[f64; 3]; 3]) {
    let (area, _) = geometry(xy);
    let mut pts: Vec<[[f64; 11]; 3]> = POINTS
        .iter()
        .map(|p| membrane_fields(xy, p[0], p[1]).0)
        .collect();
    let mut mean = [[0.0; 3]; 3];
    for (k, col) in [2, 5, 8].into_iter().enumerate() {
        for r in 0..3 {
            mean[r][k] = pts.iter().map(|b| b[r][col]).sum::<f64>() / 3.0;
            for b in pts.iter_mut() {
                b[r][col] -= mean[r][k];
            }
        }
    }
    let mut k = [[0.0; 11]; 11];
    for b in &pts {
        let cb: [[f64; 11]; 3] = std::array::from_fn(|r| {
            std::array::from_fn(|q| (0..3).map(|s| c[r][s] * b[s][q]).sum())
        });
        for p in 0..11 {
            for q in 0..11 {
                k[p][q] += (0..3).map(|r| b[r][p] * cb[r][q]).sum::<f64>() * area / 3.0;
            }
        }
    }
    let (_, b0) = membrane_fields(xy, 1.0 / 3.0, 1.0 / 3.0);
    let pen = DRILLING_FACTOR * g * t * area;
    for p in 0..11 {
        for q in 0..11 {
            k[p][q] += pen * b0[p] * b0[q];
        }
    }
    let alt = ALTERNATING_FACTOR * g * t * area;
    for (i, hi) in [(2, 1.0), (5, -1.0)] {
        for (j, hj) in [(2, 1.0), (5, -1.0)] {
            k[i][j] += alt * hi * hj;
        }
    }
    // Condense the bubble.
    let (a, bb, cc) = (k[9][9], k[9][10], k[10][10]);
    let det = a * cc - bb * bb;
    let kinv = [[cc / det, -bb / det], [-bb / det, a / det]];
    let mut out = [[0.0; 9]; 9];
    for p in 0..9 {
        for q in 0..9 {
            let mut v = k[p][q];
            for r in 0..2 {
                for s in 0..2 {
                    v -= k[p][9 + r] * kinv[r][s] * k[9 + s][q];
                }
            }
            out[p][q] = v;
        }
    }
    (out, mean)
}

/// Corrected membrane strain operator (3 x 9) at a point: no bubble, drilling columns minus their mean.
fn membrane_strain(xy: &Xy3, mean: &[[f64; 3]; 3], xi: f64, eta: f64) -> [[f64; 9]; 3] {
    let (b, _) = membrane_fields(xy, xi, eta);
    std::array::from_fn(|r| {
        std::array::from_fn(|q| {
            let drill = [2, 5, 8].iter().position(|&c| c == q);
            b[r][q] - drill.map_or(0.0, |k| mean[r][k])
        })
    })
}

// ---------------------------------------------------------------- DKT

/// 6-node triangle functions (corners 0-2, midsides 3-5 on SIDES): values or one natural derivative.
fn quad6(xi: f64, eta: f64, which: usize) -> [f64; 6] {
    let l = area_coords(xi, eta);
    let dl = |a: usize| {
        if which == 0 {
            0.0
        } else {
            DL_NAT[which - 1][a]
        }
    };
    let mut out = [0.0; 6];
    for a in 0..3 {
        out[a] = if which == 0 {
            l[a] * (2.0 * l[a] - 1.0)
        } else {
            (4.0 * l[a] - 1.0) * dl(a)
        };
    }
    for (k, &(i, j)) in SIDES.iter().enumerate() {
        out[3 + k] = if which == 0 {
            4.0 * l[i] * l[j]
        } else {
            4.0 * (dl(i) * l[j] + l[i] * dl(j))
        };
    }
    out
}

/// beta_x = Hx . U and beta_y = Hy . U (U = w, rx, ry per corner); the DKQ side coefficients
/// (shell.rs) with the 6-node triangle functions.
fn dkt_h(xy: &Xy3, n: &[f64; 6]) -> ([f64; 9], [f64; 9]) {
    let mut co = [[0.0; 5]; 3];
    for (k, &(i, j)) in SIDES.iter().enumerate() {
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
    let mut hx = [0.0; 9];
    let mut hy = [0.0; 9];
    for a in 0..3 {
        let (k, m) = (a, (a + 2) % 3);
        let (nk, nm) = (n[3 + k], n[3 + m]);
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

/// Curvature operator [beta_x,x; beta_y,y; beta_x,y + beta_y,x] (3 x 9) at a point.
fn dkt_b(xy: &Xy3, xi: f64, eta: f64) -> [[f64; 9]; 3] {
    let j = [
        [xy[1][0] - xy[0][0], xy[1][1] - xy[0][1]],
        [xy[2][0] - xy[0][0], xy[2][1] - xy[0][1]],
    ];
    let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
    let inv = [
        [j[1][1] / det, -j[0][1] / det],
        [-j[1][0] / det, j[0][0] / det],
    ];
    let (hxi, hyi) = dkt_h(xy, &quad6(xi, eta, 1));
    let (hxe, hye) = dkt_h(xy, &quad6(xi, eta, 2));
    let mut b = [[0.0; 9]; 3];
    for q in 0..9 {
        let hx = [
            inv[0][0] * hxi[q] + inv[0][1] * hxe[q],
            inv[1][0] * hxi[q] + inv[1][1] * hxe[q],
        ];
        let hy = [
            inv[0][0] * hyi[q] + inv[0][1] * hye[q],
            inv[1][0] * hyi[q] + inv[1][1] * hye[q],
        ];
        b[0][q] = hx[0];
        b[1][q] = hy[1];
        b[2][q] = hx[1] + hy[0];
    }
    b
}

/// DKT bending stiffness (9 x 9, DOFs w, rx, ry per corner); `d` = bending rigidity matrix.
pub fn dkt(xy: &Xy3, d: &[[f64; 3]; 3]) -> [[f64; 9]; 9] {
    let (area, _) = geometry(xy);
    let mut k = [[0.0; 9]; 9];
    for p in &POINTS {
        let b = dkt_b(xy, p[0], p[1]);
        for i in 0..9 {
            for j in 0..9 {
                let mut v = 0.0;
                for r in 0..3 {
                    for s in 0..3 {
                        v += b[r][i] * d[r][s] * b[s][j];
                    }
                }
                k[i][j] += v * area / 3.0;
            }
        }
    }
    k
}

/// Corner joint forces (local, per corner) of a uniform load per area: a third of the total each.
pub fn area_load(xy: &Xy3, q: [f64; 3]) -> [[f64; 3]; 3] {
    let (area, _) = geometry(xy);
    [q.map(|v| v * area / 3.0); 3]
}

/// Corner forces [F11, F22, F12, M11, M22, M12, V13, V23] (positive M = tension at the bottom) from local membrane DOFs
/// (u, v, rz per corner) and plate DOFs (w, rx, ry per corner); `plate` is None for membranes.
pub fn corner_forces(
    xy: &Xy3,
    mean: &[[f64; 3]; 3],
    c: &[[f64; 3]; 3],
    um: &[f64; 9],
    plate: Option<(&[[f64; 3]; 3], &[f64; 9])>,
) -> [[f64; 8]; 3] {
    let mut out = [[0.0; 8]; 3];
    for (k, p) in CORNERS.iter().enumerate() {
        let b = membrane_strain(xy, mean, p[0], p[1]);
        let strain: [f64; 3] = std::array::from_fn(|r| (0..9).map(|q| b[r][q] * um[q]).sum());
        for r in 0..3 {
            out[k][r] = (0..3).map(|s| c[r][s] * strain[s]).sum();
        }
    }
    if let Some((d, up)) = plate {
        for (k, p) in CORNERS.iter().enumerate() {
            let b = dkt_b(xy, p[0], p[1]);
            let kappa: [f64; 3] = std::array::from_fn(|r| (0..9).map(|q| b[r][q] * up[q]).sum());
            // Sign: M11 = -int(x3 s11), so positive moments give tension at the bottom.
            for r in 0..3 {
                out[k][3 + r] = -(0..3).map(|s| d[r][s] * kappa[s]).sum::<f64>();
            }
        }
        // Thin plates: constant shears from the linear moment field (V13 = -(M11,x + M12,y),
        // V23 = -(M12,x + M22,y)).
        let (_, dl) = geometry(xy);
        let grad = |d: usize, r: usize| (0..3).map(|a| dl[d][a] * out[a][3 + r]).sum::<f64>();
        let v13 = -(grad(0, 0) + grad(1, 2));
        let v23 = -(grad(0, 2) + grad(1, 1));
        for row in out.iter_mut() {
            row[6] = v13;
            row[7] = v23;
        }
    }
    out
}
