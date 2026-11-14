//! Element local axes, following the common default rule of building analysis programs:
//! - local 1 runs from end i to end j;
//! - non-vertical members: local 2 lies in the vertical plane with an upward (+Z) sense;
//! - vertical members: local 2 is global +X;
//! - local 3 = 1 x 2 (always horizontal before any rotation);
//! - `angle` (degrees) rotates 2 and 3 about 1, counterclockwise when +1 points at the viewer.

pub type Vec3 = [f64; 3];

/// Rows are the unit vectors of local axes 1, 2, 3 in global coordinates.
pub type Axes = [Vec3; 3];

/// A member counts as vertical when the horizontal part of its unit axis is below this.
const VERTICAL_TOL: f64 = 1e-3;

pub fn local_axes(xi: Vec3, xj: Vec3, angle_deg: f64) -> Axes {
    let e1 = normalize(sub(xj, xi));
    let (e2, e3) = if (e1[0] * e1[0] + e1[1] * e1[1]).sqrt() < VERTICAL_TOL {
        let e2 = [1.0, 0.0, 0.0];
        (e2, normalize(cross(e1, e2)))
    } else {
        let e3 = normalize(cross(e1, [0.0, 0.0, 1.0]));
        (cross(e3, e1), e3)
    };
    let (s, c) = angle_deg.to_radians().sin_cos();
    let r2 = add(scale(e2, c), scale(e3, s));
    let r3 = add(scale(e2, -s), scale(e3, c));
    [e1, r2, r3]
}

/// Area (shell) local axes, the common rule (docs/SOURCES.md; checked on 312 reference
/// cases in `tests/area_axes_cases.json`):
/// - 3 is the normal, pointing at the viewer when corners 1-2-3 run counterclockwise; for a quad the
///   plane through the lines joining the midpoints of opposite sides;
/// - reference vector +Z, or +Y when 3 is within sine 0.001 of vertical;
/// - 1 = ref x 3 (normalised, horizontal), 2 = 3 x 1;
/// - `angle` (degrees) rotates 1 and 2 about 3, counterclockwise when +3 points at the viewer.
pub fn area_axes(p: &[Vec3], angle_deg: f64) -> Axes {
    let n = if p.len() == 4 {
        let a = sub(scale(add(p[1], p[2]), 0.5), scale(add(p[0], p[3]), 0.5));
        let b = sub(scale(add(p[2], p[3]), 0.5), scale(add(p[0], p[1]), 0.5));
        cross(a, b)
    } else {
        cross(sub(p[1], p[0]), sub(p[2], p[0]))
    };
    let e3 = normalize(n);
    let mut reference = [0.0, 0.0, 1.0];
    if dot(cross(e3, reference), cross(e3, reference)).sqrt() < VERTICAL_TOL {
        reference = [0.0, 1.0, 0.0];
    }
    let e1 = normalize(cross(reference, e3));
    let e2 = cross(e3, e1);
    let (s, c) = angle_deg.to_radians().sin_cos();
    [
        add(scale(e1, c), scale(e2, s)),
        add(scale(e1, -s), scale(e2, c)),
        e3,
    ]
}

/// Global vector -> local components.
pub fn to_local(axes: &Axes, v: Vec3) -> Vec3 {
    [dot(axes[0], v), dot(axes[1], v), dot(axes[2], v)]
}

/// Local components -> global vector.
pub fn to_global(axes: &Axes, v: Vec3) -> Vec3 {
    let mut g = [0.0; 3];
    for (k, a) in axes.iter().enumerate() {
        for d in 0..3 {
            g[d] += a[d] * v[k];
        }
    }
    g
}

/// 12-vector (two nodes x 6 DOF) global -> local.
pub fn vec12_to_local(axes: &Axes, g: &[f64; 12]) -> [f64; 12] {
    let mut l = [0.0; 12];
    for b in 0..4 {
        let v = to_local(axes, [g[3 * b], g[3 * b + 1], g[3 * b + 2]]);
        l[3 * b..3 * b + 3].copy_from_slice(&v);
    }
    l
}

/// 12-vector local -> global.
pub fn vec12_to_global(axes: &Axes, l: &[f64; 12]) -> [f64; 12] {
    let mut g = [0.0; 12];
    for b in 0..4 {
        let v = to_global(axes, [l[3 * b], l[3 * b + 1], l[3 * b + 2]]);
        g[3 * b..3 * b + 3].copy_from_slice(&v);
    }
    g
}

/// K_global = T^T k T, with T = blockdiag(R, R, R, R) and R rows = local axes.
pub fn mat12_to_global(axes: &Axes, k: &[[f64; 12]; 12]) -> [[f64; 12]; 12] {
    // T as a full matrix keeps this short and obviously correct; it is only 12x12.
    let mut t = [[0.0; 12]; 12];
    for b in 0..4 {
        for r in 0..3 {
            for c in 0..3 {
                t[3 * b + r][3 * b + c] = axes[r][c];
            }
        }
    }
    let mut kt = [[0.0; 12]; 12];
    for i in 0..12 {
        for j in 0..12 {
            kt[i][j] = (0..12).map(|m| k[i][m] * t[m][j]).sum();
        }
    }
    let mut out = [[0.0; 12]; 12];
    for i in 0..12 {
        for j in 0..12 {
            out[i][j] = (0..12).map(|m| t[m][i] * kt[m][j]).sum();
        }
    }
    out
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn scale(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn normalize(a: Vec3) -> Vec3 {
    scale(a, 1.0 / dot(a, a).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-12)
    }

    #[test]
    fn horizontal_beam_along_x() {
        let ax = local_axes([0.0; 3], [5.0, 0.0, 0.0], 0.0);
        assert!(close(ax[0], [1.0, 0.0, 0.0]));
        assert!(close(ax[1], [0.0, 0.0, 1.0]));
        assert!(close(ax[2], [0.0, -1.0, 0.0]));
    }

    #[test]
    fn horizontal_beam_along_y() {
        let ax = local_axes([0.0; 3], [0.0, 4.0, 0.0], 0.0);
        assert!(close(ax[1], [0.0, 0.0, 1.0]));
        assert!(close(ax[2], [1.0, 0.0, 0.0]));
    }

    #[test]
    fn column_up_and_down() {
        let up = local_axes([0.0; 3], [0.0, 0.0, 3.0], 0.0);
        assert!(close(up[1], [1.0, 0.0, 0.0]));
        assert!(close(up[2], [0.0, 1.0, 0.0]));
        let down = local_axes([0.0, 0.0, 3.0], [0.0; 3], 0.0);
        assert!(close(down[1], [1.0, 0.0, 0.0]));
        assert!(close(down[2], [0.0, -1.0, 0.0]));
    }

    #[test]
    fn rotated_column() {
        let ax = local_axes([0.0; 3], [0.0, 0.0, 3.0], 90.0);
        assert!(close(ax[1], [0.0, 1.0, 0.0]));
        assert!(close(ax[2], [-1.0, 0.0, 0.0]));
    }

    #[test]
    fn inclined_member_is_orthonormal_with_upward_axis_2() {
        let ax = local_axes([1.0, 2.0, 0.0], [3.0, 5.0, 6.0], 0.0);
        assert!(ax[1][2] > 0.0);
        assert!(ax[2][2].abs() < 1e-12);
        for i in 0..3 {
            for j in 0..3 {
                let expect = if i == j { 1.0 } else { 0.0 };
                assert!((dot(ax[i], ax[j]) - expect).abs() < 1e-12);
            }
        }
        assert!(close(cross(ax[0], ax[1]), ax[2]));
    }
}
