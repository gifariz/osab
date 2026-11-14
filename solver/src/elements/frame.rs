//! Two-node 3D frame element (beam-column), linear elastic.
//! Euler-Bernoulli bending, plus shear deformation in a plane when that shear area is given
//! (Timoshenko-corrected stiffness, see e.g. McGuire, Gallagher & Ziemian, "Matrix Structural Analysis").

use super::{distributed_part, LocalLoad, Props, GAUSS3};

/// Local stiffness; with rigid zones (`p.ri`, `p.rj`) the flexible part bends and shears over
/// l - ri - rj and is joined to the joints by rigid arms, while axial and torsion use the whole length
/// (docs/phases/PHASE4_PLAN.md, probe findings 2c). With a P-Delta axial force (`p.pd`) the geometric
/// stiffness is added: the flexible part's consistent one (see [`plain_geometric`]) through the arms,
/// and each rigid arm as a bar carrying the axial force (docs/phases/P_DELTA_PLAN.md).
pub fn local_stiffness(p: &Props) -> [[f64; 12]; 12] {
    if let Some((p12, p13)) = planes(p) {
        return merge_matrix(local_stiffness(&p12), local_stiffness(&p13));
    }
    if p.ri == 0.0 && p.rj == 0.0 {
        let mut k = plain_stiffness(p);
        add_geometric(&mut k, p);
        return k;
    }
    let mut flex = *p;
    flex.l = p.l - p.ri - p.rj;
    let mut k = plain_stiffness(&flex);
    add_geometric(&mut k, &flex);
    for (a, v) in [(0, p.e * p.a / p.l), (3, p.g * p.j / p.l)] {
        k[a][a] = v;
        k[a + 6][a + 6] = v;
        k[a][a + 6] = -v;
        k[a + 6][a] = -v;
    }
    let h = arms(p);
    let mut out = [[0.0; 12]; 12];
    for a in 0..12 {
        for b in 0..12 {
            out[a][b] = (0..12)
                .flat_map(|c| (0..12).map(move |d| (c, d)))
                .map(|(c, d)| h[c][a] * k[c][d] * h[d][b])
                .sum();
        }
    }
    arm_strings(&mut out, p);
    out
}

/// Bending shape functions of a member of length `l` (cubic; with shear deformation, phi = 12 EI /
/// (G As l²), those of the Timoshenko element, which reduce to the cubic ones at phi = 0) for the end
/// values (v_i, theta_i, v_j, theta_j), theta = dv/dx without shear: their values and their
/// derivatives along x at `xi` = x / l.
fn bending_shapes(l: f64, phi: f64, xi: f64) -> ([f64; 4], [f64; 4]) {
    let c = 1.0 / (1.0 + phi);
    let (x2, x3) = (xi * xi, xi * xi * xi);
    let n = [
        c * (2.0 * x3 - 3.0 * x2 - phi * xi + 1.0 + phi),
        c * l * (x3 - (2.0 + phi / 2.0) * x2 + (1.0 + phi / 2.0) * xi),
        c * (-2.0 * x3 + 3.0 * x2 + phi * xi),
        c * l * (x3 - (1.0 - phi / 2.0) * x2 - phi / 2.0 * xi),
    ];
    let d = [
        c * (6.0 * x2 - 6.0 * xi - phi) / l,
        c * (3.0 * x2 - (4.0 + phi) * xi + 1.0 + phi / 2.0),
        c * (-6.0 * x2 + 6.0 * xi + phi) / l,
        c * (3.0 * x2 - (2.0 - phi) * xi - phi / 2.0),
    ];
    (n, d)
}

/// The two bending planes: the local DOFs (u, r at end i, u, r at end j), the sign that makes r the
/// slope dv/dx (r3 in the 1-2 plane, -r2 in the 1-3 plane), and the plane's EI and G As.
fn bending_planes(p: &Props) -> [([usize; 4], f64, f64, f64); 2] {
    [
        ([1, 5, 7, 11], 1.0, p.e * p.i33, p.g * p.as2),
        ([2, 4, 8, 10], -1.0, p.e * p.i22, p.g * p.as3),
    ]
}

/// Consistent geometric stiffness of a plain member under the axial force `p.pd` (tension positive):
/// pd ∫ N'ᵀ N' dx over the length, N the member's own bending shape functions (see [`bending_shapes`]),
/// in both bending planes (e.g. Przemieniecki, "Theory of Matrix Structural Analysis", ch. 15; for the
/// Timoshenko shapes, McGuire, Gallagher & Ziemian, "Matrix Structural Analysis"). Zero when pd = 0.
pub fn plain_geometric(p: &Props) -> [[f64; 12]; 12] {
    let mut k = [[0.0; 12]; 12];
    add_geometric(&mut k, p);
    k
}

fn add_geometric(k: &mut [[f64; 12]; 12], p: &Props) {
    if p.pd == 0.0 || p.kind != crate::model::FrameKind::Frame {
        return;
    }
    let l = p.l;
    for (dofs, sign, ei, gas) in bending_planes(p) {
        let phi = shear_factor(ei, gas, l);
        for (g, w) in GAUSS3 {
            let (_, d) = bending_shapes(l, phi, (g + 1.0) / 2.0);
            let d = [d[0], sign * d[1], d[2], sign * d[3]];
            for a in 0..4 {
                for b in 0..4 {
                    k[dofs[a]][dofs[b]] += p.pd * w * l / 2.0 * d[a] * d[b];
                }
            }
        }
    }
}

/// The rigid arms as bars carrying the P-Delta axial force: an arm of length a turning by r moves its
/// far end sideways by a r, so its geometric stiffness is pd a on that turn (both planes).
fn arm_strings(k: &mut [[f64; 12]; 12], p: &Props) {
    if p.pd == 0.0 {
        return;
    }
    for (d, a) in [(5, p.ri), (11, p.rj), (4, p.ri3), (10, p.rj3)] {
        k[d][d] += p.pd * a;
    }
}

/// The hinges of a member whose releases act at its clear-length ends, as far as its shape needs them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hinges {
    /// The member has them.
    pub inside: bool,
    /// Length of the flexible piece between each end of the flexible part and its hinge (0: no hinge,
    /// or the hinge at that end), in the 1-2 and the 1-3 plane.
    pub pieces: [[f64; 2]; 2],
    /// Displacement [u1, u2, u3, r1, r2, r3] at each hinge, on the side of the member's end.
    pub near: [[f64; 6]; 2],
    /// What the displacement gains crossing each hinge, going away from the member's end.
    pub jumps: [[f64; 6]; 2],
    /// The same for the shape of the P-Delta moments: the released piece's own bending stiffness alone,
    /// without shear deformation or loads on it.
    pub shape_jumps: [[f64; 6]; 2],
    /// A member with a bending release at its joints: its end displacements for that shape, the released
    /// ones found the same way.
    pub shape_ends: Option<[f64; 12]>,
}

/// The P-Delta part of the bending moments at `x` from end i: the P-Delta axial force times the
/// member's sideways deflection there less at end i, [M2, M3] with M2 = pd (v3(x) - v3_i) and
/// M3 = pd (v2(x) - v2_i), the moment of the axial force at end i about the deflected cut. v is the
/// member's own shape from its end displacements `ul` (local, released DOFs included): the bending
/// shape functions on the flexible part (on each piece of it between hinges inside the member; without
/// shear deformation when the member has a bending release), the rigid arms turning as bars; a truss
/// stays straight. Zero without P-Delta.
pub fn pdelta_moments(p: &Props, ul: &[f64; 12], hinges: &Hinges, x: f64) -> [f64; 2] {
    if p.pd == 0.0 {
        return [0.0; 2];
    }
    let ul = hinges.shape_ends.as_ref().unwrap_or(ul);
    let l = p.l;
    if p.kind != crate::model::FrameKind::Frame {
        return [
            p.pd * (ul[8] - ul[2]) * x / l,
            p.pd * (ul[7] - ul[1]) * x / l,
        ];
    }
    let zones = [(p.ri, p.rj), (p.ri3, p.rj3)];
    let mut out = [0.0; 2];
    for (k, (dofs, sign, ei, gas)) in bending_planes(p).into_iter().enumerate() {
        let (ri, rj) = zones[k];
        let (vi, ti, vj, tj) = (
            ul[dofs[0]],
            sign * ul[dofs[1]],
            ul[dofs[2]],
            sign * ul[dofs[3]],
        );
        let lf = l - ri - rj;
        let v = if x <= ri {
            vi + ti * x
        } else if x >= l - rj {
            vj - tj * (l - x)
        } else {
            let ends = ((vi + ri * ti, ti), (vj - rj * tj, tj));
            let (x0, len, (v0, t0), (v1, t1)) = if hinges.inside {
                let at = |h: &[f64; 6]| (h[dofs[0]], sign * h[dofs[1]]);
                let own = |e: usize| {
                    let (near, jump) = (at(&hinges.near[e]), at(&hinges.shape_jumps[e]));
                    (near.0 + jump.0, near.1 + jump.1)
                };
                let [a, b] = hinges.pieces[k];
                let s = x - ri;
                if s < a {
                    (0.0, a, ends.0, at(&hinges.near[0]))
                } else if s <= lf - b {
                    (a, lf - a - b, own(0), own(1))
                } else {
                    (lf - b, b, at(&hinges.near[1]), ends.1)
                }
            } else {
                (0.0, lf, ends.0, ends.1)
            };
            let phi = if hinges.inside || hinges.shape_ends.is_some() {
                0.0
            } else {
                shear_factor(ei, gas, len)
            };
            let (n, _) = bending_shapes(len, phi, (x - ri - x0) / len);
            n[0] * v0 + n[1] * t0 + n[2] * v1 + n[3] * t1
        };
        out[1 - k] = p.pd * (v - vi);
    }
    out
}

/// The local DOFs of bending in the 1-3 plane: u3 and r2 at both ends.
const PLANE13: [usize; 4] = [2, 4, 8, 10];

/// When the two bending planes have different rigid zones: `p` with both planes' zones those of the 1-2
/// plane, and with both those of the 1-3 plane. The planes do not interact, so a result is the first's
/// with the 1-3 plane's DOFs from the second.
pub fn planes(p: &Props) -> Option<(Props, Props)> {
    let both = |ri, rj| Props {
        ri,
        rj,
        ri3: ri,
        rj3: rj,
        ..*p
    };
    (p.ri3 != p.ri || p.rj3 != p.rj).then(|| (both(p.ri, p.rj), both(p.ri3, p.rj3)))
}

fn merge_vector(mut a: [f64; 12], b: [f64; 12]) -> [f64; 12] {
    for d in PLANE13 {
        a[d] = b[d];
    }
    a
}

fn merge_matrix(mut a: [[f64; 12]; 12], b: [[f64; 12]; 12]) -> [[f64; 12]; 12] {
    for r in 0..12 {
        for c in 0..12 {
            if PLANE13.contains(&r) || PLANE13.contains(&c) {
                a[r][c] = b[r][c];
            }
        }
    }
    a
}

/// The rigid arms' map from joint to flexible-end displacements: u2 += ri r3, u3 -= ri r2 at end i;
/// u2 -= rj r3, u3 += rj r2 at end j.
fn arms(p: &Props) -> [[f64; 12]; 12] {
    let mut h = [[0.0; 12]; 12];
    for (a, row) in h.iter_mut().enumerate() {
        row[a] = 1.0;
    }
    h[1][5] = p.ri;
    h[2][4] = -p.ri;
    h[7][11] = -p.rj;
    h[8][10] = p.rj;
    h
}

fn plain_stiffness(p: &Props) -> [[f64; 12]; 12] {
    let mut k = [[0.0; 12]; 12];
    let l = p.l;

    let ea = p.e * p.a / l;
    k[0][0] = ea;
    k[0][6] = -ea;
    k[6][6] = ea;

    let gj = p.g * p.j / l;
    k[3][3] = gj;
    k[3][9] = -gj;
    k[9][9] = gj;

    // Bending in the 1-2 plane (translation u2, rotation r3; du2/dx = r3), uses I33 and As2.
    let phi2 = shear_factor(p.e * p.i33, p.g * p.as2, l);
    let c = p.e * p.i33 / (l.powi(3) * (1.0 + phi2));
    k[1][1] = 12.0 * c;
    k[1][5] = 6.0 * l * c;
    k[1][7] = -12.0 * c;
    k[1][11] = 6.0 * l * c;
    k[5][5] = (4.0 + phi2) * l * l * c;
    k[5][7] = -6.0 * l * c;
    k[5][11] = (2.0 - phi2) * l * l * c;
    k[7][7] = 12.0 * c;
    k[7][11] = -6.0 * l * c;
    k[11][11] = (4.0 + phi2) * l * l * c;

    // Bending in the 1-3 plane (translation u3, rotation r2; du3/dx = -r2), uses I22 and As3.
    let phi3 = shear_factor(p.e * p.i22, p.g * p.as3, l);
    let c = p.e * p.i22 / (l.powi(3) * (1.0 + phi3));
    k[2][2] = 12.0 * c;
    k[2][4] = -6.0 * l * c;
    k[2][8] = -12.0 * c;
    k[2][10] = -6.0 * l * c;
    k[4][4] = (4.0 + phi3) * l * l * c;
    k[4][8] = 6.0 * l * c;
    k[4][10] = (2.0 - phi3) * l * l * c;
    k[8][8] = 12.0 * c;
    k[8][10] = 6.0 * l * c;
    k[10][10] = (4.0 + phi3) * l * l * c;

    for i in 0..12 {
        for j in 0..i {
            k[i][j] = k[j][i];
        }
    }
    k
}

/// phi = 12 EI / (G As L^2); zero when there is no shear area.
fn shear_factor(ei: f64, gas: f64, l: f64) -> f64 {
    if gas > 0.0 {
        12.0 * ei / (gas * l * l)
    } else {
        0.0
    }
}

/// Fixed-end reactions: forces the two fixed supports apply to the element when the member loads act
/// and both ends are fully restrained. Local DOF order.
pub fn fixed_end_forces(p: &Props, loads: &[LocalLoad]) -> [f64; 12] {
    if let Some((p12, p13)) = planes(p) {
        return merge_vector(fixed_end_forces(&p12, loads), fixed_end_forces(&p13, loads));
    }
    if p.ri == 0.0 && p.rj == 0.0 {
        return plain_fixed_end_forces(p, loads);
    }
    let (mut f, flex_loads) = split_loads(p, loads);
    let ff = plain_fixed_end_forces(&flexible(p), &flex_loads);
    let h = arms(p);
    for a in 0..12 {
        f[a] += (0..12).map(|c| h[c][a] * ff[c]).sum::<f64>();
    }
    f
}

/// The flexible part between the rigid zones, as a plain member.
fn flexible(p: &Props) -> Props {
    Props {
        l: p.l - p.ri - p.rj,
        ri: 0.0,
        rj: 0.0,
        ri3: 0.0,
        rj3: 0.0,
        oi: 0.0,
        oj: 0.0,
        ..*p
    }
}

/// Loads on a member with rigid zones: the joint forces of the axial loads (on the whole, axially
/// flexible member) and of the transverse loads on the rigid zones (by statics), and the transverse
/// loads on the flexible part (measured from its end i).
fn split_loads(p: &Props, loads: &[LocalLoad]) -> ([f64; 12], Vec<LocalLoad>) {
    let (l, ri, rj) = (p.l, p.ri, p.rj);
    let axial: Vec<LocalLoad> = loads.iter().copied().filter(|ld| dir_of(ld) == 0).collect();
    let mut f = plain_fixed_end_forces(
        &Props {
            ri: 0.0,
            rj: 0.0,
            ri3: 0.0,
            rj3: 0.0,
            ..*p
        },
        &axial,
    );
    let mut flex_loads = Vec::new();
    for ld in loads.iter().filter(|ld| dir_of(ld) != 0) {
        let d = dir_of(ld);
        // Resultant and first moment (about end i) of the parts on the two rigid zones.
        let (zi, zj) = match *ld {
            LocalLoad::Point { p: v, at, .. } => {
                if at <= ri {
                    ((v, v * at), (0.0, 0.0))
                } else if at >= l - rj {
                    ((0.0, 0.0), (v, v * at))
                } else {
                    flex_loads.push(LocalLoad::Point {
                        dir: d,
                        p: v,
                        at: at - ri,
                    });
                    ((0.0, 0.0), (0.0, 0.0))
                }
            }
            LocalLoad::Distributed {
                w1, w2, from, to, ..
            } => {
                let w = |s: f64| w1 + (w2 - w1) * (s - from) / (to - from);
                let (a, b) = (from.max(ri), to.min(l - rj));
                if b > a {
                    flex_loads.push(LocalLoad::Distributed {
                        dir: d,
                        w1: w(a),
                        w2: w(b),
                        from: a - ri,
                        to: b - ri,
                    });
                }
                (
                    super::distributed_part(w1, w2, from, to, 0.0, ri),
                    super::distributed_part(w1, w2, from, to, l - rj, l),
                )
            }
        };
        // Forces the joints apply: minus the resultant, and minus its moment about the joint.
        let (fi, mi) = zi;
        let (fj, mj) = (zj.0, zj.1 - l * zj.0);
        f[d] -= fi;
        f[6 + d] -= fj;
        if d == 1 {
            f[5] -= mi;
            f[11] -= mj;
        } else {
            f[4] += mi;
            f[10] += mj;
        }
    }
    (f, flex_loads)
}

/// Whether any bending DOF (shear or moment, at either end) is released.
pub fn bending_release(rel: &[bool; 12]) -> bool {
    [1, 2, 4, 5, 7, 8, 10, 11].iter().any(|&d| rel[d])
}

/// Whether the member's end releases act inside it: at the clear-length ends of a member with end
/// offsets, rather than at the joints.
pub fn hinged_inside(p: &Props, rel: &[bool; 12]) -> bool {
    rel.contains(&true) && (p.oi > 0.0 || p.oj > 0.0)
}

/// Bending and shear of the flexible part of a member whose releases act at its clear-length ends:
/// the part split at the hinges (x = oi and l - oj from the joints, at an end with a bending release)
/// into up to three pieces, the middle one released. DOFs: flexible end i (0..6), flexible end j
/// (6..12), hinge i (12..18), hinge j (18..24); axial and torsion over the whole length (unless
/// released) between the flexible ends.
struct Hinged {
    /// Length of the pieces before hinge i and after hinge j.
    pieces: [f64; 2],
    k: Vec<f64>,
    f: Vec<f64>,
    /// The hinge DOFs condensed out.
    inner: Vec<usize>,
    /// The released middle piece: its nodes, stiffness, fixed-end forces and released DOFs.
    mid: ([usize; 2], [[f64; 12]; 12], [f64; 12], [bool; 12]),
    /// The middle piece's stiffness without P-Delta or shear deformation.
    mid_plain: [[f64; 12]; 12],
    /// Joint forces of the axial loads and of the loads on the rigid zones.
    direct: [f64; 12],
}

const N: usize = 24;

fn hinged(p: &Props, rel: &[bool; 12], loads: &[LocalLoad]) -> Hinged {
    let lf = p.l - p.ri - p.rj;
    let tiny = 1e-12 * p.l;
    let bends = |dofs: [usize; 4]| dofs.iter().any(|&d| rel[d]);
    let a = if p.oi - p.ri > tiny && bends([1, 2, 4, 5]) {
        p.oi - p.ri
    } else {
        0.0
    };
    let b = if p.oj - p.rj > tiny && bends([7, 8, 10, 11]) {
        p.oj - p.rj
    } else {
        0.0
    };
    let (mut direct, flex_loads) = split_loads(p, loads);
    // Axial (the only loads with a P release): a released end passes its share to the other end.
    for (r, from, to) in [(0, 0, 6), (6, 6, 0)] {
        if rel[r] {
            direct[to] += direct[from];
            direct[from] = 0.0;
        }
    }
    // Bending-only pieces: (start, end, nodes).
    let (hi, hj) = (if a > 0.0 { 2 } else { 0 }, if b > 0.0 { 3 } else { 1 });
    let pieces = [
        (0.0, a, [0, 2]),
        (a, lf - b, [hi, hj]),
        (lf - b, lf, [3, 1]),
    ];
    let mut k = vec![0.0; N * N];
    let mut f = vec![0.0; N];
    let mut bend_rel = *rel;
    for d in [0, 3, 6, 9] {
        bend_rel[d] = false;
    }
    let mut mid = ([0, 1], [[0.0; 12]; 12], [0.0; 12], [false; 12]);
    let mut mid_plain = [[0.0; 12]; 12];
    for (n, &(x0, x1, nodes)) in pieces.iter().enumerate() {
        if x1 - x0 <= tiny {
            continue;
        }
        let bp = Props {
            l: x1 - x0,
            a: 0.0,
            j: 0.0,
            ..flexible(p)
        };
        let on: Vec<LocalLoad> = flex_loads
            .iter()
            .filter_map(|ld| {
                piece_load(ld, x0, x1, n == 0 || (n == 1 && a == 0.0), x1 >= lf - tiny)
            })
            .collect();
        let mut kp = plain_stiffness(&bp);
        if n == 1 {
            mid_plain = plain_stiffness(&Props {
                as2: 0.0,
                as3: 0.0,
                ..bp
            });
        }
        add_geometric(&mut kp, &bp);
        let mut fp = plain_fixed_end_forces(&bp, &on);
        if n == 1 {
            mid = (nodes, kp, fp, bend_rel);
            if bend_rel.contains(&true) {
                (kp, fp) = condense(&kp, &fp, &bend_rel);
            }
        }
        for r in 0..12 {
            let gr = nodes[r / 6] * 6 + r % 6;
            f[gr] += fp[r];
            for c in 0..12 {
                k[gr * N + nodes[c / 6] * 6 + c % 6] += kp[r][c];
            }
        }
    }
    for (d, v) in [(0, p.e * p.a / p.l), (3, p.g * p.j / p.l)] {
        if !rel[d] && !rel[d + 6] {
            k[d * N + d] += v;
            k[(d + 6) * N + d + 6] += v;
            k[d * N + d + 6] -= v;
            k[(d + 6) * N + d] -= v;
        }
    }
    let inner = [(a > 0.0, 12), (b > 0.0, 18)]
        .iter()
        .filter(|x| x.0)
        .flat_map(|&(_, o)| [1, 2, 4, 5].map(|d| o + d))
        .collect();
    Hinged {
        pieces: [a, b],
        k,
        f,
        inner,
        mid,
        mid_plain,
        direct,
    }
}

/// The part of a flexible-part load on the piece from x0 to x1, measured from x0. A point load at a
/// piece boundary goes to the piece before it (`first`: this piece starts the member; `last`: it ends
/// it).
fn piece_load(ld: &LocalLoad, x0: f64, x1: f64, first: bool, last: bool) -> Option<LocalLoad> {
    match *ld {
        LocalLoad::Point { dir, p, at } => {
            ((at > x0 || first) && (at <= x1 || last)).then_some(LocalLoad::Point {
                dir,
                p,
                at: at - x0,
            })
        }
        LocalLoad::Distributed {
            dir,
            w1,
            w2,
            from,
            to,
        } => {
            let (a, b) = (from.max(x0), to.min(x1));
            let w = |s: f64| w1 + (w2 - w1) * (s - from) / (to - from);
            (b > a).then(|| LocalLoad::Distributed {
                dir,
                w1: w(a),
                w2: w(b),
                from: a - x0,
                to: b - x0,
            })
        }
    }
}

impl Hinged {
    /// K_ei K_ii^-1 (external rows 0..12).
    fn carry(&self) -> Vec<[f64; 12]> {
        let n = self.inner.len();
        let kii: Vec<f64> = (0..n * n)
            .map(|x| self.k[self.inner[x / n] * N + self.inner[x % n]])
            .collect();
        let inv = inverse(&kii, n);
        (0..n)
            .map(|q| {
                std::array::from_fn(|e| {
                    (0..n)
                        .map(|p| self.k[e * N + self.inner[p]] * inv[p * n + q])
                        .sum()
                })
            })
            .collect()
    }

    /// Stiffness and fixed-end forces at the joints.
    fn joints(&self, p: &Props) -> ([[f64; 12]; 12], [f64; 12]) {
        let g = self.carry();
        let mut kc = [[0.0; 12]; 12];
        let mut fc = [0.0; 12];
        for e in 0..12 {
            for c in 0..12 {
                kc[e][c] = self.k[e * N + c]
                    - (0..g.len())
                        .map(|q| g[q][e] * self.k[self.inner[q] * N + c])
                        .sum::<f64>();
            }
            fc[e] = self.f[e]
                - (0..g.len())
                    .map(|q| g[q][e] * self.f[self.inner[q]])
                    .sum::<f64>();
        }
        let h = arms(p);
        let mut k = [[0.0; 12]; 12];
        let mut f = self.direct;
        for a in 0..12 {
            f[a] += (0..12).map(|c| h[c][a] * fc[c]).sum::<f64>();
            for b in 0..12 {
                k[a][b] = (0..12)
                    .flat_map(|c| (0..12).map(move |d| (c, d)))
                    .map(|(c, d)| h[c][a] * kc[c][d] * h[d][b])
                    .sum();
            }
        }
        arm_strings(&mut k, p);
        (k, f)
    }
}

/// Stiffness at the joints of a member whose releases act at its clear-length ends.
pub fn hinged_stiffness(p: &Props, rel: &[bool; 12]) -> [[f64; 12]; 12] {
    if let Some((p12, p13)) = planes(p) {
        return merge_matrix(hinged_stiffness(&p12, rel), hinged_stiffness(&p13, rel));
    }
    hinged(p, rel, &[]).joints(p).0
}

/// Fixed-end forces at the joints of a member whose releases act at its clear-length ends.
pub fn hinged_fixed_end_forces(p: &Props, rel: &[bool; 12], loads: &[LocalLoad]) -> [f64; 12] {
    if let Some((p12, p13)) = planes(p) {
        return merge_vector(
            hinged_fixed_end_forces(&p12, rel, loads),
            hinged_fixed_end_forces(&p13, rel, loads),
        );
    }
    hinged(p, rel, loads).joints(p).1
}

/// End forces at the joints of a member whose releases act at its clear-length ends, from the joint
/// displacements `u` (local); and its hinges: the displacements [u1, u2, u3, r1, r2, r3] at them and the
/// jumps across them at x = oi (going towards end j) and x = l - oj (going towards end i).
pub fn hinged_end_forces(
    p: &Props,
    rel: &[bool; 12],
    loads: &[LocalLoad],
    u: &[f64; 12],
) -> ([f64; 12], Hinges) {
    if let Some((p12, p13)) = planes(p) {
        let (end, mut hinges) = hinged_end_forces(&p12, rel, loads, u);
        let (end13, h13) = hinged_end_forces(&p13, rel, loads, u);
        hinges.pieces[1] = h13.pieces[1];
        for e in 0..2 {
            for d in [2, 4] {
                hinges.near[e][d] = h13.near[e][d];
                hinges.jumps[e][d] = h13.jumps[e][d];
                hinges.shape_jumps[e][d] = h13.shape_jumps[e][d];
            }
        }
        return (merge_vector(end, end13), hinges);
    }
    let s = hinged(p, rel, loads);
    let (k, f) = s.joints(p);
    let mut end = f;
    for a in 0..12 {
        end[a] += (0..12).map(|b| k[a][b] * u[b]).sum::<f64>();
    }
    // Displacements of the flexible ends (through the arms) and of the hinges.
    let h = arms(p);
    let mut all = [0.0; N];
    for a in 0..12 {
        all[a] = (0..12).map(|b| h[a][b] * u[b]).sum();
    }
    let n = s.inner.len();
    let kii: Vec<f64> = (0..n * n)
        .map(|x| s.k[s.inner[x / n] * N + s.inner[x % n]])
        .collect();
    let inv = inverse(&kii, n);
    let rhs: Vec<f64> = s
        .inner
        .iter()
        .map(|&i| s.f[i] + (0..12).map(|e| s.k[i * N + e] * all[e]).sum::<f64>())
        .collect();
    for (q, &i) in s.inner.iter().enumerate() {
        all[i] = -(0..n).map(|r| inv[q * n + r] * rhs[r]).sum::<f64>();
    }
    let (nodes, km, fm, mrel) = s.mid;
    let at: [f64; 12] = std::array::from_fn(|a| all[nodes[a / 6] * 6 + a % 6]);
    let mut own = at;
    let mut own_plain = at;
    if mrel.contains(&true) {
        released_displacements(&km, &fm, &mrel, &mut own);
        released_displacements(&s.mid_plain, &[0.0; 12], &mrel, &mut own_plain);
    }
    let hinges = Hinges {
        inside: bending_release(rel),
        pieces: [s.pieces, s.pieces],
        near: [
            std::array::from_fn(|d| at[d]),
            std::array::from_fn(|d| at[6 + d]),
        ],
        jumps: [
            std::array::from_fn(|d| own[d] - at[d]),
            std::array::from_fn(|d| own[6 + d] - at[6 + d]),
        ],
        shape_jumps: [
            std::array::from_fn(|d| own_plain[d] - at[d]),
            std::array::from_fn(|d| own_plain[6 + d] - at[6 + d]),
        ],
        shape_ends: None,
    };
    (end, hinges)
}

/// Inverse of a small dense matrix (row-major, n x n) by Gauss-Jordan with partial pivoting.
fn inverse(m: &[f64], n: usize) -> Vec<f64> {
    let mut a = vec![0.0; n * 2 * n];
    for p in 0..n {
        a[p * 2 * n..p * 2 * n + n].copy_from_slice(&m[p * n..p * n + n]);
        a[p * 2 * n + n + p] = 1.0;
    }
    for c in 0..n {
        let piv = (c..n)
            .max_by(|&x, &y| a[x * 2 * n + c].abs().total_cmp(&a[y * 2 * n + c].abs()))
            .expect("rows");
        for q in 0..2 * n {
            a.swap(c * 2 * n + q, piv * 2 * n + q);
        }
        let d = a[c * 2 * n + c];
        for q in 0..2 * n {
            a[c * 2 * n + q] /= d;
        }
        for p in (0..n).filter(|&p| p != c) {
            let f = a[p * 2 * n + c];
            if f != 0.0 {
                for q in 0..2 * n {
                    a[p * 2 * n + q] -= f * a[c * 2 * n + q];
                }
            }
        }
    }
    (0..n * n).map(|x| a[(x / n) * 2 * n + n + x % n]).collect()
}

fn dir_of(ld: &LocalLoad) -> usize {
    match *ld {
        LocalLoad::Point { dir, .. } | LocalLoad::Distributed { dir, .. } => dir,
    }
}

fn plain_fixed_end_forces(p: &Props, loads: &[LocalLoad]) -> [f64; 12] {
    let mut f = [0.0; 12];
    let l = p.l;

    // Axial: a bar fixed at both ends shares each load in proportion to the distance to the far end.
    for load in loads {
        match *load {
            LocalLoad::Point { dir: 0, p: pa, at } => {
                f[0] -= pa * (l - at) / l;
                f[6] -= pa * at / l;
            }
            LocalLoad::Distributed {
                dir: 0,
                w1,
                w2,
                from,
                to,
            } => {
                let (total, m0) = distributed_part(w1, w2, from, to, from, to);
                f[0] -= total - m0 / l;
                f[6] -= m0 / l;
            }
            _ => {}
        }
    }

    // 1-2 plane: v = u2, theta = r3.
    let r = plane_fixed_end(loads, 1, l, p.e * p.i33, p.g * p.as2);
    f[1] = r[0];
    f[5] = r[1];
    f[7] = r[2];
    f[11] = r[3];

    // 1-3 plane: v = u3, theta = -r2, so the moments change sign.
    let r = plane_fixed_end(loads, 2, l, p.e * p.i22, p.g * p.as3);
    f[2] = r[0];
    f[4] = -r[1];
    f[8] = r[2];
    f[10] = -r[3];
    f
}

/// Fixed-end reactions [V_i, M_i, V_j, M_j] for transverse loads in local direction `dir`, in a plane
/// where the rotation is theta = dv/dx. Exact for Euler-Bernoulli and Timoshenko beams.
///
/// Method: release end j (cantilever from end i), get the tip displacement and rotation from virtual
/// work, then find the end-j forces that bring them back to zero. End i follows from statics.
fn plane_fixed_end(loads: &[LocalLoad], dir: usize, l: f64, ei: f64, gas: f64) -> [f64; 4] {
    let loads: Vec<LocalLoad> = loads
        .iter()
        .copied()
        .filter(|ld| match *ld {
            LocalLoad::Point { dir: d, .. } | LocalLoad::Distributed { dir: d, .. } => d == dir,
        })
        .collect();
    if loads.is_empty() {
        return [0.0; 4];
    }
    let shear_flex = if gas > 0.0 { 1.0 / gas } else { 0.0 };

    // Cantilever internal forces at x, from the free part (x..L):
    // V(x) = sum of loads beyond x, M(x) = sum of (s - x) * load beyond x.
    let vm = |x: f64| -> (f64, f64) {
        let (mut v, mut m) = (0.0, 0.0);
        for ld in &loads {
            match *ld {
                LocalLoad::Point { p, at, .. } => {
                    if at > x {
                        v += p;
                        m += (at - x) * p;
                    }
                }
                LocalLoad::Distributed {
                    w1, w2, from, to, ..
                } => {
                    let (f, m0) = distributed_part(w1, w2, from, to, x, to);
                    v += f;
                    m += m0 - x * f;
                }
            }
        }
        (v, m)
    };

    // Integrate piecewise between load breakpoints; the integrands are polynomials of degree <= 4
    // on each piece (linearly varying loads), so 3-point Gauss is exact.
    let mut cuts = vec![0.0, l];
    for ld in &loads {
        match *ld {
            LocalLoad::Point { at, .. } => cuts.push(at),
            LocalLoad::Distributed { from, to, .. } => {
                cuts.push(from);
                cuts.push(to);
            }
        }
    }
    cuts.sort_by(|a, b| a.total_cmp(b));
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12 * l);

    let (mut tip_v, mut tip_theta) = (0.0, 0.0);
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
        for (xi, wt) in GAUSS3 {
            let x = mid + half * xi;
            let (v, m) = vm(x);
            tip_v += wt * half * ((l - x) * m / ei + v * shear_flex);
            tip_theta += wt * half * (m / ei);
        }
    }

    // Cantilever tip flexibility for a tip force and tip moment.
    let f11 = l.powi(3) / (3.0 * ei) + l * shear_flex;
    let f12 = l * l / (2.0 * ei);
    let f22 = l / ei;
    let det = f11 * f22 - f12 * f12;
    // Solve F [Vj, Mj] = -[tip_v, tip_theta].
    let vj = -(f22 * tip_v - f12 * tip_theta) / det;
    let mj = -(-f12 * tip_v + f11 * tip_theta) / det;

    // Statics: total load and its moment about end i.
    let (mut total, mut moment) = (0.0, 0.0);
    for ld in &loads {
        match *ld {
            LocalLoad::Point { p, at, .. } => {
                total += p;
                moment += at * p;
            }
            LocalLoad::Distributed {
                w1, w2, from, to, ..
            } => {
                let (f, m0) = distributed_part(w1, w2, from, to, from, to);
                total += f;
                moment += m0;
            }
        }
    }
    let vi = -(total + vj);
    let mi = -(mj + moment + l * vj);
    [vi, mi, vj, mj]
}

/// Indices of the released DOFs and the inverse of their stiffness block K_rr (row-major).
fn released_block(k: &[[f64; 12]; 12], rel: &[bool; 12]) -> (Vec<usize>, Vec<f64>) {
    let r: Vec<usize> = (0..12).filter(|&a| rel[a]).collect();
    let n = r.len();
    // K_rr is positive definite for the releases that are accepted (see `Releases::instability`).
    let krr: Vec<f64> = (0..n * n).map(|x| k[r[x / n]][r[x % n]]).collect();
    let inv = inverse(&krr, n);
    (r, inv)
}

/// End releases by static condensation: the stiffness and fixed-end forces of a frame whose
/// released end forces (`rel`, local DOFs of ends i and j) are zero. Exact; the released rows and
/// columns become zero.
pub fn condense(
    k: &[[f64; 12]; 12],
    f: &[f64; 12],
    rel: &[bool; 12],
) -> ([[f64; 12]; 12], [f64; 12]) {
    let (r, inv) = released_block(k, rel);
    let n = r.len();
    let mut kc = *k;
    let mut fc = *f;
    for a in 0..12 {
        // g = K_ar K_rr^-1
        let g: Vec<f64> = (0..n)
            .map(|q| (0..n).map(|p| k[a][r[p]] * inv[p * n + q]).sum())
            .collect();
        for b in 0..12 {
            kc[a][b] -= (0..n).map(|q| g[q] * k[r[q]][b]).sum::<f64>();
        }
        fc[a] -= (0..n).map(|q| g[q] * f[r[q]]).sum::<f64>();
    }
    for &a in &r {
        fc[a] = 0.0;
        for b in 0..12 {
            kc[a][b] = 0.0;
            kc[b][a] = 0.0;
        }
    }
    (kc, fc)
}

/// The element's own end displacements at released DOFs (they differ from the joint's): u_r =
/// -K_rr^-1 (K_rc u_c + f_r), with `u` the local end displacements taken from the joints.
pub fn released_displacements(
    k: &[[f64; 12]; 12],
    f: &[f64; 12],
    rel: &[bool; 12],
    u: &mut [f64; 12],
) {
    let (r, inv) = released_block(k, rel);
    let n = r.len();
    let rhs: Vec<f64> = r
        .iter()
        .map(|&a| {
            f[a] + (0..12)
                .filter(|&b| !rel[b])
                .map(|b| k[a][b] * u[b])
                .sum::<f64>()
        })
        .collect();
    for (p, &a) in r.iter().enumerate() {
        u[a] = -(0..n).map(|q| inv[p * n + q] * rhs[q]).sum::<f64>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{internal_forces, Side};
    use crate::model::FrameKind;

    fn props(l: f64) -> Props {
        Props {
            kind: FrameKind::Frame,
            e: 200e6,
            g: 80e6,
            a: 0.01,
            i22: 5e-5,
            i33: 2e-4,
            j: 1e-5,
            as2: 0.0,
            as3: 0.0,
            l,
            ri: 0.0,
            rj: 0.0,
            ri3: 0.0,
            rj3: 0.0,
            oi: 0.0,
            oj: 0.0,
            pd: 0.0,
        }
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} != {b}");
    }

    #[test]
    fn stiffness_is_symmetric_and_rigid_body_free() {
        let mut p = props(3.0);
        p.as2 = 0.008;
        p.as3 = 0.004;
        let k = local_stiffness(&p);
        for i in 0..12 {
            for j in 0..12 {
                assert_eq!(k[i][j], k[j][i]);
            }
        }
        // Rigid rotation about local 3 by a small angle: u2 at j = L * theta.
        let th = 1e-3;
        let mut u = [0.0; 12];
        u[5] = th;
        u[11] = th;
        u[7] = p.l * th;
        for i in 0..12 {
            let f: f64 = (0..12).map(|j| k[i][j] * u[j]).sum();
            assert!(f.abs() < 1e-6, "row {i}: {f}");
        }
        // Rigid rotation about local 2: u3 at j = -L * theta.
        let mut u = [0.0; 12];
        u[4] = th;
        u[10] = th;
        u[8] = -p.l * th;
        for i in 0..12 {
            let f: f64 = (0..12).map(|j| k[i][j] * u[j]).sum();
            assert!(f.abs() < 1e-6, "row {i}: {f}");
        }
    }

    #[test]
    fn uniform_load_fixed_end_moments() {
        let p = props(6.0);
        let w = -12.0;
        let f = fixed_end_forces(&p, &[LocalLoad::uniform(1, w, 0.0, 6.0)]);
        approx(f[1], 36.0);
        approx(f[7], 36.0);
        approx(f[5], 36.0); // w L^2 / 12 = 36, acting about +3 at end i
        approx(f[11], -36.0);
        // End moments give hogging (negative M3) at both ends, wL^2/24 sagging at midspan.
        approx(
            internal_forces(&f, &[LocalLoad::uniform(1, w, 0.0, 6.0)], 0.0, Side::Right)[5],
            -36.0,
        );
        approx(
            internal_forces(&f, &[LocalLoad::uniform(1, w, 0.0, 6.0)], 3.0, Side::Left)[5],
            18.0,
        );
    }

    #[test]
    fn triangular_load_fixed_end_moments() {
        // Load rising from 0 at end i to w at end j: w L^2 / 30 at i, w L^2 / 20 at j, and the end
        // shears 3 w L / 20 and 7 w L / 20.
        let (w, l) = (-12.0, 5.0);
        let tri = LocalLoad::Distributed {
            dir: 1,
            w1: 0.0,
            w2: w,
            from: 0.0,
            to: l,
        };
        let f = fixed_end_forces(&props(l), &[tri]);
        approx(f[5], -w * l * l / 30.0);
        approx(f[11], w * l * l / 20.0);
        approx(f[1], -3.0 * w * l / 20.0);
        approx(f[7], -7.0 * w * l / 20.0);
        // A trapezoid is a uniform load plus a triangle, also on a part of the member and with shear
        // deformation; and its internal forces at end j equal the end-j forces.
        let mut p = props(l);
        p.as2 = 2e-3;
        let trap = [LocalLoad::Distributed {
            dir: 1,
            w1: -3.0,
            w2: -8.0,
            from: 1.0,
            to: 4.5,
        }];
        let parts = [
            LocalLoad::uniform(1, -3.0, 1.0, 4.5),
            LocalLoad::Distributed {
                dir: 1,
                w1: 0.0,
                w2: -5.0,
                from: 1.0,
                to: 4.5,
            },
        ];
        let (a, b) = (fixed_end_forces(&p, &trap), fixed_end_forces(&p, &parts));
        for k in 0..12 {
            approx(a[k], b[k]);
        }
        let s = internal_forces(&a, &trap, l, Side::Left);
        approx(s[1], a[7]);
        approx(s[5], a[11]);
        // Axial: the bar's ends share the load by the lever rule about its centroid.
        let ax = [LocalLoad::Distributed {
            dir: 0,
            w1: 0.0,
            w2: 6.0,
            from: 0.0,
            to: l,
        }];
        let f = fixed_end_forces(&props(l), &ax);
        approx(f[0], -6.0 * l / 2.0 / 3.0);
        approx(f[6], -6.0 * l / 2.0 * 2.0 / 3.0);
    }

    #[test]
    fn point_load_fixed_end_moments_euler_bernoulli() {
        // P b^2 a / L^2 and P a^2 b / L^2.
        let p = props(5.0);
        let (pp, a, b) = (-20.0, 2.0, 3.0);
        let f = fixed_end_forces(
            &p,
            &[LocalLoad::Point {
                dir: 1,
                p: pp,
                at: a,
            }],
        );
        approx(f[5], -pp * a * b * b / 25.0);
        approx(f[11], pp * a * a * b / 25.0);
        approx(f[1] + f[7], -pp);
    }

    #[test]
    fn local_3_direction_mirrors_local_2() {
        let p = props(5.0);
        let f2 = fixed_end_forces(
            &p,
            &[LocalLoad::Point {
                dir: 1,
                p: -7.0,
                at: 1.5,
            }],
        );
        let mut p3 = p;
        std::mem::swap(&mut p3.i22, &mut p3.i33);
        let f3 = fixed_end_forces(
            &p3,
            &[LocalLoad::Point {
                dir: 2,
                p: -7.0,
                at: 1.5,
            }],
        );
        approx(f3[2], f2[1]);
        approx(f3[8], f2[7]);
        approx(f3[4], -f2[5]);
        approx(f3[10], -f2[11]);
    }

    #[test]
    fn shear_deformation_point_load_fixed_end_moments() {
        // Timoshenko fixed-fixed beam, central point load: end moments stay P L / 8 by symmetry,
        // and a uniform load keeps w L^2 / 12, for any shear stiffness.
        let mut p = props(4.0);
        p.as2 = 1e-4;
        let f = fixed_end_forces(
            &p,
            &[LocalLoad::Point {
                dir: 1,
                p: -10.0,
                at: 2.0,
            }],
        );
        approx(f[5], 10.0 * 4.0 / 8.0);
        let f = fixed_end_forces(&p, &[LocalLoad::uniform(1, -3.0, 0.0, 4.0)]);
        approx(f[5], 3.0 * 16.0 / 12.0);
    }

    #[test]
    fn axial_loads() {
        let p = props(4.0);
        let f = fixed_end_forces(
            &p,
            &[LocalLoad::Point {
                dir: 0,
                p: 8.0,
                at: 1.0,
            }],
        );
        approx(f[0], -6.0);
        approx(f[6], -2.0);
    }

    #[test]
    fn internal_forces_match_end_j() {
        // Any equilibrated end-force set: internal forces at x = L must equal the end-j values.
        let p = props(5.0);
        let loads = [
            LocalLoad::uniform(1, -4.0, 1.0, 4.0),
            LocalLoad::Point {
                dir: 2,
                p: 3.0,
                at: 2.0,
            },
            LocalLoad::Point {
                dir: 0,
                p: 2.0,
                at: 4.5,
            },
        ];
        let f = fixed_end_forces(&p, &loads);
        let s = internal_forces(&f, &loads, p.l, Side::Left);
        approx(s[0], f[6]);
        approx(s[1], f[7]);
        approx(s[2], f[8]);
        approx(s[3], f[9]);
        approx(s[4], -f[10]);
        approx(s[5], f[11]);
    }

    /// Tip deflection under a tip load `h` along local 2 of a cantilever fixed at end i (tip DOFs u2, r3).
    fn tip(p: &Props, h: f64) -> f64 {
        let k = local_stiffness(p);
        let (a, b, c) = (k[7][7], k[7][11], k[11][11]);
        h * c / (a * c - b * b)
    }

    /// The probe's 3 m column 0.3 x 0.6 (E 25e6, I33 0.0054) with 3000 kN compression.
    fn column() -> Props {
        Props {
            e: 25e6,
            g: 25e6 / 2.4,
            a: 0.18,
            i22: 0.00135,
            i33: 0.0054,
            pd: -3000.0,
            ..props(3.0)
        }
    }

    #[test]
    fn geometric_stiffness_is_the_consistent_one() {
        // Cubic shapes: pd / 30 l [36, 3l, -36, 3l; 3l, 4l², -3l, -l²; ...]; r2 is minus the slope in the
        // 1-3 plane.
        let mut p = props(3.0);
        p.pd = -300.0;
        let k = plain_geometric(&p);
        let c = p.pd / 90.0;
        approx(k[1][1], 36.0 * c);
        approx(k[1][5], 9.0 * c);
        approx(k[1][7], -36.0 * c);
        approx(k[5][5], 36.0 * c);
        approx(k[5][11], -9.0 * c);
        approx(k[2][4], -9.0 * c);
        approx(k[4][4], 36.0 * c);
        approx(k[0][0], 0.0);
        // Rigid-body translation gives no force.
        for r in 0..12 {
            approx(k[r][1] + k[r][7], 0.0);
            approx(k[r][2] + k[r][8], 0.0);
        }
    }

    #[test]
    fn p_delta_cantilevers_match_the_probe() {
        // docs/phases/P_DELTA_PLAN.md, probe 1 (the reference tip deflections, 10 kN on I33).
        let p = column();
        assert!((tip(&p, 10.0) / 7.2466411332e-4 - 1.0).abs() < 1e-10);
        // With shear deformation (As 5/6 A): the Timoshenko element's shapes.
        let p2 = Props { as2: 0.15, ..p };
        assert!((tip(&p2, 10.0) / 7.4679299451e-4 - 1.0).abs() < 1e-10);
        // End offsets 0.5 m, rigid: the flexible part's geometric stiffness and the arms as bars.
        let p3 = Props {
            ri: 0.5,
            rj: 0.5,
            ri3: 0.5,
            rj3: 0.5,
            oi: 0.5,
            oj: 0.5,
            ..p
        };
        assert!((tip(&p3, 10.0) / 4.0496241011e-4 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn p_delta_moments_follow_the_members_own_shape() {
        // A cantilever with end i fixed: M3(x) gets pd (v2(x) - v2(0)), v2 the cubic shape of the tip's
        // deflection and turn; at the tip pd times the tip deflection.
        let p = column();
        let mut ul = [0.0; 12];
        ul[7] = 0.01;
        ul[11] = 0.004;
        let [m2, m3] = pdelta_moments(&p, &ul, &Hinges::default(), 3.0);
        approx(m2, 0.0);
        approx(m3, -3000.0 * 0.01);
        let [_, mid] = pdelta_moments(&p, &ul, &Hinges::default(), 1.5);
        approx(mid, -3000.0 * (0.01 * 0.5 + 0.004 * 3.0 * (0.125 - 0.25)));
    }
}
