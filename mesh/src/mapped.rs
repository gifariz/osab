//! Mapped mesh: a face with four straight sides whose opposite sides have the same number of pieces
//! gets a structured quad grid (transfinite interpolation). On a rectangle with equally divided sides
//! this is exactly the equal grid.

use crate::geom::{self, P2};

/// Indices (into `loop_`) of the corners of a closed counterclockwise boundary: the points where it
/// turns by more than `min_turn` radians.
pub fn corners(loop_: &[P2], min_turn: f64) -> Vec<usize> {
    let n = loop_.len();
    (0..n)
        .filter(|&k| {
            let a = geom::sub(loop_[k], loop_[(k + n - 1) % n]);
            let b = geom::sub(loop_[(k + 1) % n], loop_[k]);
            let turn = geom::cross(a, b).atan2(geom::dot(a, b));
            turn.abs() > min_turn
        })
        .collect()
}

/// Interior grid of a four-sided face. `loop_` is the face's boundary (counterclockwise, every point on
/// it); returns the grid's points (boundary points first, in `loop_` order, then new interior points)
/// and its quads (counterclockwise, indices into the points), or None when the face is not four-sided
/// with matching opposite sides.
pub fn mapped(loop_: &[P2]) -> Option<(Vec<P2>, Vec<[usize; 4]>)> {
    let n = loop_.len();
    let c = corners(loop_, 1e-6);
    if c.len() != 4 {
        return None;
    }
    // Every turn must be to the left (convex face).
    for &k in &c {
        let a = geom::sub(loop_[k], loop_[(k + n - 1) % n]);
        let b = geom::sub(loop_[(k + 1) % n], loop_[k]);
        if geom::cross(a, b) <= 0.0 {
            return None;
        }
    }
    // Sides as index runs from one corner to the next.
    let side = |s: usize| -> Vec<usize> {
        let (a, b) = (c[s], c[(s + 1) % 4]);
        let len = (b + n - a) % n;
        (0..=len).map(|k| (a + k) % n).collect()
    };
    let (s0, s1, s2, s3) = (side(0), side(1), side(2), side(3));
    let (nu, nv) = (s0.len() - 1, s1.len() - 1);
    if s2.len() - 1 != nu || s3.len() - 1 != nv {
        return None;
    }
    // Grid (i along s0, j along s1): bottom = s0, right = s1, top = s2 reversed, left = s3 reversed.
    let bottom: Vec<usize> = s0.clone();
    let top: Vec<usize> = s2.iter().rev().copied().collect();
    let right: Vec<usize> = s1.clone();
    let left: Vec<usize> = s3.iter().rev().copied().collect();
    let param = |run: &[usize]| -> Vec<f64> {
        let mut s = vec![0.0];
        for w in run.windows(2) {
            s.push(s.last().unwrap() + geom::dist(loop_[w[0]], loop_[w[1]]));
        }
        let t = *s.last().unwrap();
        s.iter().map(|v| v / t).collect()
    };
    let (pb, pt, pl, pr) = (param(&bottom), param(&top), param(&left), param(&right));
    let mut pts: Vec<P2> = loop_.to_vec();
    let mut id = vec![vec![usize::MAX; nv + 1]; nu + 1];
    for i in 0..=nu {
        id[i][0] = bottom[i];
        id[i][nv] = top[i];
    }
    id[0].copy_from_slice(&left);
    id[nu].copy_from_slice(&right);
    let (c00, c10, c11, c01) = (
        loop_[bottom[0]],
        loop_[bottom[nu]],
        loop_[top[nu]],
        loop_[top[0]],
    );
    for i in 1..nu {
        for j in 1..nv {
            let xi = (pb[i] + pt[i]) / 2.0;
            let eta = (pl[j] + pr[j]) / 2.0;
            let (b, t, l, r) = (
                loop_[bottom[i]],
                loop_[top[i]],
                loop_[left[j]],
                loop_[right[j]],
            );
            let mut p = [0.0; 2];
            for d in 0..2 {
                p[d] = (1.0 - eta) * b[d] + eta * t[d] + (1.0 - xi) * l[d] + xi * r[d]
                    - ((1.0 - xi) * (1.0 - eta) * c00[d]
                        + xi * (1.0 - eta) * c10[d]
                        + xi * eta * c11[d]
                        + (1.0 - xi) * eta * c01[d]);
            }
            id[i][j] = pts.len();
            pts.push(p);
        }
    }
    let mut quads = Vec::with_capacity(nu * nv);
    for i in 0..nu {
        for j in 0..nv {
            quads.push([id[i][j], id[i + 1][j], id[i + 1][j + 1], id[i][j + 1]]);
        }
    }
    Some((pts, quads))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 5 x 4 rectangle with 4 x 4 equal pieces: the grid of probe case f_rect.
    #[test]
    fn rectangle_gives_the_equal_grid() {
        let mut loop_ = Vec::new();
        for k in 0..4 {
            loop_.push([1.25 * k as f64, 0.0]);
        }
        for k in 0..4 {
            loop_.push([5.0, k as f64]);
        }
        for k in 0..4 {
            loop_.push([5.0 - 1.25 * k as f64, 4.0]);
        }
        for k in 0..4 {
            loop_.push([0.0, 4.0 - k as f64]);
        }
        let (pts, quads) = mapped(&loop_).unwrap();
        assert_eq!(quads.len(), 16);
        assert_eq!(pts.len(), 25);
        for q in &quads {
            let p: Vec<P2> = q.iter().map(|&i| pts[i]).collect();
            assert!((geom::area(&p) - 1.25).abs() < 1e-12);
        }
    }

    #[test]
    fn unequal_sides_are_not_mapped() {
        let loop_ = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]];
        assert!(mapped(&loop_).is_none());
    }
}
