//! Results of the analysis model, told in objects: a split frame's pieces become one frame result again
//! (stations along the whole frame, the pieces kept for queries), shell elements name their area.

use std::collections::{HashMap, HashSet};

use osab_solver::elements::Side;
use osab_solver::model::{Frame, Id};
use osab_solver::results::{CaseResults, ElementResults, Results, Station};

use crate::build::Meshed;

/// One frame result from its pieces; `offsets` are the object's end length offsets: its stations cover the
/// clear length between them, though a piece's own offset may stop short of the object's (a hinge on it).
fn merge(id: &Id, pieces: Vec<ElementResults>, offsets: [f64; 2]) -> ElementResults {
    let first = &pieces[0];
    let last = &pieces[pieces.len() - 1];
    let mut end_forces = [0.0; 12];
    end_forces[..6].copy_from_slice(&first.end_forces[..6]);
    end_forces[6..].copy_from_slice(&last.end_forces[6..]);
    let mut end_displacements = [0.0; 12];
    end_displacements[..6].copy_from_slice(&first.end_displacements[..6]);
    end_displacements[6..].copy_from_slice(&last.end_displacements[6..]);
    let mut all = Vec::new();
    let mut offset = 0.0;
    for p in &pieces {
        all.extend(p.stations.iter().map(|s| {
            let mut s = *s;
            s.x += offset;
            s
        }));
        offset += p.length;
    }
    let tol = 1e-9 * offset.max(1.0);
    let (a, b) = (offsets[0], offset - offsets[1]);
    let stations = all
        .iter()
        .filter(|s| s.x >= a - tol && s.x <= b + tol)
        .copied()
        .collect();
    let mut r = ElementResults {
        element: id.clone(),
        nodes: [first.nodes[0].clone(), last.nodes[1].clone()],
        length: offset,
        offsets,
        axes: first.axes,
        stations,
        end_forces,
        loads: Vec::new(),
        end_displacements,
        hinges: osab_solver::elements::frame::Hinges {
            jumps: [first.hinges.jumps[0], last.hinges.jumps[1]],
            ..Default::default()
        },
        pdelta: None,
        enveloped: first.enveloped,
        pieces,
    };
    // A clear-length end inside a piece's flexible part (past a hinge) has no station of its own: forces
    // there, the displacement between the stations around it.
    if !r.enveloped {
        for (x, side) in [(a, Side::Right), (b, Side::Left)] {
            if r.stations.iter().any(|s| (s.x - x).abs() <= tol) {
                continue;
            }
            let lo = all.iter().rev().find(|s| s.x < x);
            let hi = all.iter().find(|s| s.x > x);
            let (Some(lo), Some(hi)) = (lo, hi) else {
                continue;
            };
            let t = (x - lo.x) / (hi.x - lo.x);
            let f = r.forces_at_side(x, side);
            let s = Station {
                x,
                P: f[0],
                V2: f[1],
                V3: f[2],
                T: f[3],
                M2: f[4],
                M3: f[5],
                ux: lo.ux + t * (hi.ux - lo.ux),
                uy: lo.uy + t * (hi.uy - lo.uy),
                uz: lo.uz + t * (hi.uz - lo.uz),
            };
            let at = r.stations.partition_point(|q| q.x < x);
            r.stations.insert(at, s);
        }
    }
    r
}

/// Tells one case's results in objects (`frames`: the object model's frames, their order kept).
pub struct Objects<'a> {
    m: &'a Meshed,
    frames: &'a [Frame],
    pieces: HashMap<&'a Id, &'a Vec<Id>>,
}

impl<'a> Objects<'a> {
    pub fn new(m: &'a Meshed, frames: &'a [Frame]) -> Self {
        Objects {
            m,
            frames,
            pieces: m.pieces.iter().map(|(f, p)| (f, p)).collect(),
        }
    }

    pub fn case(&self, c: &mut CaseResults) {
        for s in &mut c.shells {
            if let Some(p) = self.m.parent.get(&s.element) {
                s.area = p.clone();
            }
        }
        let mut by_id: HashMap<Id, ElementResults> =
            c.frames.drain(..).map(|e| (e.element.clone(), e)).collect();
        for f in self.frames {
            match self.pieces.get(&f.id) {
                Some(ids) => {
                    let parts: Vec<ElementResults> =
                        ids.iter().filter_map(|id| by_id.remove(id)).collect();
                    if parts.len() == ids.len() {
                        c.frames.push(merge(&f.id, parts, self.m.offsets[&f.id]));
                    }
                }
                None => {
                    if let Some(e) = by_id.remove(&f.id) {
                        c.frames.push(e);
                    }
                }
            }
        }
    }
}

/// The rest of the results told in objects: the mesh step's notes and the mesh.
pub fn finish(m: &Meshed, mut r: Results) -> Results {
    let mut notes = m.notes.clone();
    // Joints left out of the mesh are held where they are (reported with no displacement); the
    // solver's automatic restraints need no note for them.
    const AUTO: &str = "Restrained automatically (no element gives these DOFs stiffness): ";
    if !m.left_out.is_empty() {
        let out: HashSet<&str> = m.left_out.iter().map(|id| id.0.as_str()).collect();
        r.notes.retain_mut(|n| {
            let Some(list) = n.strip_prefix(AUTO) else {
                return true;
            };
            let kept: Vec<String> = list
                .split(", ")
                .filter(|e| e.rsplit_once(':').is_none_or(|(id, _)| !out.contains(id)))
                .map(str::to_string)
                .collect();
            *n = format!("{AUTO}{}", kept.join(", "));
            !kept.is_empty()
        });
    }
    notes.append(&mut r.notes);
    r.notes = notes;
    let info = &m.info;
    if !(info.joints.is_empty()
        && info.shells.is_empty()
        && info.frames.is_empty()
        && info.edge_constraints.is_empty())
    {
        r.mesh = Some(info.clone());
    }
    r
}
