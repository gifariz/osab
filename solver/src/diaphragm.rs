//! Rigid diaphragms: the joints that name a diaphragm (directly or through an area) form one
//! constraint per story. Every joint of a constraint follows a master joint in UX, UY and RZ,
//! u = u_m - θz (y - y_m), v = v_m + θz (x - x_m), θz = θz_m, and keeps its own UZ, RX and RY
//! (docs/phases/PHASE4_PLAN.md, probe finding 5). The analysis applies it exactly with [`crate::rigid::Ties`].
//! A semi-rigid diaphragm ties nothing; it groups the floor's joints per story for the centres of mass
//! and user wind loads.
//!
//! Centres (docs/phases/DIAPHRAGM_CENTERS_PLAN.md, step 1 outcome): the masses of a diaphragm at a story
//! are the lateral masses of its joints there; the centre of mass has x weighted by the Y masses and y by
//! the X masses; the cumulative values cover the same diaphragm at this story and every story above.
//!
//! User wind loads ([`wind_joint_loads`]) act at the diaphragm's centre of mass or at given (x, y), and no
//! joint is added: on a rigid diaphragm as Fx, Fy and Mz + (x - x_m) Fy - (y - y_m) Fx at the master
//! (statics of a rigid body); on a semi-rigid one into the floor element under the point, spread over the
//! joints as a rigid floor's inertia forces, or tied to the joint object nearest the centre of mass.

use std::collections::HashMap;

use crate::model::{Id, Model, WindLoad, WindSemiRigid};
use crate::results::DiaphragmCentre;
use crate::Error;

/// The joints of one diaphragm at one story, the master first.
#[derive(Debug, Clone)]
pub struct Level {
    pub diaphragm: Id,
    /// The story's name, or its height when the model has no stories.
    pub story: String,
    pub semi_rigid: bool,
    pub joints: Vec<usize>,
}

/// The model's diaphragm constraints (rigid levels with two or more joints). A joint between two story
/// levels belongs to the story above, one above the top story to the top story's level of its own
/// height. Fails on an unknown diaphragm, a joint in two diaphragms or also in a rigid-link body, and a
/// constraint with more than one joint restrained in UX, UY or RZ (that joint becomes the master).
pub fn levels(model: &Model) -> Result<Vec<Level>, Error> {
    let mut groups = groups(model)?;
    groups.retain(|g| !g.semi_rigid && g.joints.len() > 1);
    Ok(groups)
}

/// Every diaphragm level: rigid ones (also with one joint) and semi-rigid ones (a joint in a rigid
/// diaphragm is not also in a semi-rigid one; of two semi-rigid ones the first assigned counts).
pub fn groups(model: &Model) -> Result<Vec<Level>, Error> {
    let bad = |msg: String| Err(Error::Invalid(msg));
    if model.diaphragms.is_empty()
        && model.nodes.iter().all(|n| n.diaphragm.is_none())
        && model.areas.iter().all(|a| a.diaphragm.is_none())
    {
        return Ok(Vec::new());
    }
    let index: HashMap<&Id, usize> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (&n.id, i))
        .collect();
    // Each joint's rigid diaphragm, and its semi-rigid one, as indices into `model.diaphragms`.
    let mut of: Vec<Option<usize>> = vec![None; model.nodes.len()];
    let mut semi_of: Vec<Option<usize>> = vec![None; model.nodes.len()];
    let mut join = |n: usize, d: &Id, what: String| -> Result<(), Error> {
        let Some(k) = model.diaphragms.iter().position(|x| &x.id == d) else {
            return Err(Error::Invalid(format!("{what}: unknown diaphragm {d}")));
        };
        if model.diaphragms[k].semi_rigid {
            semi_of[n].get_or_insert(k);
            return Ok(());
        }
        match of[n] {
            Some(e) if e != k => Err(Error::Invalid(format!(
                "joint {} is in two diaphragms ({} and {d}); a joint can be in one only",
                model.nodes[n].id, model.diaphragms[e].id
            ))),
            _ => {
                of[n] = Some(k);
                Ok(())
            }
        }
    };
    for (i, n) in model.nodes.iter().enumerate() {
        if let Some(d) = &n.diaphragm {
            join(i, d, format!("joint {}", n.id))?;
        }
    }
    for a in &model.areas {
        if let Some(d) = &a.diaphragm {
            for id in &a.nodes {
                let Some(&i) = index.get(id) else {
                    return bad(format!("area {}: unknown node {id}", a.id));
                };
                join(i, d, format!("area {}", a.id))?;
            }
        }
    }
    let linked: Vec<bool> = {
        let mut v = vec![false; model.nodes.len()];
        for l in &model.links {
            for id in &l.nodes {
                if let Some(&i) = index.get(id) {
                    v[i] = true;
                }
            }
        }
        v
    };
    let tol = tolerance(model);
    let story = |z: f64| -> String {
        match model.stories.iter().find(|s| s.z >= z - tol) {
            Some(s) => s.id.to_string(),
            None => format!("z = {z}"),
        }
    };
    let mut groups: Vec<Level> = Vec::new();
    let mut at: HashMap<(usize, String), usize> = HashMap::new();
    for i in 0..model.nodes.len() {
        let Some(k) = of[i].or(semi_of[i]) else {
            continue;
        };
        let d = &model.diaphragms[k];
        if linked[i] && !d.semi_rigid {
            return bad(format!(
                "joint {} is in diaphragm {} and joined by a rigid link; a joint can be in one of them only",
                model.nodes[i].id, d.id
            ));
        }
        // Without stories, joints within the tolerance of one height share a level.
        let z = model.nodes[i].z;
        let s = if model.stories.is_empty() {
            let same = groups
                .iter()
                .find(|g| g.diaphragm == d.id && (model.nodes[g.joints[0]].z - z).abs() <= tol);
            same.map_or_else(|| format!("z = {z}"), |g| g.story.clone())
        } else {
            story(z)
        };
        let g = *at.entry((k, s.clone())).or_insert_with(|| {
            groups.push(Level {
                diaphragm: d.id.clone(),
                story: s,
                semi_rigid: d.semi_rigid,
                joints: Vec::new(),
            });
            groups.len() - 1
        });
        groups[g].joints.push(i);
    }
    // The master: the joint restrained in UX, UY or RZ, else the first.
    let mut held = vec![false; model.nodes.len()];
    for s in &model.supports {
        if let (Some(&i), Ok(r)) = (index.get(&s.node), s.restraints()) {
            held[i] |= r[0] || r[1] || r[5];
        }
    }
    for g in groups.iter_mut().filter(|g| !g.semi_rigid) {
        let h: Vec<usize> = g.joints.iter().copied().filter(|&i| held[i]).collect();
        if h.len() > 1 {
            return bad(format!(
                "diaphragm {} at {}: joints {} and {} are both restrained in UX, UY or RZ; a diaphragm \
                 level can have such restraints at one joint only",
                g.diaphragm, g.story, model.nodes[h[0]].id, model.nodes[h[1]].id
            ));
        }
        if let Some(&m) = h.first() {
            g.joints.retain(|&i| i != m);
            g.joints.insert(0, m);
        }
    }
    Ok(groups)
}

fn tolerance(model: &Model) -> f64 {
    let span = model
        .nodes
        .iter()
        .flat_map(|n| [n.x.abs(), n.y.abs(), n.z.abs()])
        .fold(1.0f64, f64::max);
    1e-9 * span
}

/// Lateral masses (X, Y) of a level's joints and its centre of mass (x by the Y masses, y by the X
/// masses; None without mass in that direction).
pub fn centre_of_mass(
    model: &Model,
    g: &Level,
    masses: &[[f64; 6]],
) -> (f64, f64, Option<f64>, Option<f64>) {
    let mx: f64 = g.joints.iter().map(|&j| masses[j][0]).sum();
    let my: f64 = g.joints.iter().map(|&j| masses[j][1]).sum();
    let x = g
        .joints
        .iter()
        .map(|&j| masses[j][1] * model.nodes[j].x)
        .sum::<f64>();
    let y = g
        .joints
        .iter()
        .map(|&j| masses[j][0] * model.nodes[j].y)
        .sum::<f64>();
    (
        mx,
        my,
        (my > 0.0).then(|| x / my),
        (mx > 0.0).then(|| y / mx),
    )
}

/// The rows of the centres table, the highest story first, in diaphragm order; `rigidity` gives a rigid
/// level's centre of rigidity (by its index in `groups`).
pub fn centres(
    model: &Model,
    groups: &[Level],
    masses: &[[f64; 6]],
    rigidity: &HashMap<usize, (f64, f64)>,
) -> Vec<DiaphragmCentre> {
    // Story order: the model's stories, else heights.
    let height = |g: &Level| -> f64 {
        model
            .stories
            .iter()
            .find(|s| s.id.0 == g.story)
            .map_or(model.nodes[g.joints[0]].z, |s| s.z)
    };
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by(|&a, &b| {
        let da = model
            .diaphragms
            .iter()
            .position(|d| d.id == groups[a].diaphragm);
        let db = model
            .diaphragms
            .iter()
            .position(|d| d.id == groups[b].diaphragm);
        height(&groups[b])
            .total_cmp(&height(&groups[a]))
            .then(da.cmp(&db))
    });
    order
        .iter()
        .map(|&k| {
            let g = &groups[k];
            let (mx, my, xcm, ycm) = centre_of_mass(model, g, masses);
            if g.semi_rigid {
                return DiaphragmCentre {
                    story: g.story.clone(),
                    diaphragm: g.diaphragm.clone(),
                    mass_x: 0.0,
                    mass_y: 0.0,
                    xcm,
                    ycm,
                    cum_mass_x: 0.0,
                    cum_mass_y: 0.0,
                    xccm: None,
                    yccm: None,
                    xcr: None,
                    ycr: None,
                };
            }
            // This diaphragm at this story and above.
            let mut cum = Level {
                joints: Vec::new(),
                ..g.clone()
            };
            for h in groups {
                if h.diaphragm == g.diaphragm && !h.semi_rigid && height(h) >= height(g) {
                    cum.joints.extend(&h.joints);
                }
            }
            let (cmx, cmy, xccm, yccm) = centre_of_mass(model, &cum, masses);
            let cr = rigidity.get(&k);
            DiaphragmCentre {
                story: g.story.clone(),
                diaphragm: g.diaphragm.clone(),
                mass_x: mx,
                mass_y: my,
                xcm,
                ycm,
                cum_mass_x: cmx,
                cum_mass_y: cmy,
                xccm,
                yccm,
                xcr: cr.map(|c| c.0),
                ycr: cr.map(|c| c.1),
            }
        })
        .collect()
}

/// The floor element of a semi-rigid level under a point (in plan): its joints and shape function values.
pub type FloorAt<'a> = &'a dyn Fn(&Level, (f64, f64)) -> Option<Vec<(usize, f64)>>;

/// The user wind rows with no joints of their diaphragm at their story, as "load pattern P: story,
/// diaphragm" or, once the patterns are taken into the cases, "load case C: story, diaphragm"; each listed
/// once. They carry no load.
pub fn skipped_wind_rows(model: &Model, groups: &[Level]) -> Vec<String> {
    let missing = |w: &&WindLoad| {
        !groups
            .iter()
            .any(|g| g.diaphragm == w.diaphragm && g.story == w.story.0)
    };
    let patterns = model
        .load_patterns
        .iter()
        .map(|p| ("load pattern", &p.id, &p.wind_loads));
    let cases = model
        .load_cases
        .iter()
        .map(|c| ("load case", &c.id, &c.wind_loads));
    let mut out: Vec<String> = Vec::new();
    for (kind, id, rows) in patterns.chain(cases) {
        for w in rows.iter().filter(missing) {
            let row = format!("{kind} {id}: {}, {}", w.story, w.diaphragm);
            if !out.contains(&row) {
                out.push(row);
            }
        }
    }
    out
}

/// Joint loads (joint index, Fx Fy Fz Mx My Mz) for one user wind load (rules in the module text).
/// `floor(level, (x, y))` gives the joints and shape function values of the floor element of a
/// semi-rigid level under the point, if any; `attached[j]` says whether joint j has an element, support,
/// spring or link. `notes` gets a line when a point on no floor element falls back to spreading by mass.
#[allow(clippy::too_many_arguments)]
pub fn wind_joint_loads(
    model: &Model,
    groups: &[Level],
    masses: &[[f64; 6]],
    attached: &[bool],
    floor: FloorAt,
    case: &Id,
    w: &WindLoad,
    notes: &mut Vec<String>,
) -> Result<Vec<(usize, [f64; 6])>, Error> {
    let what = format!(
        "load case {case}: user wind load at {} on {}",
        w.story, w.diaphragm
    );
    let Some(g) = groups
        .iter()
        .find(|g| g.diaphragm == w.diaphragm && g.story == w.story.0)
    else {
        // No joints of that diaphragm at that story: the row is skipped (the run lists it once, see
        // `skipped_wind_rows`).
        return Ok(Vec::new());
    };
    let (mx, my, xcm, ycm) = centre_of_mass(model, g, masses);
    let cm = match (xcm, ycm) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };
    let need_cm = || {
        cm.ok_or_else(|| {
            Error::Invalid(format!(
                "{what}: the diaphragm has no mass there (the mass source gives it none), so it has no centre of mass"
            ))
        })
    };
    let p = match w.point {
        crate::model::WindPoint::CentreOfMass => need_cm()?,
        crate::model::WindPoint::Coordinates => (w.x, w.y),
    };
    // Fx, Fy at p and Mz, moved to joint j as a rigid body: the moment of the forces about j added.
    let at = |j: usize| -> (usize, [f64; 6]) {
        let n = &model.nodes[j];
        (
            j,
            [
                w.fx,
                w.fy,
                0.0,
                0.0,
                0.0,
                w.mz + (p.0 - n.x) * w.fy - (p.1 - n.y) * w.fx,
            ],
        )
    };
    if !g.semi_rigid {
        return Ok(vec![at(g.joints[0])]);
    }
    let spread = || -> Result<Vec<(usize, [f64; 6])>, Error> {
        let (xc, yc) = need_cm()?;
        // Torsion about the centre of mass, carried as m alpha r at right angles to each radius.
        let torsion = w.mz + (p.0 - xc) * w.fy - (p.1 - yc) * w.fx;
        let inertia: f64 = g
            .joints
            .iter()
            .map(|&j| {
                let n = &model.nodes[j];
                masses[j][0] * (n.y - yc).powi(2) + masses[j][1] * (n.x - xc).powi(2)
            })
            .sum();
        if torsion != 0.0 && inertia <= 0.0 {
            return Err(Error::Invalid(format!(
                "{what}: the diaphragm's mass has no moment of inertia to carry the torsion"
            )));
        }
        let alpha = if torsion == 0.0 {
            0.0
        } else {
            torsion / inertia
        };
        Ok(g.joints
            .iter()
            .filter(|&&j| masses[j][0] != 0.0 || masses[j][1] != 0.0)
            .map(|&j| {
                let n = &model.nodes[j];
                let fx = masses[j][0] * (w.fx / mx - alpha * (n.y - yc));
                let fy = masses[j][1] * (w.fy / my + alpha * (n.x - xc));
                (j, [fx, fy, 0.0, 0.0, 0.0, 0.0])
            })
            .collect())
    };
    match w.semi_rigid {
        WindSemiRigid::Floor => match floor(g, p) {
            Some(shares) => Ok(shares
                .iter()
                .map(|&(j, s)| (j, [w.fx * s, w.fy * s, 0.0, 0.0, 0.0, w.mz * s]))
                .collect()),
            None => {
                let line = format!(
                    "User wind loads at {} on {}: the point ({:.4}, {:.4}) lies on no floor element of the \
                     diaphragm, so they are spread over its joints by mass",
                    w.story, w.diaphragm, p.0, p.1
                );
                if !notes.contains(&line) {
                    notes.push(line);
                }
                spread()
            }
        },
        WindSemiRigid::Mass => spread(),
        WindSemiRigid::NearestJoint => {
            let (xc, yc) = need_cm()?;
            // Joint objects only (mesh joints are named ~n), and only ones joined to the structure.
            let best = g
                .joints
                .iter()
                .copied()
                .filter(|&j| attached[j] && !model.nodes[j].id.0.starts_with('~'))
                .min_by(|&a, &b| {
                    let d = |j: usize| (model.nodes[j].x - xc).hypot(model.nodes[j].y - yc);
                    d(a).total_cmp(&d(b))
                })
                .ok_or_else(|| {
                    Error::Invalid(format!("{what}: the diaphragm has no joint to carry it"))
                })?;
            Ok(vec![at(best)])
        }
    }
}
