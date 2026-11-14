//! Stored meshes (user mesh, Phase 3b step 8b): an area can keep the mesh it once got, so it
//! stays fixed whatever the mesh settings. Before meshing, such an area is replaced by
//! - its stored elements, each an area of its own analysed as one element (`"mesh": "none"`), and
//! - a null copy of itself (no section, same id). Its area loads go to its stored elements as their own
//!   area loads (global X, Y, Z parts), so they stay the elements' loads (pier, spandrel and story
//!   forces leave an element's own loads out); when the elements leave part of it out for an object
//!   that won an overlap there, the null copy keeps them and they go to the elements under it, its stored
//!   ones and the winner's, as every object's load does.
//!
//! Meshed neighbours then share the stored joints on their common edges (the mesh step divides shared
//! lines at the joints lying on them; edge constraints tie any joints of theirs in between). Stored
//! joints that are not object joints become mesh joints `~1, ~2…`. After meshing, the elements are named
//! after the area again.

use std::collections::{HashMap, HashSet};

use osab_solver::axes::{self, Vec3};
use osab_solver::model::{
    coords, Area, AreaLoad, AreaMesh, AreaMeshWord, Id, LoadDir, Model, Node, ShellType, Support,
};
use osab_solver::results::{MeshElement, MeshJoint};
use osab_solver::Error;

use crate::build::{area_axes, model_tol, polygons_overlap, Meshed};
use crate::geom::{self, Plane, P2};

/// What the stored meshes added, to tell the results in objects again.
pub struct Stored {
    elements: Vec<MeshElement>,
    joints: Vec<MeshJoint>,
    notes: Vec<String>,
}

/// The model with every stored mesh expanded into its elements (see the module notes).
pub fn expand(model: &Model) -> Result<(Model, Stored), Error> {
    let xyz: Vec<Vec3> = model.nodes.iter().map(coords).collect();
    let tol = model_tol(&xyz);
    let index: HashMap<&Id, usize> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (&n.id, i))
        .collect();
    let support_of: HashMap<&Id, &Support> = model.supports.iter().map(|s| (&s.node, s)).collect();
    let mut taken: HashSet<Id> = model.nodes.iter().map(|n| n.id.clone()).collect();
    let mut object_ids: HashSet<Id> = model
        .areas
        .iter()
        .map(|a| a.id.clone())
        .chain(model.frames.iter().map(|f| f.id.clone()))
        .chain(model.links.iter().map(|l| l.id.clone()))
        .collect();
    let mut work = model.clone();
    let mut out = Stored {
        elements: Vec::new(),
        joints: Vec::new(),
        notes: Vec::new(),
    };
    let mut next_joint = 1usize;
    let mut regions: Vec<(Plane, Vec<P2>)> = Vec::new();
    let corners_of = |b: &Area| -> Option<Vec<Vec3>> {
        b.nodes
            .iter()
            .map(|id| index.get(id).map(|&i| xyz[i]))
            .collect()
    };
    let openings: Vec<Vec<Vec3>> = model
        .areas
        .iter()
        .filter(|b| b.opening)
        .filter_map(corners_of)
        .collect();
    // The openings lying in a plane, in its 2D coordinates.
    let openings_in = |plane: &Plane| -> Vec<Vec<P2>> {
        openings
            .iter()
            .filter(|p| p.iter().all(|q| plane.offset(*q) <= tol))
            .map(|p| p.iter().map(|q| plane.to2(*q)).collect())
            .collect()
    };
    for (ai, a) in model.areas.iter().enumerate() {
        let Some(um) = &a.user_mesh else {
            continue;
        };
        let bad = |why: &str| {
            Error::Invalid(format!(
                "area {}: its stored mesh {why}; store it again or remove it",
                a.id
            ))
        };
        if a.opening || a.section.is_empty() {
            return Err(bad(
                "belongs to an opening or a null area, which have no elements",
            ));
        }
        let mut corners: Vec<Vec3> = Vec::new();
        for id in &a.nodes {
            let i = index
                .get(id)
                .ok_or_else(|| Error::Invalid(format!("area {}: unknown node {id}", a.id)))?;
            corners.push(xyz[*i]);
        }
        let ax = area_axes(&corners, a.angle);
        let plane = Plane {
            origin: corners[0],
            axes: ax,
        };
        let slope = axes::dot(ax[2], [0.0, 0.0, 1.0])
            .abs()
            .clamp(0.0, 1.0)
            .acos()
            .to_degrees();
        let wall = slope >= model.wall_slope() - 1e-9;
        let membrane = model
            .area_section(&a.section)
            .is_some_and(|s| s.kind == ShellType::Membrane);
        if membrane && !wall {
            return Err(bad(
                "is on a Membrane floor, which is cut at its beams and walls only",
            ));
        }
        // The stored mesh must still fit the area: its joints in the area, a joint at every corner that
        // no opening covers (where one does, the mesh goes round the opening).
        let c2: Vec<P2> = corners.iter().map(|p| plane.to2(*p)).collect();
        let holes = openings_in(&plane);
        if um
            .joints
            .iter()
            .any(|p| plane.offset(*p) > tol || !geom::inside(plane.to2(*p), &c2, tol))
        {
            return Err(bad("no longer fits the area (a joint lies outside it)"));
        }
        if corners.iter().zip(&c2).any(|(c, q)| {
            !um.joints.iter().any(|p| geom::dist3(*p, *c) <= tol)
                && !holes.iter().any(|h| geom::inside(*q, h, tol))
        }) {
            return Err(bad("no longer fits the area (a corner moved)"));
        }
        for e in &um.elements {
            let distinct: HashSet<usize> = e.iter().copied().collect();
            if !(3..=4).contains(&e.len())
                || distinct.len() != e.len()
                || e.iter().any(|&i| i >= um.joints.len())
            {
                return Err(bad("has an element that is not 3 or 4 of its joints"));
            }
        }
        // Joint ids: an object joint at the point, a stored joint of another area there, or a new ~k.
        let mut ids: Vec<Id> = Vec::new();
        for p in &um.joints {
            let found = work
                .nodes
                .iter()
                .find(|n| geom::dist3(coords(n), *p) <= tol)
                .map(|n| n.id.clone());
            let id = match found {
                Some(id) => id,
                None => {
                    let id = loop {
                        let id = Id(format!("~{next_joint}"));
                        next_joint += 1;
                        if taken.insert(id.clone()) {
                            break id;
                        }
                    };
                    work.nodes.push(Node {
                        id: id.clone(),
                        x: p[0],
                        y: p[1],
                        z: p[2],
                        in_mesh: None,
                        diaphragm: None,
                    });
                    out.joints.push(MeshJoint {
                        id: id.clone(),
                        x: p[0],
                        y: p[1],
                        z: p[2],
                    });
                    id
                }
            };
            ids.push(id);
        }
        // The elements, each an area analysed as one element.
        for (k, e) in um.elements.iter().enumerate() {
            let mut s = 0;
            let id = loop {
                let id = if s == 0 {
                    Id(format!("{}-{}", a.id, k + 1))
                } else {
                    Id(format!("{}-{}.{s}", a.id, k + 1))
                };
                if object_ids.insert(id.clone()) {
                    break id;
                }
                s += 1;
            };
            let nodes: Vec<Id> = e.iter().map(|&i| ids[i].clone()).collect();
            work.areas.push(Area {
                id: id.clone(),
                nodes: nodes.clone(),
                section: a.section.clone(),
                angle: a.angle,
                modifiers: a.modifiers,
                insertion: a.insertion,
                area_spring: a.area_spring.clone(),
                pier: a.pier.clone(),
                spandrel: a.spandrel.clone(),
                diaphragm: a.diaphragm.clone(),
                opening: false,
                mesh: Some(AreaMesh::Word(AreaMeshWord::None)),
                edge_restraints: None,
                user_mesh: None,
            });
            out.elements.push(MeshElement {
                id,
                parent: a.id.clone(),
                nodes,
            });
        }
        // Its area loads go to its elements when they are all of it: the elements cover the area less
        // its openings and no other area overlaps it (else the null copy shares them out, the overlap's
        // winner getting its part).
        let mine: Vec<Id> = out.elements[out.elements.len() - um.elements.len()..]
            .iter()
            .map(|e| e.id.clone())
            .collect();
        let covered: f64 = um
            .elements
            .iter()
            .map(|e| {
                let q: Vec<P2> = e.iter().map(|&i| plane.to2(um.joints[i])).collect();
                geom::area(&q).abs()
            })
            .sum();
        let open: f64 = holes.iter().map(|h| geom::area(h).abs()).sum();
        let span = geom::dist3(corners[0], corners[1]).max(1.0);
        let whole = covered >= geom::area(&c2).abs() - open - 1e-9 * span * span;
        let alone = !model.areas.iter().any(|b| {
            b.id != a.id
                && !b.opening
                && !b.section.is_empty()
                && corners_of(b).is_some_and(|p| {
                    p.iter().all(|q| plane.offset(*q) <= tol) && {
                        let p2: Vec<P2> = p.iter().map(|q| plane.to2(*q)).collect();
                        polygons_overlap(&p2, &c2, tol)
                    }
                })
        });
        for set in work.load_sets_mut().filter(|_| whole && alone) {
            let (here, other): (Vec<AreaLoad>, Vec<AreaLoad>) =
                set.area_loads.drain(..).partition(|l| l.area == a.id);
            set.area_loads = other;
            for l in here {
                let local = match l.dir {
                    LoadDir::L1 => Some(ax[0]),
                    LoadDir::L2 => Some(ax[1]),
                    LoadDir::L3 => Some(ax[2]),
                    _ => None,
                };
                let parts: Vec<(LoadDir, f64)> = match local {
                    Some(d) => [LoadDir::X, LoadDir::Y, LoadDir::Z]
                        .into_iter()
                        .zip(d)
                        .filter(|(_, v)| *v != 0.0)
                        .map(|(g, v)| (g, v * l.q))
                        .collect(),
                    None => vec![(l.dir, l.q)],
                };
                for id in &mine {
                    for &(dir, q) in &parts {
                        set.area_loads.push(AreaLoad {
                            area: id.clone(),
                            dir,
                            q,
                        });
                    }
                }
            }
        }
        // The area itself stays as a null area.
        let w = &mut work.areas[ai];
        w.section = Id(String::new());
        w.user_mesh = None;
        w.mesh = None;
        // "Add restraints on edges" (on for walls, off for floors): stored joints on an edge
        // whose two corners have the same restraints get them too.
        if a.edge_restraints.unwrap_or(wall) {
            let n = a.nodes.len();
            for k in 0..n {
                let (ca, cb) = (&a.nodes[k], &a.nodes[(k + 1) % n]);
                let (Some(sa), Some(sb)) = (support_of.get(ca), support_of.get(cb)) else {
                    continue;
                };
                let (ra, rb) = (sa.restraints()?, sb.restraints()?);
                if ra != rb || !ra.iter().any(|r| *r) {
                    continue;
                }
                for (p, id) in um.joints.iter().zip(&ids) {
                    let on = geom::on_segment3(*p, corners[k], corners[(k + 1) % n], tol);
                    if on.is_some_and(|t| t > 0.0 && t < 1.0)
                        && !work.supports.iter().any(|s| &s.node == id)
                    {
                        work.supports.push(Support {
                            node: id.clone(),
                            fix: sa.fix.clone(),
                        });
                    }
                }
            }
        }
        out.notes.push(format!(
            "Stored mesh kept for area {} ({} elements)",
            a.id,
            um.elements.len()
        ));
        regions.push((plane, c2));
    }
    // Openings inside a stored area, or over stored areas only (across two floors, say), are already
    // holes in their stored meshes.
    let inside_stored = |p: &[Vec3]| {
        regions.iter().any(|(plane, c2)| {
            p.iter()
                .all(|q| plane.offset(*q) <= tol && geom::inside(plane.to2(*q), c2, tol))
        })
    };
    let hosts: Vec<(bool, Plane, Vec<P2>)> = if regions.is_empty() {
        Vec::new()
    } else {
        model
            .areas
            .iter()
            .filter(|b| !b.opening && !b.section.is_empty())
            .filter_map(|b| {
                let c = corners_of(b)?;
                let plane = Plane {
                    origin: c[0],
                    axes: area_axes(&c, b.angle),
                };
                let c2 = c.iter().map(|q| plane.to2(*q)).collect();
                Some((b.user_mesh.is_some(), plane, c2))
            })
            .collect()
    };
    let over_stored_only = |p: &[Vec3]| {
        let mut over = hosts.iter().filter(|(_, plane, c2)| {
            p.iter().all(|q| plane.offset(*q) <= tol) && {
                let p2: Vec<P2> = p.iter().map(|q| plane.to2(*q)).collect();
                polygons_overlap(&p2, c2, tol)
            }
        });
        let first = over.next();
        first.is_some_and(|f| f.0) && over.all(|h| h.0)
    };
    work.areas.retain(|b| {
        !(b.opening && corners_of(b).is_some_and(|p| inside_stored(&p) || over_stored_only(&p)))
    });
    Ok((work, out))
}

impl Stored {
    /// Names the stored elements after their area again and lists them, and their joints, in the mesh.
    pub fn restore(self, m: &mut Meshed) {
        for el in &self.elements {
            m.parent.insert(el.id.clone(), el.parent.clone());
        }
        let mut joints = self.joints;
        joints.append(&mut m.info.joints);
        m.info.joints = joints;
        m.info.shells.extend(self.elements);
        let mut notes = self.notes;
        notes.append(&mut m.notes);
        m.notes = notes;
    }
}
