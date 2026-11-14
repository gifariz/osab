//! Axis-aligned boxes on a uniform grid of cells, to find the boxes at a point or meeting another box
//! without comparing every pair (the mesh step's checks of which areas overlap).

use std::collections::HashMap;

use osab_solver::axes::Vec3;

/// Lowest and highest corner.
pub type Box3 = [Vec3; 2];

/// At most this many cells along the model's largest extent.
const MAX_CELLS: f64 = 512.0;
/// A box over more cells than this is not put in the cells but checked at every query.
const MAX_SPAN: i64 = 4096;

pub struct Boxes {
    boxes: Vec<Box3>,
    cell: f64,
    /// The boxes overlapping each cell, ascending.
    cells: HashMap<[i64; 3], Vec<usize>>,
    /// The boxes over too many cells, ascending.
    big: Vec<usize>,
}

impl Boxes {
    /// The cell is about the median box size, so most boxes cover a few cells.
    pub fn new(boxes: Vec<Box3>) -> Boxes {
        let side = |b: &Box3| (0..3).map(|d| b[1][d] - b[0][d]).fold(0.0, f64::max);
        let mut sides: Vec<f64> = boxes.iter().map(side).collect();
        sides.sort_by(f64::total_cmp);
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for b in &boxes {
            for d in 0..3 {
                lo[d] = lo[d].min(b[0][d]);
                hi[d] = hi[d].max(b[1][d]);
            }
        }
        let extent = (0..3).map(|d| hi[d] - lo[d]).fold(0.0, f64::max);
        let median = sides.get(sides.len() / 2).copied().unwrap_or(0.0);
        let cell = median.max(extent / MAX_CELLS);
        let cell = if cell > 0.0 && cell.is_finite() {
            cell
        } else {
            1.0
        };
        let mut grid = Boxes {
            boxes,
            cell,
            cells: HashMap::new(),
            big: Vec::new(),
        };
        for (i, b) in grid.boxes.iter().enumerate() {
            let (k0, k1) = (grid.key(b[0]), grid.key(b[1]));
            if span(k0, k1) > MAX_SPAN {
                grid.big.push(i);
                continue;
            }
            for x in k0[0]..=k1[0] {
                for y in k0[1]..=k1[1] {
                    for z in k0[2]..=k1[2] {
                        grid.cells.entry([x, y, z]).or_default().push(i);
                    }
                }
            }
        }
        grid
    }

    fn key(&self, p: Vec3) -> [i64; 3] {
        p.map(|v| (v / self.cell).floor() as i64)
    }

    /// The boxes containing `p` (boundary included), ascending.
    pub fn at(&self, p: Vec3) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .cells
            .get(&self.key(p))
            .into_iter()
            .flatten()
            .chain(&self.big)
            .copied()
            .filter(|&i| contains(&self.boxes[i], p))
            .collect();
        if !self.big.is_empty() {
            out.sort_unstable();
        }
        out
    }

    /// The boxes meeting box `i` (itself included), ascending.
    pub fn near(&self, i: usize) -> Vec<usize> {
        self.meeting(&self.boxes[i])
    }

    /// The boxes meeting `b` (touching included), ascending.
    pub fn meeting(&self, b: &Box3) -> Vec<usize> {
        let (k0, k1) = (self.key(b[0]), self.key(b[1]));
        // Over more cells than there are boxes: every box is checked instead.
        if span(k0, k1) > self.boxes.len() as i64 {
            return (0..self.boxes.len())
                .filter(|&i| meet(&self.boxes[i], b))
                .collect();
        }
        let mut out: Vec<usize> = self
            .big
            .iter()
            .copied()
            .filter(|&i| meet(&self.boxes[i], b))
            .collect();
        for x in k0[0]..=k1[0] {
            for y in k0[1]..=k1[1] {
                for z in k0[2]..=k1[2] {
                    if let Some(list) = self.cells.get(&[x, y, z]) {
                        out.extend(list.iter().copied().filter(|&i| meet(&self.boxes[i], b)));
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// The box around `points`, grown by `margin` on every side.
pub fn around(points: impl IntoIterator<Item = Vec3>, margin: f64) -> Box3 {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in points {
        for d in 0..3 {
            lo[d] = lo[d].min(p[d]);
            hi[d] = hi[d].max(p[d]);
        }
    }
    [lo.map(|v| v - margin), hi.map(|v| v + margin)]
}

/// The number of cells from key `k0` to `k1` (corners included), saturating.
fn span(k0: [i64; 3], k1: [i64; 3]) -> i64 {
    (0..3).fold(1i64, |n, d| {
        n.saturating_mul(k1[d].saturating_sub(k0[d]).saturating_add(1).max(0))
    })
}

fn contains(b: &Box3, p: Vec3) -> bool {
    (0..3).all(|d| b[0][d] <= p[d] && p[d] <= b[1][d])
}

fn meet(a: &Box3, b: &Box3) -> bool {
    (0..3).all(|d| a[0][d] <= b[1][d] && b[0][d] <= a[1][d])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_what_comparing_every_pair_finds() {
        // Boxes of very different sizes (a few large floors, many small pieces, flat ones), and queries.
        let mut s: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut rnd = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s % 1_000_000) as f64 / 1_000_000.0
        };
        let mut boxes = Vec::new();
        for i in 0..400 {
            let size = if i % 50 == 0 { 30.0 } else { 0.5 + rnd() };
            let lo = [100.0 * rnd() - 20.0, 60.0 * rnd(), 3.0 * (i % 7) as f64];
            let flat = if i % 3 == 0 { 0.0 } else { size };
            boxes.push([lo, [lo[0] + size, lo[1] + 0.7 * size, lo[2] + flat]]);
        }
        let grid = Boxes::new(boxes.clone());
        for _ in 0..300 {
            let p = [
                110.0 * rnd() - 25.0,
                70.0 * rnd(),
                3.0 * (rnd() * 7.0).floor(),
            ];
            let want: Vec<usize> = (0..boxes.len())
                .filter(|&i| contains(&boxes[i], p))
                .collect();
            assert_eq!(grid.at(p), want);
            let b = around([p, [p[0] + 3.0 * rnd(), p[1] + rnd(), p[2]]], 0.01);
            let want: Vec<usize> = (0..boxes.len()).filter(|&i| meet(&boxes[i], &b)).collect();
            assert_eq!(grid.meeting(&b), want);
        }
        // A query over the whole model (more cells than boxes) finds them all; large boxes (over more
        // cells than a box is put in) are found too.
        assert_eq!(
            grid.meeting(&[[-50.0, -10.0, -1.0], [150.0, 80.0, 30.0]]),
            (0..boxes.len()).collect::<Vec<_>>()
        );
        assert!(!grid.big.is_empty());
        // Each box meets itself, and a corner point is inside it.
        for (i, b) in boxes.iter().enumerate() {
            assert!(grid.meeting(b).contains(&i));
            assert!(grid.at(b[1]).contains(&i));
        }
        assert!(Boxes::new(Vec::new()).at([0.0; 3]).is_empty());
    }
}
