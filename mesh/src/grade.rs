//! Grading (`refine`): smaller elements near openings and re-entrant corners of a General floor mesh.
//! The element size is half the maximum size at such a corner and grows back to the full size over two
//! elements: size(d) = h * min(1, 1/2 + d / 4h), d the distance to the nearest of these corners. Used
//! only in the irregular regions (the quad-dominant mesh); four-sided regions keep their equal grid.

use crate::geom::{self, P2};

/// The size field of one area (in its plane).
pub struct Field<'a> {
    pub h: f64,
    /// Re-entrant corners of the area and corners of its openings.
    pub hot: &'a [P2],
}

/// Samples along a segment for its graded division.
const SAMPLES: usize = 400;

impl Field<'_> {
    pub fn size(&self, p: P2) -> f64 {
        let d = self
            .hot
            .iter()
            .map(|q| geom::dist(p, *q))
            .fold(f64::INFINITY, f64::min);
        if d.is_finite() {
            self.h * (0.5 + 0.25 * d / self.h).min(1.0)
        } else {
            self.h
        }
    }

    /// Division of the segment a-b into pieces that follow the size field: the parameters (0..1) of the
    /// inner points, or None when the field is the full size all along (the equal division then).
    pub fn divide(&self, a: P2, b: P2) -> Option<Vec<f64>> {
        let len = geom::dist(a, b);
        let at = |k: usize| self.size(geom::lerp(a, b, k as f64 / SAMPLES as f64));
        let sizes: Vec<f64> = (0..=SAMPLES).map(at).collect();
        if sizes.iter().all(|&s| s >= self.h) {
            return None;
        }
        // Cumulative number of elements along the segment, F(t) = ∫ len / size dt.
        let mut f = vec![0.0];
        for k in 0..SAMPLES {
            let inv = 0.5 * (1.0 / sizes[k] + 1.0 / sizes[k + 1]);
            f.push(f[k] + inv * len / SAMPLES as f64);
        }
        let total = f[SAMPLES];
        let n = (total - 1e-9).ceil().max(1.0) as usize;
        let mut out = Vec::with_capacity(n.saturating_sub(1));
        let mut k = 0;
        for s in 1..n {
            let target = total * s as f64 / n as f64;
            while f[k + 1] < target {
                k += 1;
            }
            let w = (target - f[k]) / (f[k + 1] - f[k]);
            out.push((k as f64 + w) / SAMPLES as f64);
        }
        Some(out)
    }
}

/// The corners grading refines towards: the polygon's re-entrant corners and the corners of its holes
/// that lie in it.
pub fn hot_spots(corners: &[P2], holes: &[Vec<P2>], tol: f64) -> Vec<P2> {
    let n = corners.len();
    let sign = geom::area(corners).signum();
    let mut out: Vec<P2> = (0..n)
        .filter(|&k| {
            let a = geom::sub(corners[k], corners[(k + n - 1) % n]);
            let b = geom::sub(corners[(k + 1) % n], corners[k]);
            sign * geom::cross(a, b) < -tol * (geom::norm(a) + geom::norm(b))
        })
        .map(|k| corners[k])
        .collect();
    for hole in holes {
        out.extend(
            hole.iter()
                .copied()
                .filter(|p| geom::inside(*p, corners, tol)),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_is_half_at_a_corner_and_full_two_elements_away() {
        let hot = [[0.0, 0.0]];
        let f = Field { h: 1.0, hot: &hot };
        assert!((f.size([0.0, 0.0]) - 0.5).abs() < 1e-12);
        assert!((f.size([1.0, 0.0]) - 0.75).abs() < 1e-12);
        assert!((f.size([2.0, 0.0]) - 1.0).abs() < 1e-12);
        assert!((f.size([5.0, 0.0]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn far_segments_keep_the_equal_division_and_near_ones_get_finer_at_the_corner() {
        let hot = [[0.0, 0.0]];
        let f = Field { h: 1.0, hot: &hot };
        assert!(f.divide([3.0, 0.0], [6.0, 0.0]).is_none());
        let t = f.divide([0.0, 0.0], [4.0, 0.0]).unwrap();
        // ∫ dx / size over 0..4 = 4 ln 2 + 2 ≈ 4.77, so 5 pieces, the shortest at the corner.
        assert_eq!(t.len(), 4);
        let pieces: Vec<f64> = std::iter::once(0.0)
            .chain(t.iter().copied())
            .chain(std::iter::once(1.0))
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| 4.0 * (w[1] - w[0]))
            .collect();
        assert!(pieces.windows(2).all(|w| w[0] < w[1] + 1e-9), "{pieces:?}");
        assert!(pieces[0] < 0.75 && pieces[4] > 0.9, "{pieces:?}");
    }

    #[test]
    fn hot_spots_are_re_entrant_corners_and_opening_corners() {
        let l = [
            [0.0, 0.0],
            [8.0, 0.0],
            [8.0, 4.0],
            [4.0, 4.0],
            [4.0, 8.0],
            [0.0, 8.0],
        ];
        let hole = vec![[1.0, 5.0], [2.0, 5.0], [2.0, 7.0], [1.0, 7.0]];
        let hot = hot_spots(&l, std::slice::from_ref(&hole), 1e-9);
        assert_eq!(hot[0], [4.0, 4.0]);
        assert_eq!(&hot[1..], &hole[..]);
        // Clockwise order gives the same corner.
        let mut cw = l.to_vec();
        cw.reverse();
        assert_eq!(hot_spots(&cw, &[], 1e-9), vec![[4.0, 4.0]]);
    }
}
