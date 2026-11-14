//! Area geometry: corner order, flatness and shape checks, local plane coordinates.

use crate::axes::{self, Axes, Vec3};

/// Corners may lie this far (relative to the area's size) off the area's plane.
const FLAT_TOL: f64 = 1e-6;

/// A warped quad (corners off one plane by more than `FLAT_TOL`) is analysed as the flat element on its
/// mean plane (through the corner centroid, normal from the lines joining opposite mid-sides, as the
/// local axes), with rigid arms along the normal from each joint to its corner's projection there
/// (docs/SOURCES.md). Returns those arms, or None for a flat area or a triangle.
pub fn warp_arms(p: &[Vec3], ax: &Axes) -> Option<Vec<Vec3>> {
    if p.len() != 4 {
        return None;
    }
    let c = axes::scale(p.iter().fold([0.0; 3], |s, q| axes::add(s, *q)), 0.25);
    let h: Vec<f64> = p
        .iter()
        .map(|q| axes::dot(axes::sub(*q, c), ax[2]))
        .collect();
    if h.iter().all(|v| v.abs() <= FLAT_TOL * size(p)) {
        return None;
    }
    Some(h.iter().map(|v| axes::scale(ax[2], -v)).collect())
}

/// How far a quad's corners lie off its mean plane (the largest distance), and the twist: the largest angle
/// (degrees) between the normals at two corners (each normal square to the two sides meeting there).
pub fn warp(p: &[Vec3]) -> (f64, f64) {
    if p.len() != 4 {
        return (0.0, 0.0);
    }
    let ax = axes::area_axes(p, 0.0);
    let c = axes::scale(p.iter().fold([0.0; 3], |s, q| axes::add(s, *q)), 0.25);
    let off = p
        .iter()
        .map(|q| axes::dot(axes::sub(*q, c), ax[2]).abs())
        .fold(0.0, f64::max);
    let normals: Vec<Vec3> = (0..4)
        .map(|k| {
            let n = axes::cross(
                axes::sub(p[(k + 1) % 4], p[k]),
                axes::sub(p[(k + 3) % 4], p[k]),
            );
            axes::scale(n, 1.0 / axes::dot(n, n).sqrt())
        })
        .collect();
    let mut twist = 0.0f64;
    for i in 0..4 {
        for j in i + 1..4 {
            twist = twist.max(
                axes::dot(normals[i], normals[j])
                    .clamp(-1.0, 1.0)
                    .acos()
                    .to_degrees(),
            );
        }
    }
    (off, twist)
}

/// Size of an area: the largest distance between two corners (a quad's longer diagonal, a
/// triangle's longest side).
fn size(p: &[Vec3]) -> f64 {
    let d = |a: Vec3, b: Vec3| axes::dot(axes::sub(a, b), axes::sub(a, b)).sqrt();
    let mut s = 0.0f64;
    for i in 0..p.len() {
        for j in i + 1..p.len() {
            s = s.max(d(p[i], p[j]));
        }
    }
    s
}

/// An area's corners are analysed starting at the first corner (in the given order) at the lowest Z;
/// the direction is kept. Returns that corner's index.
pub fn first_corner(p: &[Vec3]) -> usize {
    let low = p.iter().fold(f64::INFINITY, |m, q| m.min(q[2]));
    let tol = 1e-9 * size(p).max(1.0);
    p.iter().position(|q| q[2] - low <= tol).unwrap_or(0)
}

/// Corner coordinates in the area's local 1-2 plane, relative to the corner centroid.
pub fn local_xy(p: &[Vec3], ax: &Axes) -> Vec<[f64; 2]> {
    let c = axes::scale(
        p.iter().fold([0.0; 3], |s, q| axes::add(s, *q)),
        1.0 / p.len() as f64,
    );
    p.iter()
        .map(|q| {
            let d = axes::sub(*q, c);
            [axes::dot(d, ax[0]), axes::dot(d, ax[1])]
        })
        .collect()
}

/// Inside angle (degrees) at each corner, in the area's plane.
fn corner_angles(xy: &[[f64; 2]]) -> Vec<f64> {
    let n = xy.len();
    let mut out = vec![0.0; n];
    for k in 0..n {
        let (prev, cur, next) = (xy[(k + n - 1) % n], xy[k], xy[(k + 1) % n]);
        let a = [prev[0] - cur[0], prev[1] - cur[1]];
        let b = [next[0] - cur[0], next[1] - cur[1]];
        // Counterclockwise corners: the inside angle turns from b to a.
        let ang = (b[0] * a[1] - b[1] * a[0]).atan2(b[0] * a[0] + b[1] * a[1]);
        out[k] = ang.to_degrees();
    }
    out
}

/// Why a 4- or 3-corner area cannot be analysed (not convex, degenerate), or None.
pub fn shape_problem(p: &[Vec3]) -> Option<String> {
    let s = size(p);
    if !(s > 1e-9) {
        return Some("its corners are (almost) at the same point".into());
    }
    if p.len() == 3 {
        let n = axes::cross(axes::sub(p[1], p[0]), axes::sub(p[2], p[0]));
        if !(axes::dot(n, n).sqrt() > 1e-9 * s * s) {
            return Some("its corners are in one line".into());
        }
        return None;
    }
    // Quads need not be flat: a warped quad is analysed on its mean plane (`warp_arms`).
    let ax = axes::area_axes(p, 0.0);
    let angles = corner_angles(&local_xy(p, &ax));
    if angles.iter().any(|a| !(*a > 1e-6 && *a < 180.0 - 1e-6)) {
        return Some(
            "it is not convex (every corner angle must be below 180 degrees, and corners must not cross)"
                .into(),
        );
    }
    None
}

/// Shape warning (common shell-element guidelines, docs/SOURCES.md): an aspect ratio above 4 (above 10 results are poor)
/// -- for a quad the distances between midpoints of opposite sides, for a triangle the longest side over
/// the shortest -- or, for quads, corner angles outside 45-135 degrees, or a warped quad's twist (the
/// largest angle between corner normals) above 30 degrees (above 45 results are poor).
pub fn shape_warning(p: &[Vec3]) -> Option<String> {
    let (_, twist) = warp(p);
    if twist > 45.0 {
        return Some(format!(
            "corners off one plane, twist {twist:.0} degrees (above 45: results are poor)"
        ));
    } else if twist > 30.0 {
        return Some(format!(
            "corners off one plane, twist {twist:.0} degrees (best below 30)"
        ));
    }
    let ax = axes::area_axes(p, 0.0);
    let xy = local_xy(p, &ax);
    let angles = corner_angles(&xy);
    let mid = |i: usize, j: usize| [(xy[i][0] + xy[j][0]) / 2.0, (xy[i][1] + xy[j][1]) / 2.0];
    let dist = |a: [f64; 2], b: [f64; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let aspect = if xy.len() == 3 {
        let sides = [dist(xy[0], xy[1]), dist(xy[1], xy[2]), dist(xy[2], xy[0])];
        sides.iter().fold(0.0f64, |m, v| m.max(*v))
            / sides.iter().fold(f64::INFINITY, |m, v| m.min(*v))
    } else {
        let d1 = dist(mid(0, 1), mid(2, 3));
        let d2 = dist(mid(1, 2), mid(3, 0));
        d1.max(d2) / d1.min(d2)
    };
    let worst = angles.iter().fold(90.0f64, |m, a| {
        if (a - 90.0).abs() > (m - 90.0).abs() {
            *a
        } else {
            m
        }
    });
    if aspect > 10.0 {
        Some(format!(
            "aspect ratio {aspect:.1} (above 10: results are poor)"
        ))
    } else if aspect > 4.0 {
        Some(format!("aspect ratio {aspect:.1} (best below 4)"))
    } else if xy.len() == 4 && !(45.0..=135.0).contains(&worst) {
        Some(format!("corner angle {worst:.0} degrees (best 45-135)"))
    } else {
        None
    }
}

/// Shape function values of a 3- or 4-corner element at the point `q`, everything in plan (x, y): area
/// coordinates on a triangle, bilinear on a quad (its natural coordinates found by Newton's method). None
/// when `q` lies off the element.
pub fn plan_shape(p: &[[f64; 2]], q: [f64; 2]) -> Option<Vec<f64>> {
    let span = p
        .iter()
        .flat_map(|c| p.iter().map(move |d| (c[0] - d[0]).hypot(c[1] - d[1])))
        .fold(0.0f64, f64::max);
    let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    if p.len() == 3 {
        let a = cross(p[0], p[1], p[2]);
        if a == 0.0 {
            return None;
        }
        let s = vec![
            cross(q, p[1], p[2]) / a,
            cross(p[0], q, p[2]) / a,
            cross(p[0], p[1], q) / a,
        ];
        let tol = 1e-9;
        return s.iter().all(|v| *v >= -tol).then_some(s);
    }
    if p.len() != 4 || span == 0.0 {
        return None;
    }
    const XI: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    let n = |s: f64, t: f64| XI.map(|c| 0.25 * (1.0 + c[0] * s) * (1.0 + c[1] * t));
    let (mut s, mut t) = (0.0f64, 0.0f64);
    for _ in 0..50 {
        let w = n(s, t);
        let (mut x, mut j, mut k) = ([0.0; 2], [0.0; 2], [0.0; 2]);
        for c in 0..4 {
            let ds = 0.25 * XI[c][0] * (1.0 + XI[c][1] * t);
            let dt = 0.25 * XI[c][1] * (1.0 + XI[c][0] * s);
            for d in 0..2 {
                x[d] += w[c] * p[c][d];
                j[d] += ds * p[c][d];
                k[d] += dt * p[c][d];
            }
        }
        let r = [q[0] - x[0], q[1] - x[1]];
        let det = j[0] * k[1] - j[1] * k[0];
        if det == 0.0 {
            return None;
        }
        let (a, b) = (
            (r[0] * k[1] - r[1] * k[0]) / det,
            (j[0] * r[1] - j[1] * r[0]) / det,
        );
        s += a;
        t += b;
        if a.abs() + b.abs() < 1e-14 {
            break;
        }
    }
    let tol = 1e-9;
    (s.abs() <= 1.0 + tol && t.abs() <= 1.0 + tol).then(|| n(s, t).to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn angle_rotates_axes_1_and_2_about_3() {
        let p = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let ax = axes::area_axes(&p, 90.0);
        let want = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        for r in 0..3 {
            for d in 0..3 {
                assert!((ax[r][d] - want[r][d]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn shape_checks() {
        let warped = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.1],
            [0.0, 1.0, 0.0],
        ];
        // A warped quad is analysed (on its mean plane); a strongly twisted one gets a warning.
        assert!(shape_problem(&warped).is_none());
        let (off, twist) = warp(&warped);
        assert!((off - 0.025).abs() < 1e-3 && twist < 30.0, "{off} {twist}");
        assert!(shape_warning(&warped).is_none());
        let twisted = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
        ];
        assert!(shape_warning(&twisted).unwrap().contains("twist"));
        let dart = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.5, 0.5, 0.0],
            [0.0, 2.0, 0.0],
        ];
        assert!(shape_problem(&dart).unwrap().contains("convex"));
        let bow = [
            [0.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        assert!(shape_problem(&bow).is_some());
        let long = [
            [0.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [5.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        assert!(shape_warning(&long).unwrap().contains("aspect"));
        let skew = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.5, 1.0, 0.0],
            [1.5, 1.0, 0.0],
        ];
        assert!(shape_warning(&skew).unwrap().contains("angle"));
        let ok = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.5, 0.0],
            [0.0, 1.5, 0.0],
        ];
        assert!(shape_warning(&ok).is_none());
    }
}
