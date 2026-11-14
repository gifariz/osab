//! Plane geometry: 2D points in an area's plane, the plane itself, polygons and segments.

use osab_solver::axes::{self, Axes, Vec3};

pub type P2 = [f64; 2];

pub fn sub(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn add(a: P2, b: P2) -> P2 {
    [a[0] + b[0], a[1] + b[1]]
}

pub fn scale(a: P2, s: f64) -> P2 {
    [a[0] * s, a[1] * s]
}

pub fn dot(a: P2, b: P2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// z of the 3D cross product.
pub fn cross(a: P2, b: P2) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

pub fn norm(a: P2) -> f64 {
    dot(a, a).sqrt()
}

pub fn dist(a: P2, b: P2) -> f64 {
    norm(sub(a, b))
}

pub fn lerp(a: P2, b: P2, t: f64) -> P2 {
    add(a, scale(sub(b, a), t))
}

/// Twice the signed area of triangle a b c (> 0 counterclockwise).
pub fn orient(a: P2, b: P2, c: P2) -> f64 {
    cross(sub(b, a), sub(c, a))
}

/// Signed area of a polygon (> 0 counterclockwise).
pub fn area(p: &[P2]) -> f64 {
    let n = p.len();
    (0..n).map(|k| cross(p[k], p[(k + 1) % n])).sum::<f64>() / 2.0
}

/// Distance from `p` to segment a-b.
pub fn seg_dist(p: P2, a: P2, b: P2) -> f64 {
    let d = sub(b, a);
    let l2 = dot(d, d);
    if l2 == 0.0 {
        return dist(p, a);
    }
    let t = (dot(sub(p, a), d) / l2).clamp(0.0, 1.0);
    dist(p, lerp(a, b, t))
}

/// Inside test for a polygon (even-odd rule); points on the boundary count as inside when within `tol`.
pub fn inside(p: P2, poly: &[P2], tol: f64) -> bool {
    let n = poly.len();
    if (0..n).any(|k| seg_dist(p, poly[k], poly[(k + 1) % n]) <= tol) {
        return true;
    }
    let mut c = false;
    for k in 0..n {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                c = !c;
            }
        }
    }
    c
}

/// Parameters (t on a-b, u on c-d) where two segments cross, if they do (ends included within `tol`
/// relative to the segment lengths). Parallel segments give None.
pub fn seg_cross(a: P2, b: P2, c: P2, d: P2, tol: f64) -> Option<(f64, f64)> {
    let r = sub(b, a);
    let s = sub(d, c);
    let den = cross(r, s);
    if den.abs() <= 1e-12 * norm(r) * norm(s) {
        return None;
    }
    let t = cross(sub(c, a), s) / den;
    let u = cross(sub(c, a), r) / den;
    let (tt, tu) = (tol / norm(r), tol / norm(s));
    (t >= -tt && t <= 1.0 + tt && u >= -tu && u <= 1.0 + tu).then_some((t, u))
}

/// An area's plane: origin, local axes (rows 1, 2, 3 = normal) and the conversions to 2D (u, v) along
/// local 1 and 2.
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub origin: Vec3,
    pub axes: Axes,
}

impl Plane {
    pub fn to2(&self, p: Vec3) -> P2 {
        let d = axes::sub(p, self.origin);
        [axes::dot(d, self.axes[0]), axes::dot(d, self.axes[1])]
    }

    pub fn to3(&self, p: P2) -> Vec3 {
        axes::add(
            self.origin,
            axes::add(
                axes::scale(self.axes[0], p[0]),
                axes::scale(self.axes[1], p[1]),
            ),
        )
    }

    /// Distance of a 3D point from the plane.
    pub fn offset(&self, p: Vec3) -> f64 {
        axes::dot(axes::sub(p, self.origin), self.axes[2]).abs()
    }
}

pub fn dist3(a: Vec3, b: Vec3) -> f64 {
    let d = axes::sub(a, b);
    axes::dot(d, d).sqrt()
}

/// Position of `p` along segment a-b (0 at a, 1 at b) when it lies on the segment's line within `tol`,
/// else None.
pub fn on_segment3(p: Vec3, a: Vec3, b: Vec3, tol: f64) -> Option<f64> {
    let d = axes::sub(b, a);
    let l2 = axes::dot(d, d);
    if l2 == 0.0 {
        return None;
    }
    let t = axes::dot(axes::sub(p, a), d) / l2;
    let q = axes::add(a, axes::scale(d, t));
    (dist3(p, q) <= tol).then_some(t)
}
