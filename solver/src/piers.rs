//! Pier and spandrel forces (docs/phases/PIER_SPANDREL_PLAN.md).
//!
//! A pier is the vertical walls and vertical frames with one pier label at a story; a spandrel the
//! vertical walls and horizontal frames with one spandrel label at a story (or on every story). Each has
//! two cuts: a pier its bottom and top (horizontal planes at its lowest and highest joint), a spandrel
//! its left and right end (vertical planes at the ends of its plan direction). The forces at a cut are
//! the sum of what the cut's joints put on the labelled elements on the material side of it (frame end
//! forces, shell joint forces K u less the element's own loads, the hidden beams of the shell's edge
//! constraints and its frame-joint springs), moved to the cut's centroid and given in the section's
//! axes: P, V2, V3 along 1, 2, 3, T about 1, M2 = -(moment about 2), M3 about 3; sign +1 at the top and
//! the right end, -1 at the bottom and the left end (P > 0 is tension).
//!
//! - **Labels**: a pier label counts on a wall whose normal is within sine 0.001 of horizontal and on a
//!   frame whose ends are within the merge tolerance (0.001 m) in plan; a spandrel label on such a wall
//!   and on a frame whose ends are at the same height within that tolerance. Others are left out.
//! - **Stories**: a pier's objects at the story holding their highest joint. A spandrel wall at the
//!   story level nearest its middle height (the base too; halfway goes up); a spandrel frame at its
//!   story like a pier's, but it makes a spandrel at the level nearest its middle only above the base.
//!   A multi-story spandrel label is one spandrel at the highest of those levels (the first story for
//!   the base). A spandrel needs all its corners within the merge tolerance of its first wall's plane
//!   (of the vertical plane through its first frame) and a joint at or below the highest level; joints
//!   above that level give it no force.
//! - **Pier axes**: 1 = +Z; 2 horizontal at the angle from the walls at the bottom cut (else the top
//!   cut): walls in the model's order, each with its corners from the first of its lowest edge, group
//!   with the first earlier wall whose normal is within 0.99 of theirs (|n . n_g| > 0.99) and whose
//!   first corner is that wall's or lies in its plane direction (|unit(v - v_g) x n_g| > 0.99); the
//!   group with the longest length at the cut (the first of equal ones) gives 2 = its first wall's
//!   horizontal direction, as an angle in (-90, 90]. Without walls at either cut, the first frame's
//!   local 2 (an angle in (-180, 180]). 3 = 1 x 2.
//! - **Spandrel axes**: the plan direction of the longest plan distance between two of its corners or
//!   frame ends (the first pair of equal ones) gives the two cuts; 1 = the horizontal direction from
//!   the left cut's centroid to the right one's, 2 = +Z, 3 = 1 x 2.
//! - **Centroids**: wall strips on the cut (length x thickness, at their middle) and frames crossing it
//!   (section area at the joint), by area. A cut touching only corners at one plan point: that point at
//!   height 0.

use std::collections::HashMap;

use serde::Serialize;

use crate::axes::{self, Vec3};
use crate::model::{coords, Id, Model};

/// Pier or spandrel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pier,
    Spandrel,
}

/// A pier or spandrel found from the objects: its label and story, the direction found from its
/// objects, and its members as analysis elements.
#[derive(Debug, Clone)]
pub struct WallSection {
    pub kind: Kind,
    pub label: Id,
    pub story: String,
    /// Pier: local axis 2; spandrel: the plan direction its two cuts lie across.
    pub dir: Vec3,
    pub areas: Vec<Id>,
    pub frames: Vec<Id>,
    /// Spandrel: joints above this height (the highest story level) give no force.
    pub roof: f64,
}

/// Forces of a pier at one story: P V2 V3 T M2 M3 at its top and bottom.
#[derive(Debug, Clone, Serialize)]
pub struct PierForce {
    pub story: String,
    pub pier: Id,
    pub top: [f64; 6],
    pub bottom: [f64; 6],
}

/// Forces of a spandrel at one story: P V2 V3 T M2 M3 at its left and right end.
#[derive(Debug, Clone, Serialize)]
pub struct SpandrelForce {
    pub story: String,
    pub spandrel: Id,
    pub left: [f64; 6],
    pub right: [f64; 6],
}

/// A pier's axis angle (local 2 from X, degrees) and the centroids of its bottom and top cut.
#[derive(Debug, Clone, Serialize)]
pub struct PierSection {
    pub story: String,
    pub pier: Id,
    pub angle: f64,
    pub bottom: Vec3,
    pub top: Vec3,
}

/// The centroids of a spandrel's left and right cut.
#[derive(Debug, Clone, Serialize)]
pub struct SpandrelSection {
    pub story: String,
    pub spandrel: Id,
    pub left: Vec3,
    pub right: Vec3,
}

pub const FORCE_NAMES: [&str; 6] = ["P", "V2", "V3", "T", "M2", "M3"];

/// The merge tolerance the labels and spandrel planes are judged with: 0.001 m.
fn merge_tol(model: &Model) -> f64 {
    0.001 / model.units.metres()
}

/// Unit normal of a polygon (Newell's method).
fn newell(p: &[Vec3]) -> Vec3 {
    let mut n = [0.0; 3];
    for k in 0..p.len() {
        let (a, b) = (p[k], p[(k + 1) % p.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    axes::normalize(n)
}

/// `a` over its length (each part divided: the 0.99 limits are judged on exactly these numbers).
fn unit(a: Vec3) -> Vec3 {
    let l = axes::dot(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

/// An angle in degrees made to lie in (-90, 90].
fn half_turn(a: f64) -> f64 {
    let a = (a + 90.0).rem_euclid(180.0) - 90.0;
    if a <= -90.0 + 1e-8 {
        90.0
    } else {
        a
    }
}

fn plan_dist2(a: Vec3, b: Vec3) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

/// A labelled object: wall corners from the first of its lowest edge (the winding kept), or a frame's
/// two ends, lower first.
struct Object<'a> {
    id: &'a Id,
    wall: bool,
    pts: Vec<Vec3>,
    /// Story holding its highest joint (index into the levels, 1 and up).
    story: usize,
    /// Level nearest its middle height (0 = the base).
    near: usize,
}

/// The piers and spandrels of an object model, the highest story first, then by label; members are
/// the objects themselves (the mesh step changes them into its elements). Labels the rules leave out
/// are named in `notes`.
pub fn sections(model: &Model, notes: &mut Vec<String>) -> Vec<WallSection> {
    if model.pier_labels.is_empty() && model.spandrel_labels.is_empty() {
        return Vec::new();
    }
    let index: HashMap<&Id, usize> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (&n.id, i))
        .collect();
    let at = |id: &Id| coords(&model.nodes[index[id]]);
    let size = model
        .nodes
        .iter()
        .flat_map(|n| [n.x.abs(), n.y.abs(), n.z.abs()])
        .fold(1.0f64, f64::max);
    let tol = 1e-9 * size;
    let merge = merge_tol(model);
    let levels: Vec<f64> = model.stories.iter().map(|s| s.z).collect();
    let flat = levels.len() < 2;
    let story_of = |zmax: f64| -> usize {
        if flat {
            return 1;
        }
        (1..levels.len())
            .find(|&i| zmax <= levels[i] + tol)
            .unwrap_or(levels.len() - 1)
    };
    let nearest = |z: f64| -> usize {
        if flat {
            return 1;
        }
        let mut best = 0;
        for (i, l) in levels.iter().enumerate().skip(1) {
            if (l - z).abs() <= (levels[best] - z).abs() + tol {
                best = i;
            }
        }
        best
    };
    let name = |i: usize| -> String {
        if flat {
            String::new()
        } else {
            model.stories[i].id.to_string()
        }
    };
    fn object<'a>(
        id: &'a Id,
        wall: bool,
        pts: Vec<Vec3>,
        at: &dyn Fn(f64, f64) -> (usize, usize),
    ) -> Object<'a> {
        let lo = pts.iter().map(|p| p[2]).fold(f64::INFINITY, f64::min);
        let hi = pts.iter().map(|p| p[2]).fold(f64::NEG_INFINITY, f64::max);
        let (story, near) = at(lo, hi);
        Object {
            id,
            wall,
            pts,
            story,
            near,
        }
    }
    let levels_of = |lo: f64, hi: f64| (story_of(hi), nearest(0.5 * (lo + hi)));
    // The labelled objects the rules accept, per kind; the others named in a note.
    let mut piers: Vec<(&Id, Object)> = Vec::new();
    let mut spandrels: Vec<(&Id, Object)> = Vec::new();
    let mut left_out: Vec<String> = Vec::new();
    for a in &model.areas {
        if a.pier.is_none() && a.spandrel.is_none() {
            continue;
        }
        let pts: Vec<Vec3> = a.nodes.iter().map(at).collect();
        let n = pts.len();
        let vertical = !a.is_null() && n >= 3 && newell(&pts)[2].abs() < 0.001;
        if !vertical {
            left_out.push(format!("area {}", a.id));
            continue;
        }
        let lo = pts.iter().map(|p| p[2]).fold(f64::INFINITY, f64::min);
        let low = |k: usize| (pts[k % n][2] - lo).abs() <= tol;
        let first = (0..n)
            .find(|&k| low(k) && low(k + 1))
            .or_else(|| (0..n).find(|&k| low(k)))
            .unwrap_or(0);
        let pts: Vec<Vec3> = (0..n).map(|k| pts[(first + k) % n]).collect();
        if let Some(p) = &a.pier {
            piers.push((p, object(&a.id, true, pts.clone(), &levels_of)));
        }
        if let Some(s) = &a.spandrel {
            spandrels.push((s, object(&a.id, true, pts, &levels_of)));
        }
    }
    for f in &model.frames {
        if f.pier.is_none() && f.spandrel.is_none() {
            continue;
        }
        let (mut i, mut j) = (at(&f.nodes[0]), at(&f.nodes[1]));
        if i[2] > j[2] {
            std::mem::swap(&mut i, &mut j);
        }
        let real = !f.is_null();
        let mut out = false;
        if let Some(p) = &f.pier {
            if real && plan_dist2(i, j).sqrt() <= merge {
                piers.push((p, object(&f.id, false, vec![i, j], &levels_of)));
            } else {
                out = true;
            }
        }
        if let Some(s) = &f.spandrel {
            if real && (j[2] - i[2]).abs() <= merge {
                spandrels.push((s, object(&f.id, false, vec![i, j], &levels_of)));
            } else {
                out = true;
            }
        }
        if out {
            left_out.push(format!("frame {}", f.id));
        }
    }
    if !left_out.is_empty() {
        notes.push(format!(
            "Pier and spandrel labels left out (piers take vertical walls and frames, spandrels vertical walls and horizontal frames): {}",
            left_out.join(", ")
        ));
    }
    let top = levels.last().copied().unwrap_or(f64::INFINITY);
    let mut found: Vec<(usize, usize, WallSection)> = Vec::new();

    // Piers: per label and story.
    for (li, label) in model.pier_labels.iter().enumerate() {
        let mine: Vec<&Object> = piers
            .iter()
            .filter(|(l, _)| *l == &label.id)
            .map(|(_, o)| o)
            .collect();
        let mut stories: Vec<usize> = mine.iter().map(|o| o.story).collect();
        stories.sort_unstable();
        stories.dedup();
        for s in stories {
            let members: Vec<&Object> = mine.iter().copied().filter(|o| o.story == s).collect();
            let Some(angle) = pier_angle(model, &members, tol) else {
                continue;
            };
            let (sn, cs) = angle.to_radians().sin_cos();
            found.push((
                s,
                li,
                WallSection {
                    kind: Kind::Pier,
                    label: label.id.clone(),
                    story: name(s),
                    dir: [cs, sn, 0.0],
                    areas: members
                        .iter()
                        .filter(|o| o.wall)
                        .map(|o| o.id.clone())
                        .collect(),
                    frames: members
                        .iter()
                        .filter(|o| !o.wall)
                        .map(|o| o.id.clone())
                        .collect(),
                    roof: f64::INFINITY,
                },
            ));
        }
    }

    // Spandrels: per label and the level nearest the walls' middles (frames' too, above the base).
    let pier_count = found.len();
    for (li, label) in model.spandrel_labels.iter().enumerate() {
        let mine: Vec<&Object> = spandrels
            .iter()
            .filter(|(l, _)| *l == &label.id)
            .map(|(_, o)| o)
            .collect();
        let mut seeds: Vec<usize> = mine
            .iter()
            .filter(|o| o.wall || o.near > 0)
            .map(|o| o.near)
            .collect();
        seeds.sort_unstable();
        seeds.dedup();
        let groups: Vec<(usize, Vec<&Object>)> = if label.multi_story {
            match seeds.last() {
                Some(&s) => vec![(s.max(1), mine.clone())],
                None => Vec::new(),
            }
        } else {
            seeds
                .iter()
                .map(|&s| {
                    let members = mine
                        .iter()
                        .copied()
                        .filter(|o| if o.wall { o.near == s } else { o.story == s })
                        .collect();
                    (s, members)
                })
                .collect()
        };
        for (s, members) in groups {
            let points: Vec<Vec3> = members.iter().flat_map(|o| o.pts.iter().copied()).collect();
            if points.is_empty() || points.iter().all(|p| p[2] > top + tol) {
                continue;
            }
            // All corners in one vertical plane: the first wall's, else through the first frame.
            let normal = match members.iter().find(|o| o.wall) {
                Some(w) => newell(&w.pts),
                None => axes::normalize(axes::cross(
                    axes::sub(points[1], points[0]),
                    [0.0, 0.0, 1.0],
                )),
            };
            let off = points
                .iter()
                .map(|p| axes::dot(normal, axes::sub(*p, points[0])).abs())
                .fold(0.0f64, f64::max);
            if off > merge + 1e-12 * size {
                notes.push(format!(
                    "Spandrel {} at {}: its walls and frames are not in one plane, so it has no forces",
                    label.id,
                    name(s)
                ));
                continue;
            }
            // The longest plan distance between two points (the first pair of equal ones).
            let (mut best, mut pair) = (-1.0, (points[0], points[0]));
            for a in &points {
                for b in &points {
                    let d = plan_dist2(*a, *b);
                    if d > best {
                        best = d;
                        pair = (*a, *b);
                    }
                }
            }
            let h = axes::sub(pair.1, pair.0);
            let angle = half_turn(h[1].atan2(h[0]).to_degrees()).to_radians();
            found.push((
                s,
                li,
                WallSection {
                    kind: Kind::Spandrel,
                    label: label.id.clone(),
                    story: name(s),
                    dir: [angle.cos(), angle.sin(), 0.0],
                    areas: members
                        .iter()
                        .filter(|o| o.wall)
                        .map(|o| o.id.clone())
                        .collect(),
                    frames: members
                        .iter()
                        .filter(|o| !o.wall)
                        .map(|o| o.id.clone())
                        .collect(),
                    roof: top + tol,
                },
            ));
        }
    }
    // The highest story first, then the label order (piers before spandrels).
    let (p, s) = found.split_at_mut(pier_count);
    p.sort_by_key(|(st, li, _)| (std::cmp::Reverse(*st), *li));
    s.sort_by_key(|(st, li, _)| (std::cmp::Reverse(*st), *li));
    found.into_iter().map(|(_, _, w)| w).collect()
}

/// A pier's axis angle (degrees): from its walls at the bottom cut, else the top cut, else its first
/// frame's local 2. None without either.
fn pier_angle(model: &Model, members: &[&Object], tol: f64) -> Option<f64> {
    struct Plane {
        angle: f64,
        length: f64,
        normal: Vec3,
        anchor: Vec3,
    }
    let all = members.iter().flat_map(|o| o.pts.iter());
    let lo = all.clone().map(|p| p[2]).fold(f64::INFINITY, f64::min);
    let hi = all.map(|p| p[2]).fold(f64::NEG_INFINITY, f64::max);
    for z in [lo, hi] {
        let mut planes: Vec<Plane> = Vec::new();
        for w in members.iter().filter(|o| o.wall) {
            let on: Vec<Vec3> = w
                .pts
                .iter()
                .copied()
                .filter(|p| (p[2] - z).abs() <= tol)
                .collect();
            if on.len() < 2 {
                continue;
            }
            let length = on
                .iter()
                .flat_map(|a| {
                    on.iter()
                        .map(move |b| axes::dot(axes::sub(*a, *b), axes::sub(*a, *b)))
                })
                .fold(0.0f64, f64::max)
                .sqrt();
            let p = &w.pts;
            let mut n = unit(axes::cross(axes::sub(p[1], p[0]), axes::sub(p[2], p[0])));
            let angle = half_turn((-n[0]).atan2(n[1]).to_degrees());
            let (s, c) = angle.to_radians().sin_cos();
            if axes::dot(n, [-s, c, 0.0]) < 0.0 {
                n = axes::scale(n, -1.0);
            }
            let anchor = p[0];
            let joins = |g: &Plane| {
                let d = axes::sub(anchor, g.anchor);
                let along = axes::dot(d, d).sqrt() <= tol || {
                    let c = axes::cross(unit(d), g.normal);
                    axes::dot(c, c).sqrt() > 0.99
                };
                axes::dot(n, g.normal).abs() > 0.99 && along
            };
            match planes.iter_mut().find(|g| joins(g)) {
                Some(g) => g.length += length,
                None => planes.push(Plane {
                    angle,
                    length,
                    normal: n,
                    anchor,
                }),
            }
        }
        let mut best: Option<&Plane> = None;
        for g in &planes {
            if best.is_none_or(|b| g.length > b.length) {
                best = Some(g);
            }
        }
        if let Some(b) = best {
            return Some(b.angle);
        }
    }
    let first = members.iter().find(|o| !o.wall)?;
    let f = model.frames.iter().find(|f| &f.id == first.id)?;
    let e2 = axes::local_axes(first.pts[0], first.pts[1], f.angle)[1];
    let a = e2[1].atan2(e2[0]).to_degrees();
    Some(if a <= -180.0 + 1e-8 { 180.0 } else { a })
}

/// Whether an element with joint positions `p` (along a cut's direction) lies wholly on the far side
/// of the cut at `at` from the material: at or below it for a bottom or left end (`low`), at or above
/// it for a top or right end.
fn beyond(p: &[f64], at: f64, low: bool, tol: f64) -> bool {
    if low {
        p.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)) <= at + tol
    } else {
        p.iter().fold(f64::INFINITY, |a, &b| a.min(b)) >= at - tol
    }
}

/// One cut of a pier or spandrel in the analysis model.
#[derive(Debug, Clone)]
struct End {
    centroid: Vec3,
    axes: [Vec3; 3],
    sign: f64,
    /// The cut: the plane where `dir . x = at`, material on the side of increasing `dir . x` at the
    /// bottom and left end (`low`), of decreasing at the top and right end.
    dir: Vec3,
    at: f64,
    low: bool,
    /// Joints above this height give no force.
    roof: f64,
}

/// The piers' and spandrels' cuts in the analysis model and which elements give them forces.
#[derive(Debug, Clone)]
pub struct Cuts {
    sections: Vec<(Kind, Id, String, f64)>,
    /// Two per section: bottom, top (pier) or left, right (spandrel).
    ends: Vec<End>,
    /// Per frame and per area of the model: the ends it gives forces to.
    pub frame_ends: Vec<Vec<usize>>,
    pub area_ends: Vec<Vec<usize>>,
    xyz: Vec<Vec3>,
    /// A joint lies on a cut within `on_tol`; an element lies beyond it within `side_tol`.
    on_tol: f64,
    side_tol: f64,
}

impl Cuts {
    /// The cuts of `sections` (members named by the model's frames and areas); None without any.
    pub fn new(model: &Model, sections: &[WallSection]) -> Option<Cuts> {
        if sections.is_empty() {
            return None;
        }
        let xyz: Vec<Vec3> = model.nodes.iter().map(coords).collect();
        let index: HashMap<&Id, usize> = model
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (&n.id, i))
            .collect();
        let size = xyz
            .iter()
            .flat_map(|p| p.iter().map(|v| v.abs()))
            .fold(1.0f64, f64::max);
        let (on_tol, side_tol) = (1e-9 * size, 1e-7 * size);
        let frame_at: HashMap<&Id, usize> = model
            .frames
            .iter()
            .enumerate()
            .map(|(i, f)| (&f.id, i))
            .collect();
        let area_at: HashMap<&Id, usize> = model
            .areas
            .iter()
            .enumerate()
            .map(|(i, a)| (&a.id, i))
            .collect();
        let mut cuts = Cuts {
            sections: Vec::new(),
            ends: Vec::new(),
            frame_ends: vec![Vec::new(); model.frames.len()],
            area_ends: vec![Vec::new(); model.areas.len()],
            xyz,
            on_tol,
            side_tol,
        };
        for ws in sections {
            // Members: (frame?, element index, joints, area of a wall strip per length or the frame's).
            let mut members: Vec<(bool, usize, Vec<usize>, f64)> = Vec::new();
            for id in &ws.areas {
                let Some(&i) = area_at.get(id) else { continue };
                let a = &model.areas[i];
                let t = model
                    .area_section(&a.section)
                    .map_or(0.0, |s| s.analysed().1);
                members.push((false, i, a.nodes.iter().map(|n| index[n]).collect(), t));
            }
            for id in &ws.frames {
                let Some(&i) = frame_at.get(id) else { continue };
                let f = &model.frames[i];
                let area = model.section(&f.section).map_or(0.0, |s| s.A);
                members.push((true, i, f.nodes.iter().map(|n| index[n]).collect(), area));
            }
            if members.is_empty() {
                continue;
            }
            let pier = ws.kind == Kind::Pier;
            let d = if pier { [0.0, 0.0, 1.0] } else { ws.dir };
            let proj = |n: usize| axes::dot(cuts.xyz[n], d);
            let all = members.iter().flat_map(|m| m.2.iter().map(|&n| proj(n)));
            let lo = all.clone().fold(f64::INFINITY, f64::min);
            let hi = all.fold(f64::NEG_INFINITY, f64::max);
            let mut centroids = [[0.0; 3]; 2];
            let mut taps: [Vec<(bool, usize)>; 2] = [Vec::new(), Vec::new()];
            for (e, cut) in [lo, hi].into_iter().enumerate() {
                let (mut total, mut sum) = (0.0, [0.0; 3]);
                let mut corners: Vec<Vec3> = Vec::new();
                for (frame, i, nodes, area) in &members {
                    let p: Vec<f64> = nodes.iter().map(|&n| proj(n)).collect();
                    if beyond(&p, cut, e == 0, side_tol) {
                        continue;
                    }
                    let on: Vec<usize> = nodes
                        .iter()
                        .zip(&p)
                        .filter(|(_, t)| (*t - cut).abs() <= on_tol)
                        .map(|(&n, _)| n)
                        .collect();
                    if on.is_empty() {
                        continue;
                    }
                    taps[e].push((*frame, *i));
                    corners.extend(on.iter().map(|&n| cuts.xyz[n]));
                    let (a, c) = match (frame, on.as_slice()) {
                        (false, [m, n]) => {
                            let (pm, pn) = (cuts.xyz[*m], cuts.xyz[*n]);
                            let l = axes::dot(axes::sub(pm, pn), axes::sub(pm, pn)).sqrt();
                            (l * area, axes::scale(axes::add(pm, pn), 0.5))
                        }
                        (true, [m]) => (*area, cuts.xyz[*m]),
                        _ => (0.0, [0.0; 3]),
                    };
                    total += a;
                    sum = axes::add(sum, axes::scale(c, a));
                }
                centroids[e] = if total > 0.0 {
                    axes::scale(sum, 1.0 / total)
                } else if !corners.is_empty() {
                    let k = corners.len() as f64;
                    let mean = corners.iter().fold([0.0; 3], |s, p| axes::add(s, *p));
                    [
                        mean[0] / k,
                        mean[1] / k,
                        if pier { mean[2] / k } else { 0.0 },
                    ]
                } else {
                    [0.0; 3]
                };
            }
            let basis = if pier {
                let a2 = ws.dir;
                [[0.0, 0.0, 1.0], a2, [-a2[1], a2[0], 0.0]]
            } else {
                let h = axes::sub(centroids[1], centroids[0]);
                let len = (h[0] * h[0] + h[1] * h[1]).sqrt();
                let a1 = if len > on_tol {
                    [h[0] / len, h[1] / len, 0.0]
                } else {
                    ws.dir
                };
                [a1, [0.0, 0.0, 1.0], axes::cross(a1, [0.0, 0.0, 1.0])]
            };
            let base = cuts.ends.len();
            for e in 0..2 {
                for &(frame, i) in &taps[e] {
                    let list = if frame {
                        &mut cuts.frame_ends[i]
                    } else {
                        &mut cuts.area_ends[i]
                    };
                    list.push(base + e);
                }
                cuts.ends.push(End {
                    centroid: centroids[e],
                    axes: basis,
                    sign: if e == 0 { -1.0 } else { 1.0 },
                    dir: d,
                    at: [lo, hi][e],
                    low: e == 0,
                    roof: ws.roof,
                });
            }
            cuts.sections.push((
                ws.kind,
                ws.label.clone(),
                ws.story.clone(),
                ws.dir[1].atan2(ws.dir[0]).to_degrees(),
            ));
        }
        (!cuts.sections.is_empty()).then_some(cuts)
    }

    /// Room for the sums: force and moment (about the centroid) per end.
    pub fn zero(&self) -> Vec<[f64; 6]> {
        vec![[0.0; 6]; self.ends.len()]
    }

    /// Adds an element's part to the ends `ends` (those it reaches from the material side): `g(k)` is
    /// the force and moment joint `nodes[k]` puts on it (global); only joints on each cut count.
    pub fn add(
        &self,
        acc: &mut [[f64; 6]],
        ends: &[usize],
        nodes: &[usize],
        g: &dyn Fn(usize) -> [f64; 6],
    ) {
        self.add_where(acc, ends, nodes, g, false);
    }

    /// Like [`Self::add`] for a hidden element of an area (an edge constraint's beams, a frame-joint
    /// spring), `ends` being the area's: only the ends it reaches from the material side itself.
    pub fn add_hidden(
        &self,
        acc: &mut [[f64; 6]],
        ends: &[usize],
        nodes: &[usize],
        g: &dyn Fn(usize) -> [f64; 6],
    ) {
        self.add_where(acc, ends, nodes, g, true);
    }

    fn add_where(
        &self,
        acc: &mut [[f64; 6]],
        ends: &[usize],
        nodes: &[usize],
        g: &dyn Fn(usize) -> [f64; 6],
        side: bool,
    ) {
        for &e in ends {
            let end = &self.ends[e];
            let p: Vec<f64> = nodes
                .iter()
                .map(|&n| axes::dot(self.xyz[n], end.dir))
                .collect();
            if side && beyond(&p, end.at, end.low, self.side_tol) {
                continue;
            }
            for (k, n) in nodes.iter().enumerate() {
                if (p[k] - end.at).abs() > self.on_tol || self.xyz[*n][2] > end.roof {
                    continue;
                }
                let v = g(k);
                let f = [v[0], v[1], v[2]];
                let m = axes::add(
                    [v[3], v[4], v[5]],
                    axes::cross(axes::sub(self.xyz[*n], end.centroid), f),
                );
                for d in 0..3 {
                    acc[e][d] += f[d];
                    acc[e][3 + d] += m[d];
                }
            }
        }
    }

    /// P V2 V3 T M2 M3 of the sums.
    fn components(&self, acc: &[[f64; 6]], e: usize) -> [f64; 6] {
        let end = &self.ends[e];
        let f = [acc[e][0], acc[e][1], acc[e][2]].map(|v| end.sign * v);
        let m = [acc[e][3], acc[e][4], acc[e][5]].map(|v| end.sign * v);
        let a = &end.axes;
        [
            axes::dot(f, a[0]),
            axes::dot(f, a[1]),
            axes::dot(f, a[2]),
            axes::dot(m, a[0]),
            -axes::dot(m, a[1]),
            axes::dot(m, a[2]),
        ]
    }

    /// The pier and spandrel forces of the sums.
    pub fn forces(&self, acc: &[[f64; 6]]) -> (Vec<PierForce>, Vec<SpandrelForce>) {
        let (mut piers, mut spandrels) = (Vec::new(), Vec::new());
        for (s, (kind, label, story, _)) in self.sections.iter().enumerate() {
            let [a, b] = [2 * s, 2 * s + 1].map(|e| self.components(acc, e));
            match kind {
                Kind::Pier => piers.push(PierForce {
                    story: story.clone(),
                    pier: label.clone(),
                    top: b,
                    bottom: a,
                }),
                Kind::Spandrel => spandrels.push(SpandrelForce {
                    story: story.clone(),
                    spandrel: label.clone(),
                    left: a,
                    right: b,
                }),
            }
        }
        (piers, spandrels)
    }

    /// The sections' axis angles and cut centroids.
    pub fn sections(&self) -> (Vec<PierSection>, Vec<SpandrelSection>) {
        let (mut piers, mut spandrels) = (Vec::new(), Vec::new());
        for (s, (kind, label, story, angle)) in self.sections.iter().enumerate() {
            let [a, b] = [2 * s, 2 * s + 1].map(|e| self.ends[e].centroid);
            match kind {
                Kind::Pier => piers.push(PierSection {
                    story: story.clone(),
                    pier: label.clone(),
                    angle: *angle,
                    bottom: a,
                    top: b,
                }),
                Kind::Spandrel => spandrels.push(SpandrelSection {
                    story: story.clone(),
                    spandrel: label.clone(),
                    left: a,
                    right: b,
                }),
            }
        }
        (piers, spandrels)
    }
}
