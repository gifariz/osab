//! Story results (docs/phases/RESPONSE_SPECTRUM_PLAN.md, step 6), for models with story levels: joint
//! drifts, story drifts and story forces.
//!
//! - A joint drift is (u - u below) / height between them in X and Y, for a joint at a story level (mesh
//!   joints left out) that has a joint below it: at the same plan position on the level below, or else
//!   the lower end of a frame object going down from it (wherever that end is; a frame the mesh step
//!   split counts as the object, so a joint partway along it has no such frame).
//! - A story drift is the largest joint drift of the story in each direction (by size).
//! - Story forces are the forces the structure below a horizontal cut puts on the part above it, about
//!   the point (0, 0) at the level's height: P (up), VX, VY, T, MX, MY, at the top of each story (a cut
//!   just below its level) and at its bottom (just above the level below). Elements crossing the cut give
//!   them: each one's forces on its joints above the cut, less its own loads there, and its loads above
//!   the cut, with their signs reversed (an element's own loads stay inside it: a wall's weight is all
//!   below the top cut and all above the bottom one). An element counts only in the story holding its
//!   highest joint, and only at a cut whose level has one of its joints: a column from the base to the
//!   second level gives the top of the second story and nothing else. For a frame the mesh step split,
//!   those are the object's joints, whatever joints its pieces have: the same column split at the first
//!   level still gives only the top of the second story, from the piece that the cut crosses.
//! - With P-Delta from loads an element's forces no longer balance in moment (by the P-Delta moment of
//!   its geometric stiffness), and the story forces are the members' internal forces at the cut: a
//!   frame's at the cut (its P-Delta moment along it included, `ElementResults::forces_at`); a shell's
//!   from its joints above the top cut and from its joints below the bottom cut (the joints at the
//!   story's levels, where its forces are).

use std::collections::HashMap;

use serde::Serialize;

use crate::axes;
use crate::elements::{self, LocalLoad};
use crate::model::{Id, Model};

/// Drift of one joint against the joint below it.
#[derive(Debug, Clone, Serialize)]
pub struct JointDrift {
    pub story: String,
    pub node: Id,
    pub below: Id,
    /// X and Y: (u - u below) / story height.
    pub drift: [f64; 2],
}

/// The largest joint drift of a story in one direction (by size) and its joint.
#[derive(Debug, Clone, Serialize)]
pub struct StoryDrift {
    pub story: String,
    pub dir: String,
    pub drift: f64,
    pub node: Id,
}

/// P, VX, VY, T, MX, MY at the top and the bottom of a story.
#[derive(Debug, Clone, Serialize)]
pub struct StoryForce {
    pub story: String,
    pub top: [f64; 6],
    pub bottom: [f64; 6],
}

pub const FORCE_NAMES: [&str; 6] = ["P", "VX", "VY", "T", "MX", "MY"];

/// The model's story levels as the analysis uses them.
pub struct Stories {
    names: Vec<String>,
    /// Cuts of each story above the base, top then bottom: (cut height, level height).
    cuts: Vec<[(f64, f64); 2]>,
    /// (story, joint, joint below, height between them).
    pairs: Vec<(usize, usize, usize, f64)>,
    /// The lowest and the highest end of each frame's object (a frame the mesh step split: all its pieces).
    objects: Vec<[f64; 2]>,
    /// Heights closer than this are the same.
    tol: f64,
}

impl Stories {
    /// None without at least two levels (the base and one story).
    pub fn new(model: &Model) -> Option<Stories> {
        let levels = &model.stories;
        if levels.len() < 2 {
            return None;
        }
        let top = levels.iter().fold(1.0f64, |a, s| a.max(s.z.abs()));
        let tol = 1e-6 * top;
        let level_of = |z: f64| levels.iter().position(|s| (s.z - z).abs() <= tol);
        let key =
            |l: usize, x: f64, y: f64| (l, (x / tol).round() as i64, (y / tol).round() as i64);
        let object = |n: usize| !model.nodes[n].id.0.starts_with('~');
        let mut at: HashMap<(usize, i64, i64), usize> = HashMap::new();
        for (n, node) in model.nodes.iter().enumerate() {
            if let Some(l) = level_of(node.z).filter(|_| object(n)) {
                at.entry(key(l, node.x, node.y)).or_insert(n);
            }
        }
        let index: HashMap<&Id, usize> = model
            .nodes
            .iter()
            .enumerate()
            .map(|(n, node)| (&node.id, n))
            .collect();
        // The lowest and the highest joint of each frame object (a frame the mesh step split: of all its
        // pieces), in the order of the frames.
        let object_of =
            |f: &'_ crate::model::Frame| f.object.clone().unwrap_or_else(|| f.id.clone());
        let mut seen: HashMap<Id, usize> = HashMap::new();
        let mut ends: Vec<[usize; 2]> = Vec::new();
        let mut of_frame = Vec::with_capacity(model.frames.len());
        for f in &model.frames {
            let k = *seen.entry(object_of(f)).or_insert_with(|| {
                ends.push([index[&f.nodes[0]]; 2]);
                ends.len() - 1
            });
            of_frame.push(k);
            for id in &f.nodes {
                let n = index[id];
                if model.nodes[n].z < model.nodes[ends[k][0]].z {
                    ends[k][0] = n;
                }
                if model.nodes[n].z > model.nodes[ends[k][1]].z {
                    ends[k][1] = n;
                }
            }
        }
        // The lower end of the first frame object going down from each joint.
        let mut down: HashMap<usize, usize> = HashMap::new();
        for &[lo, hi] in &ends {
            if model.nodes[hi].z - model.nodes[lo].z > tol {
                down.entry(hi).or_insert(lo);
            }
        }
        let mut pairs = Vec::new();
        for (n, node) in model.nodes.iter().enumerate() {
            let Some(l) = level_of(node.z).filter(|l| *l > 0 && object(n)) else {
                continue;
            };
            if let Some(&b) = at.get(&key(l - 1, node.x, node.y)) {
                pairs.push((l - 1, n, b, levels[l].z - levels[l - 1].z));
            } else if let Some(&b) = down.get(&n) {
                pairs.push((l - 1, n, b, node.z - model.nodes[b].z));
            }
        }
        pairs.sort_by_key(|p| (std::cmp::Reverse(p.0), p.1));
        let objects = of_frame
            .iter()
            .map(|&k| ends[k].map(|n: usize| model.nodes[n].z))
            .collect();
        let cuts = levels
            .windows(2)
            .map(|w| {
                let d = 1e-6 * (w[1].z - w[0].z);
                [(w[1].z - d, w[1].z), (w[0].z + d, w[0].z)]
            })
            .collect();
        Some(Stories {
            names: levels[1..].iter().map(|s| s.id.to_string()).collect(),
            cuts,
            pairs,
            objects,
            tol,
        })
    }

    /// The heights of the ends of frame `i`'s object.
    pub fn object_ends(&self, i: usize) -> [f64; 2] {
        self.objects[i]
    }

    /// Whether an element with joints at heights `zs` gives forces at cut `k` (at height `zc`, of the
    /// level at `z`): it crosses the cut, and its object (joints at heights `object`) has its highest
    /// joint in the cut's story and a joint at the cut's level.
    fn counts(&self, k: usize, zc: f64, z: f64, zs: &[f64], object: &[f64]) -> bool {
        let lo = zs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = zs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let highest = object.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let [(_, top), (_, bottom)] = self.cuts[k / 2];
        lo < zc
            && zc < hi
            && highest > bottom + self.tol
            && highest <= top + self.tol
            && object.iter().any(|h| (h - z).abs() <= self.tol)
    }

    /// The joint drifts of displacements `u` (6 per joint), the highest story first.
    pub fn drifts(&self, model: &Model, u: &[f64]) -> Vec<JointDrift> {
        self.pairs
            .iter()
            .map(|&(s, n, b, h)| JointDrift {
                story: self.names[s].clone(),
                node: model.nodes[n].id.clone(),
                below: model.nodes[b].id.clone(),
                drift: [0, 1].map(|d| (u[n * 6 + d] - u[b * 6 + d]) / h),
            })
            .collect()
    }

    /// Room for the story forces: two cuts per story.
    pub fn zero(&self) -> Vec<[f64; 6]> {
        vec![[0.0; 6]; 2 * self.cuts.len()]
    }

    /// Adds an element's part to the story forces: `at` holds each joint's position and the force and
    /// moment the joint puts on the element (global); `object` the heights of its object's joints;
    /// `loads_above(zc, z)` the force and the moment about (0, 0, z) of the element's own loads above
    /// height zc.
    /// `couple`, for an element whose forces carry P-Delta: the moment (global) of all its joint forces
    /// and loads, which do not balance; the bottom cuts then take the part below the cut (all less the
    /// part above).
    pub fn add(
        &self,
        acc: &mut [[f64; 6]],
        at: &[([f64; 3], [f64; 6])],
        object: &[f64],
        loads_above: &dyn Fn(f64, f64) -> ([f64; 3], [f64; 3]),
        couple: Option<[f64; 3]>,
    ) {
        let zs: Vec<f64> = at.iter().map(|a| a.0[2]).collect();
        for (k, &(zc, z)) in self.cuts.iter().flatten().enumerate() {
            if !self.counts(k, zc, z, &zs, object) {
                continue;
            }
            let (mut f, mut m) = loads_above(zc, z);
            if let (Some(c), 1) = (couple, k % 2) {
                m = axes::sub(m, c);
            }
            for (p, g) in at.iter().filter(|a| a.0[2] > zc) {
                let gf = [g[0], g[1], g[2]];
                f = axes::add(f, gf);
                let r = [p[0], p[1], p[2] - z];
                m = axes::add(m, axes::add([g[3], g[4], g[5]], axes::cross(r, gf)));
            }
            let x = &mut acc[k];
            for (a, v) in [f[2], f[0], f[1], m[2], m[0], m[1]].into_iter().enumerate() {
                x[a] -= v;
            }
        }
    }

    /// Adds a frame's part to the story forces from its internal forces at each cut it crosses (its
    /// ends at heights `lo`, `hi`, those of its object `object`): `section(zc, z)` gives the force and
    /// the moment about (0, 0, z) that the part below the cut puts on the part above.
    pub fn add_section(
        &self,
        acc: &mut [[f64; 6]],
        lo: f64,
        hi: f64,
        object: &[f64],
        section: &dyn Fn(f64, f64) -> ([f64; 3], [f64; 3]),
    ) {
        for (k, &(zc, z)) in self.cuts.iter().flatten().enumerate() {
            if !self.counts(k, zc, z, &[lo, hi], object) {
                continue;
            }
            let (f, m) = section(zc, z);
            let x = &mut acc[k];
            for (a, v) in [f[2], f[0], f[1], m[2], m[0], m[1]].into_iter().enumerate() {
                x[a] += v;
            }
        }
    }

    /// The story forces from the cuts' sums.
    pub fn forces(&self, acc: &[[f64; 6]]) -> Vec<StoryForce> {
        self.names
            .iter()
            .enumerate()
            .map(|(s, name)| StoryForce {
                story: name.clone(),
                top: acc[2 * s],
                bottom: acc[2 * s + 1],
            })
            .collect()
    }
}

/// Force and moment about (0, 0, z) of a frame's loads above the level at height `z` (split at the
/// level itself, so a member ending there has none above it): the frame starts at `start` with local
/// axes `axes`, length `l`, and goes up (end i is the lower end).
pub fn frame_loads_above(
    start: [f64; 3],
    axes3: &[[f64; 3]; 3],
    l: f64,
    loads: &[LocalLoad],
    z: f64,
) -> ([f64; 3], [f64; 3]) {
    let up = axes3[0][2];
    if loads.is_empty() || up <= 0.0 {
        return ([0.0; 3], [0.0; 3]);
    }
    let from = ((z - start[2]) / up).clamp(0.0, l);
    let r0 = [start[0], start[1], start[2] - z];
    let (mut f, mut m) = ([0.0; 3], [0.0; 3]);
    for ld in loads {
        let (dir, i0, i1) = match *ld {
            LocalLoad::Point { dir, p, at } if at >= from => (dir, p, p * at),
            LocalLoad::Point { .. } => continue,
            LocalLoad::Distributed {
                dir,
                w1,
                w2,
                from: a,
                to: b,
            } => {
                let (i0, i1) = elements::distributed_part(w1, w2, a, b, from, l);
                (dir, i0, i1)
            }
        };
        let g = axes::scale(axes3[dir], i0);
        f = axes::add(f, g);
        m = axes::add(
            m,
            axes::add(
                axes::cross(r0, g),
                axes::scale(axes::cross(axes3[0], axes3[dir]), i1),
            ),
        );
    }
    (f, m)
}

/// Force and moment about (0, 0, z) of an element's own loads, given as forces at its joints (`at`:
/// position and force), when its centre lies above height `zc`.
pub fn joint_loads_above(at: &[([f64; 3], [f64; 6])], zc: f64, z: f64) -> ([f64; 3], [f64; 3]) {
    let centre = at.iter().map(|a| a.0[2]).sum::<f64>() / at.len().max(1) as f64;
    let (mut f, mut m) = ([0.0; 3], [0.0; 3]);
    if centre > zc {
        for (p, g) in at {
            let gf = [g[0], g[1], g[2]];
            f = axes::add(f, gf);
            let r = [p[0], p[1], p[2] - z];
            m = axes::add(m, axes::add([g[3], g[4], g[5]], axes::cross(r, gf)));
        }
    }
    (f, m)
}

/// The largest joint drift of each story in X and Y, by size (the first joint of equal ones).
pub fn story_drifts(joints: &[JointDrift]) -> Vec<StoryDrift> {
    let mut out: Vec<StoryDrift> = Vec::new();
    for j in joints {
        for (d, dir) in ["X", "Y"].into_iter().enumerate() {
            let v = j.drift[d].abs();
            match out.iter_mut().find(|s| s.story == j.story && s.dir == dir) {
                Some(s) if v > s.drift => {
                    s.drift = v;
                    s.node = j.node.clone();
                }
                Some(_) => {}
                None => out.push(StoryDrift {
                    story: j.story.clone(),
                    dir: dir.into(),
                    drift: v,
                    node: j.node.clone(),
                }),
            }
        }
    }
    out
}
