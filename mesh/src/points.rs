//! The model's points: object joints first, then the points the mesh step adds. Points closer than the
//! merge tolerance are one point, so meshes of neighbouring objects share their joints.

use std::collections::HashMap;

use osab_solver::axes::Vec3;

use crate::geom::dist3;

pub struct Points {
    pub xyz: Vec<Vec3>,
    /// Index of the object joint at this point, if one is there.
    pub joint: Vec<Option<usize>>,
    tol: f64,
    cell: f64,
    cells: HashMap<[i64; 3], Vec<usize>>,
}

impl Points {
    /// `tol`: merge tolerance (model length units).
    pub fn new(tol: f64) -> Self {
        Points {
            xyz: Vec::new(),
            joint: Vec::new(),
            tol,
            cell: 4.0 * tol,
            cells: HashMap::new(),
        }
    }

    fn key(&self, p: Vec3) -> [i64; 3] {
        p.map(|v| (v / self.cell).floor() as i64)
    }

    /// The point within the tolerance of `p`, if there is one.
    pub fn find(&self, p: Vec3) -> Option<usize> {
        let k = self.key(p);
        let mut best: Option<(f64, usize)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(list) = self.cells.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else {
                        continue;
                    };
                    for &i in list {
                        let d = dist3(self.xyz[i], p);
                        if d <= self.tol && best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, i));
                        }
                    }
                }
            }
        }
        best.map(|(_, i)| i)
    }

    /// The point at `p` (an existing one within the tolerance, else a new one).
    pub fn add(&mut self, p: Vec3) -> usize {
        if let Some(i) = self.find(p) {
            return i;
        }
        let i = self.xyz.len();
        self.xyz.push(p);
        self.joint.push(None);
        let k = self.key(p);
        self.cells.entry(k).or_default().push(i);
        i
    }

    /// Adds an object joint (or marks the point already there as that joint).
    pub fn add_joint(&mut self, p: Vec3, joint: usize) -> usize {
        let i = self.add(p);
        if self.joint[i].is_none() {
            self.joint[i] = Some(joint);
        }
        i
    }

    pub fn len(&self) -> usize {
        self.xyz.len()
    }
}
