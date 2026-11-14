//! Benchmark runner: solves `tests/benchmarks/<name>/model.json` and compares the results with each
//! reference file in that folder (`hand.json`, `opensees.json`, then any other `*.json` but `model.json` whose name
//! has no `_`: helper files such as `*_extra.json` and `*_stations.json` are not references). A missing reference
//! file is skipped.
//!
//! Reference file format:
//! ```json
//! { "source": "how the numbers were obtained",
//!   "values": [
//!     { "case": "LC1", "kind": "disp",     "node": "2", "dof": "uz", "value": -0.00533 },
//!     { "case": "LC1", "kind": "reaction", "node": "1", "dof": "my", "value": -40.0 },
//!     { "case": "LC1", "kind": "force", "element": "1", "at": "i", "comp": "M3", "value": -40.0 },
//!     { "case": "LC1", "kind": "shell", "area": "F1", "node": "3", "comp": "M11", "value": 2.5 } ] }
//! ```
//! `reaction_sum` adds one reaction component over several joints:
//! `{ "case": "LC1", "kind": "reaction_sum", "nodes": ["1", "2"], "dof": "fx", "value": -50.0 }`
//! (`"nodes": ["*"]`: every joint with a reaction, mesh joints the mesh step restrained included).
//! `base_reaction` is a case's base reaction about the origin (for a spectrum case, its modes' sums
//! combined): `{ "case": "RSX", "kind": "base_reaction", "dof": "fx", "value": 512.0 }`.
//! Story results (see `crate::story`): `{ "case": "EQX", "kind": "joint_drift", "node": "2A1", "dir": "x",
//! "value": 0.0021 }` (compared by size), `{ "case": "EQX", "kind": "story_drift", "story": "Story2",
//! "dir": "x", "value": 0.0023 }` and `{ "case": "EQX", "kind": "story_force", "story": "Story2", "at":
//! "bottom", "comp": "VX", "value": 512.0 }` (P VX VY T MX MY, at top or bottom).
//! Pier and spandrel forces (see `crate::piers`): `{ "case": "L", "kind": "pier", "story": "L1", "label":
//! "P1", "at": "top", "comp": "M3", "value": 61.4 }` (at top or bottom; a spandrel's `"kind": "spandrel"`
//! at left or right).
//! `at` is "i", "j" or a distance from end i; with "i" or "j", `"station": 1.8` reads the end value at that
//! distance from end i (where the reference program reports it, past an end length offset). `element` may also name a link. `shell` values are an
//! area's corner forces (F11 F22 F12 M11 M22 M12 V13 V23) at corner joint `node`.
//!
//! Modal results (the model's modal case): `{ "kind": "period", "mode": 1, "value": 0.173 }`,
//! `{ "kind": "mass_ratio", "mode": 1, "dof": "ux", "value": 0.876 }` (participating mass ratio) and
//! `{ "kind": "mode_shape", "mode": 1, "node": "9", "dof": "ux", "value": 0.012 }` at unit modal mass
//! in the model's units. A mode shape's sign is free: each mode is compared with the sign that fits
//! the reference best.
//!
//! Centres of mass and rigidity (`diaphragm_centers`): `{ "kind": "centre", "story": "Story1",
//! "diaphragm": "D1", "comp": "xcm", "value": 3.2 }`, comp one of mass_x, mass_y, xcm, ycm, cum_mass_x,
//! cum_mass_y, xccm, yccm, xcr, ycr.
//!
//! A file with `"convergence": { "sizes": [0.5, 0.25, 0.125] }` holds values extrapolated to mesh size
//! zero (where OSAB's mesh differs from the other program's by design): the model is run at each
//! maximum floor mesh size, and OSAB's values are extrapolated the same way (`extrapolate`).

use serde::Deserialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::elements::Side;
use crate::model::{Id, MeshSetting, MeshSettings, Model, DOF_NAMES, FORCE_NAMES};
use crate::results::{Results, Step, SHELL_NAMES, STATION_NAMES};

/// Pass when |osab - ref| <= REL_TOL * |ref| + floor.
pub const REL_TOL: f64 = 1e-3;
/// floor = FLOOR_FRACTION * largest |ref| of the same quantity type in the same file, so values that
/// are essentially zero compared with the rest are not judged by relative error.
pub const FLOOR_FRACTION: f64 = 1e-5;
/// Absolute floor for quantities that are zero everywhere in a file (round-off noise only).
pub const ABS_FLOOR: f64 = 1e-9;

/// The reference files of a benchmark folder: `hand.json` and `opensees.json` first, then the other
/// reference files by name.
pub fn reference_files(dir: &Path) -> Vec<String> {
    let mut others: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| {
            n.ends_with(".json")
                && !["model.json", "hand.json", "opensees.json"].contains(&n.as_str())
                && !n.contains('_')
        })
        .collect();
    others.sort();
    let mut files: Vec<String> = ["hand.json", "opensees.json"]
        .into_iter()
        .filter(|f| dir.join(f).exists())
        .map(String::from)
        .collect();
    files.extend(others);
    files
}

#[derive(Debug, Deserialize)]
pub struct ReferenceFile {
    pub source: String,
    pub values: Vec<RefValue>,
    #[serde(default)]
    pub convergence: Option<Convergence>,
}

/// Mesh sizes (h, h/2, h/4) of a reference file whose values are extrapolated to size zero.
#[derive(Debug, Deserialize)]
pub struct Convergence {
    pub sizes: Vec<f64>,
}

/// Richardson extrapolation of values at mesh sizes h, h/2, h/4: the observed rate p from the two
/// differences, kept within 1..2 (DKQ plates: 2 for displacements of smooth solutions, less near
/// corners); the finest value when the differences change sign (no clear rate).
/// The same rule as `checks/convergence.py`.
pub fn extrapolate(f: [f64; 3]) -> f64 {
    let (d1, d2) = (f[0] - f[1], f[1] - f[2]);
    if d1 * d2 <= 0.0 {
        return f[2];
    }
    let p = (d1 / d2).log2().clamp(1.0, 2.0);
    f[2] - d2 / (2f64.powf(p) - 1.0)
}

/// The model with another maximum floor mesh size.
fn with_floor_size(model: &Model, h: f64) -> Model {
    let mut m = model.clone();
    let mut s = match &m.mesh {
        Some(MeshSetting::Auto(s)) => s.clone(),
        _ => MeshSettings::default(),
    };
    s.floor_size = Some(h);
    m.mesh = Some(MeshSetting::Auto(s));
    m
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RefValue {
    Disp {
        case: Id,
        /// "max" or "min" for a combination with both (step type).
        #[serde(default)]
        step: Option<String>,
        node: Id,
        dof: String,
        value: f64,
    },
    Reaction {
        case: Id,
        /// "max" or "min" for a combination with both (step type).
        #[serde(default)]
        step: Option<String>,
        node: Id,
        dof: String,
        value: f64,
    },
    Force {
        case: Id,
        /// "max" or "min" for a combination with both (step type).
        #[serde(default)]
        step: Option<String>,
        element: Id,
        at: At,
        /// With `at` "i" or "j": the distance from end i the reference read the end value at (past an
        /// end length offset); taken on the clear-length side of that point.
        #[serde(default)]
        station: Option<f64>,
        comp: String,
        value: f64,
    },
    #[serde(rename = "reaction_sum")]
    ReactionSum {
        case: Id,
        /// "max" or "min" for a combination with both (step type).
        #[serde(default)]
        step: Option<String>,
        nodes: Vec<Id>,
        dof: String,
        value: f64,
    },
    #[serde(rename = "joint_drift")]
    JointDrift {
        case: Id,
        #[serde(default)]
        step: Option<String>,
        node: Id,
        dir: String,
        value: f64,
    },
    #[serde(rename = "story_drift")]
    StoryDrift {
        case: Id,
        #[serde(default)]
        step: Option<String>,
        story: String,
        dir: String,
        value: f64,
    },
    #[serde(rename = "story_force")]
    StoryForce {
        case: Id,
        #[serde(default)]
        step: Option<String>,
        story: String,
        at: String,
        comp: String,
        value: f64,
    },
    /// Pier forces (`at` top or bottom) and spandrel forces (`at` left or right), comp P V2 V3 T M2 M3.
    Pier {
        case: Id,
        #[serde(default)]
        step: Option<String>,
        story: String,
        label: Id,
        at: String,
        comp: String,
        value: f64,
    },
    Spandrel {
        case: Id,
        #[serde(default)]
        step: Option<String>,
        story: String,
        label: Id,
        at: String,
        comp: String,
        value: f64,
    },
    #[serde(rename = "base_reaction")]
    BaseReaction {
        case: Id,
        /// "max" or "min" for a combination with both (step type).
        #[serde(default)]
        step: Option<String>,
        dof: String,
        value: f64,
    },
    Shell {
        case: Id,
        /// "max" or "min" for a combination with both (step type).
        #[serde(default)]
        step: Option<String>,
        area: Id,
        /// Shell element of a meshed area (missing: the area's own element).
        #[serde(default)]
        element: Option<Id>,
        node: Id,
        comp: String,
        value: f64,
    },
    Period {
        mode: usize,
        value: f64,
    },
    #[serde(rename = "mass_ratio")]
    MassRatio {
        mode: usize,
        dof: String,
        value: f64,
    },
    #[serde(rename = "mode_shape")]
    ModeShape {
        mode: usize,
        node: Id,
        dof: String,
        value: f64,
    },
    Centre {
        story: String,
        diaphragm: Id,
        comp: String,
        value: f64,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum At {
    End(String),
    Distance(f64),
}

#[derive(Debug, Clone)]
pub struct Check {
    pub benchmark: String,
    pub source: String,
    pub label: String,
    pub osab: f64,
    pub reference: f64,
    pub tolerance: f64,
    pub pass: bool,
}

impl Check {
    pub fn rel_error(&self) -> f64 {
        let d = (self.osab - self.reference).abs();
        if self.reference.abs() > 0.0 {
            d / self.reference.abs()
        } else {
            d
        }
    }
}

impl RefValue {
    fn value(&self) -> f64 {
        match self {
            RefValue::Disp { value, .. }
            | RefValue::Reaction { value, .. }
            | RefValue::Force { value, .. }
            | RefValue::ReactionSum { value, .. }
            | RefValue::BaseReaction { value, .. }
            | RefValue::JointDrift { value, .. }
            | RefValue::StoryDrift { value, .. }
            | RefValue::StoryForce { value, .. }
            | RefValue::Pier { value, .. }
            | RefValue::Spandrel { value, .. }
            | RefValue::Shell { value, .. }
            | RefValue::Period { value, .. }
            | RefValue::MassRatio { value, .. }
            | RefValue::ModeShape { value, .. }
            | RefValue::Centre { value, .. } => *value,
        }
    }

    /// Quantity type for the tolerance floor: translations, rotations, forces, moments.
    fn class(&self) -> &'static str {
        match self {
            RefValue::Disp { dof, .. } => {
                if dof.starts_with('u') {
                    "translation"
                } else {
                    "rotation"
                }
            }
            RefValue::Reaction { dof, .. }
            | RefValue::ReactionSum { dof, .. }
            | RefValue::BaseReaction { dof, .. } => {
                if dof.starts_with('f') {
                    "force"
                } else {
                    "moment"
                }
            }
            RefValue::JointDrift { .. } | RefValue::StoryDrift { .. } => "drift",
            RefValue::StoryForce { comp, .. } => {
                if comp.starts_with('M') || comp == "T" {
                    "story moment"
                } else {
                    "story force"
                }
            }
            RefValue::Pier { comp, .. } | RefValue::Spandrel { comp, .. } => {
                if comp.starts_with('M') || comp == "T" {
                    "wall moment"
                } else {
                    "wall force"
                }
            }
            RefValue::Force { comp, .. } => {
                if comp.starts_with('M') || comp == "T" {
                    "moment"
                } else {
                    "force"
                }
            }
            RefValue::Shell { comp, .. } => match comp.as_bytes().first() {
                Some(b'F') => "membrane force",
                Some(b'M') => "plate moment",
                _ => "plate shear",
            },
            RefValue::Period { .. } => "period",
            RefValue::MassRatio { .. } => "mass ratio",
            RefValue::ModeShape { dof, .. } => {
                if dof.starts_with('u') {
                    "mode shape translation"
                } else {
                    "mode shape rotation"
                }
            }
            RefValue::Centre { comp, .. } => {
                if comp.contains("mass") {
                    "diaphragm mass"
                } else {
                    "diaphragm centre"
                }
            }
        }
    }

    /// The combination step this value is for (None for load cases and one-valued combinations).
    fn step(&self) -> Result<Option<Step>, String> {
        let s = match self {
            RefValue::Disp { step, .. }
            | RefValue::Reaction { step, .. }
            | RefValue::Force { step, .. }
            | RefValue::ReactionSum { step, .. }
            | RefValue::BaseReaction { step, .. }
            | RefValue::JointDrift { step, .. }
            | RefValue::StoryDrift { step, .. }
            | RefValue::StoryForce { step, .. }
            | RefValue::Pier { step, .. }
            | RefValue::Spandrel { step, .. }
            | RefValue::Shell { step, .. } => step,
            RefValue::Period { .. }
            | RefValue::MassRatio { .. }
            | RefValue::ModeShape { .. }
            | RefValue::Centre { .. } => return Ok(None),
        };
        match s.as_deref() {
            None => Ok(None),
            Some("max") => Ok(Some(Step::Max)),
            Some("min") => Ok(Some(Step::Min)),
            Some(other) => Err(format!("unknown step \"{other}\" (max or min)")),
        }
    }

    fn label(&self) -> String {
        let step = match self.step() {
            Ok(Some(Step::Max)) => " (max)",
            Ok(Some(Step::Min)) => " (min)",
            _ => "",
        };
        format!("{}{step}", self.label_without_step())
    }

    fn label_without_step(&self) -> String {
        match self {
            RefValue::Disp {
                case, node, dof, ..
            } => format!("{case} disp node {node} {dof}"),
            RefValue::Reaction {
                case, node, dof, ..
            } => format!("{case} reaction node {node} {dof}"),
            RefValue::Force {
                case,
                element,
                at,
                comp,
                ..
            } => {
                let at = match at {
                    At::End(s) => s.clone(),
                    At::Distance(x) => format!("x={x}"),
                };
                format!("{case} element {element} {at} {comp}")
            }
            RefValue::ReactionSum {
                case, nodes, dof, ..
            } => format!("{case} reaction sum over {} joints {dof}", nodes.len()),
            RefValue::BaseReaction { case, dof, .. } => format!("{case} base reaction {dof}"),
            RefValue::JointDrift {
                case, node, dir, ..
            } => format!("{case} joint drift node {node} {dir}"),
            RefValue::StoryDrift {
                case, story, dir, ..
            } => format!("{case} story drift {story} {dir}"),
            RefValue::StoryForce {
                case,
                story,
                at,
                comp,
                ..
            } => format!("{case} story force {story} {at} {comp}"),
            RefValue::Pier {
                case,
                story,
                label,
                at,
                comp,
                ..
            } => format!("{case} pier {label} {story} {at} {comp}"),
            RefValue::Spandrel {
                case,
                story,
                label,
                at,
                comp,
                ..
            } => format!("{case} spandrel {label} {story} {at} {comp}"),
            RefValue::Shell {
                case,
                area,
                node,
                comp,
                ..
            } => format!("{case} area {area} node {node} {comp}"),
            RefValue::Period { mode, .. } => format!("mode {mode} period"),
            RefValue::MassRatio { mode, dof, .. } => format!("mode {mode} mass ratio {dof}"),
            RefValue::ModeShape {
                mode, node, dof, ..
            } => format!("mode {mode} shape node {node} {dof}"),
            RefValue::Centre {
                story,
                diaphragm,
                comp,
                ..
            } => format!("centres {story} {diaphragm} {comp}"),
        }
    }

    fn lookup(&self, results: &Results) -> Result<f64, String> {
        let not_found = || format!("{}: not found in OSAB results", self.label());
        let idx = |names: &[&str], name: &str| {
            names.iter().position(|n| *n == name).ok_or_else(not_found)
        };
        let step = self.step()?;
        match self {
            RefValue::Disp {
                case, node, dof, ..
            } => {
                let d = results
                    .case_step(case, step)
                    .and_then(|c| c.displacement(node))
                    .ok_or_else(not_found)?;
                Ok(d.as_array()[idx(&DOF_NAMES, dof)?])
            }
            RefValue::Reaction {
                case, node, dof, ..
            } => {
                let r = results
                    .case_step(case, step)
                    .and_then(|c| c.reaction(node))
                    .ok_or_else(not_found)?;
                Ok(r.as_array()[idx(&FORCE_NAMES, dof)?])
            }
            RefValue::Force {
                case,
                element,
                at,
                station,
                comp,
                ..
            } => {
                let e = results
                    .case_step(case, step)
                    .and_then(|c| c.frame(element).or_else(|| c.link(element)))
                    .ok_or_else(not_found)?;
                let f = match at {
                    At::End(s) if s == "i" => e.forces_at_side(station.unwrap_or(0.0), Side::Right),
                    At::End(s) if s == "j" => {
                        e.forces_at_side(station.unwrap_or(e.length), Side::Left)
                    }
                    At::End(_) => return Err(not_found()),
                    At::Distance(x) => e.forces_at(*x),
                };
                Ok(f[idx(&STATION_NAMES, comp)?])
            }
            RefValue::ReactionSum {
                case, nodes, dof, ..
            } => {
                let c = results.case_step(case, step).ok_or_else(not_found)?;
                let d = idx(&FORCE_NAMES, dof)?;
                // "*": every joint with a reaction (mesh joints the mesh step restrained included).
                if nodes.len() == 1 && nodes[0].0 == "*" {
                    return Ok(c.reactions.iter().map(|r| r.values.as_array()[d]).sum());
                }
                nodes.iter().try_fold(0.0, |s, n| {
                    Ok(s + c.reaction(n).ok_or_else(not_found)?.as_array()[d])
                })
            }
            RefValue::JointDrift {
                case, node, dir, ..
            } => {
                let d = ["x", "y"]
                    .iter()
                    .position(|x| x == dir)
                    .ok_or_else(not_found)?;
                let j = results
                    .case_step(case, step)
                    .and_then(|c| c.joint_drifts.iter().find(|j| &j.node == node))
                    .ok_or_else(not_found)?;
                Ok(j.drift[d].abs())
            }
            RefValue::StoryDrift {
                case, story, dir, ..
            } => {
                let s = results
                    .case_step(case, step)
                    .and_then(|c| {
                        c.story_drifts
                            .iter()
                            .find(|s| &s.story == story && s.dir.eq_ignore_ascii_case(dir))
                    })
                    .ok_or_else(not_found)?;
                Ok(s.drift)
            }
            RefValue::StoryForce {
                case,
                story,
                at,
                comp,
                ..
            } => {
                let s = results
                    .case_step(case, step)
                    .and_then(|c| c.story_forces.iter().find(|s| &s.story == story))
                    .ok_or_else(not_found)?;
                let v = match at.as_str() {
                    "top" => s.top,
                    "bottom" => s.bottom,
                    _ => return Err(not_found()),
                };
                Ok(v[idx(&crate::story::FORCE_NAMES, comp)?])
            }
            RefValue::Pier {
                case,
                story,
                label,
                at,
                comp,
                ..
            } => {
                let p = results
                    .case_step(case, step)
                    .and_then(|c| {
                        c.pier_forces
                            .iter()
                            .find(|p| &p.story == story && &p.pier == label)
                    })
                    .ok_or_else(not_found)?;
                let v = match at.as_str() {
                    "top" => p.top,
                    "bottom" => p.bottom,
                    _ => return Err(not_found()),
                };
                Ok(v[idx(&crate::piers::FORCE_NAMES, comp)?])
            }
            RefValue::Spandrel {
                case,
                story,
                label,
                at,
                comp,
                ..
            } => {
                let s = results
                    .case_step(case, step)
                    .and_then(|c| {
                        c.spandrel_forces
                            .iter()
                            .find(|s| &s.story == story && &s.spandrel == label)
                    })
                    .ok_or_else(not_found)?;
                let v = match at.as_str() {
                    "left" => s.left,
                    "right" => s.right,
                    _ => return Err(not_found()),
                };
                Ok(v[idx(&crate::piers::FORCE_NAMES, comp)?])
            }
            RefValue::BaseReaction { case, dof, .. } => {
                let b = results
                    .case_step(case, step)
                    .and_then(|c| c.base_reaction)
                    .ok_or_else(not_found)?;
                Ok(b[idx(&FORCE_NAMES, dof)?])
            }
            RefValue::Shell {
                case,
                area,
                element,
                node,
                comp,
                ..
            } => {
                let c = results
                    .case_step(case, step)
                    .and_then(|c| c.shell_corner(area, element.as_ref(), node))
                    .ok_or_else(not_found)?;
                Ok(c.as_array()[idx(&SHELL_NAMES, comp)?])
            }
            RefValue::Period { mode, .. } => Ok(results.mode(*mode).ok_or_else(not_found)?.period),
            RefValue::MassRatio { mode, dof, .. } => {
                Ok(results.mode(*mode).ok_or_else(not_found)?.mass_ratio[idx(&DOF_NAMES, dof)?])
            }
            RefValue::ModeShape {
                mode, node, dof, ..
            } => {
                let d = results
                    .mode(*mode)
                    .and_then(|m| m.shape_at(node))
                    .ok_or_else(not_found)?;
                Ok(d.as_array()[idx(&DOF_NAMES, dof)?])
            }
            RefValue::Centre {
                story,
                diaphragm,
                comp,
                ..
            } => {
                let c = results
                    .diaphragm_centers
                    .iter()
                    .find(|c| &c.story == story && &c.diaphragm == diaphragm)
                    .ok_or_else(not_found)?;
                let v = match comp.as_str() {
                    "mass_x" => Some(c.mass_x),
                    "mass_y" => Some(c.mass_y),
                    "xcm" => c.xcm,
                    "ycm" => c.ycm,
                    "cum_mass_x" => Some(c.cum_mass_x),
                    "cum_mass_y" => Some(c.cum_mass_y),
                    "xccm" => c.xccm,
                    "yccm" => c.yccm,
                    "xcr" => c.xcr,
                    "ycr" => c.ycr,
                    _ => None,
                };
                v.ok_or_else(not_found)
            }
        }
    }
}

/// The sign each mode's shape is compared with: the one that fits the reference values best.
fn shape_signs(refs: &[RefValue], results: &Results) -> std::collections::HashMap<usize, f64> {
    let mut dots: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
    for v in refs {
        if let RefValue::ModeShape { mode, value, .. } = v {
            if let Ok(o) = v.lookup(results) {
                *dots.entry(*mode).or_insert(0.0) += o * value;
            }
        }
    }
    dots.into_iter()
        .map(|(m, d)| (m, if d < 0.0 { -1.0 } else { 1.0 }))
        .collect()
}

/// How a benchmark model is run: the analysis alone, or the mesh step and then the analysis.
pub type Runner<'a> = &'a dyn Fn(&Model) -> Result<Results, String>;

/// Runs one benchmark folder against every reference file it contains.
pub fn run_benchmark(dir: &Path, run: Runner) -> Result<Vec<Check>, String> {
    let name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let read =
        |p: PathBuf| std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()));
    let model = serde_json::from_str::<Model>(&read(dir.join("model.json"))?)
        .map_err(|e| format!("{name}: {e}"))?;
    let results = run(&model).map_err(|e| format!("{name}: {e}"))?;
    // Results at other mesh sizes, for references by convergence.
    let mut at_size: Vec<(f64, Results)> = Vec::new();

    let mut checks = Vec::new();
    for file in reference_files(dir) {
        let file = file.as_str();
        let path = dir.join(file);
        let refs: ReferenceFile =
            serde_json::from_str(&read(path)?).map_err(|e| format!("{name}/{file}: {e}"))?;
        let floor = |class: &str| {
            FLOOR_FRACTION
                * refs
                    .values
                    .iter()
                    .filter(|v| v.class() == class)
                    .fold(0.0f64, |m, v| m.max(v.value().abs()))
        };
        let mut source = file.trim_end_matches(".json").to_string();
        let mut sized: Vec<&Results> = Vec::new();
        if let Some(c) = &refs.convergence {
            if c.sizes.len() != 3 {
                return Err(format!("{name}/{file}: convergence needs three sizes"));
            }
            for &h in &c.sizes {
                if !at_size.iter().any(|(s, _)| *s == h) {
                    let r = run(&with_floor_size(&model, h)).map_err(|e| format!("{name}: {e}"))?;
                    at_size.push((h, r));
                }
            }
            sized = c
                .sizes
                .iter()
                .map(|h| &at_size.iter().find(|(s, _)| s == h).unwrap().1)
                .collect();
            source += " (extrapolated)";
        }
        let signs = shape_signs(&refs.values, &results);
        for v in &refs.values {
            let look = |r: &Results| v.lookup(r).map_err(|e| format!("{name}/{file}: {e}"));
            let mut osab = if sized.is_empty() {
                look(&results)?
            } else {
                extrapolate([look(sized[0])?, look(sized[1])?, look(sized[2])?])
            };
            if let RefValue::ModeShape { mode, .. } = v {
                osab *= signs.get(mode).copied().unwrap_or(1.0);
            }
            let tolerance = REL_TOL * v.value().abs() + floor(v.class()) + ABS_FLOOR;
            checks.push(Check {
                benchmark: name.clone(),
                source: source.clone(),
                label: v.label(),
                osab,
                reference: v.value(),
                tolerance,
                pass: (osab - v.value()).abs() <= tolerance,
            });
        }
    }
    Ok(checks)
}

/// Runs every benchmark folder under `root` (sorted by name).
pub fn run_all(root: &Path, run: Runner) -> Result<Vec<Check>, String> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("model.json").exists())
        .collect();
    dirs.sort();
    let mut all = Vec::new();
    for d in dirs {
        all.extend(run_benchmark(&d, run)?);
    }
    Ok(all)
}

/// One line per benchmark and reference source, then every failed value.
pub fn report(checks: &[Check]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<34} {:<9} {:>6} {:>12}  result",
        "benchmark", "reference", "values", "max error"
    );
    let mut groups: Vec<(&str, &str)> = checks
        .iter()
        .map(|c| (c.benchmark.as_str(), c.source.as_str()))
        .collect();
    groups.dedup();
    for (b, s) in groups {
        let group: Vec<&Check> = checks
            .iter()
            .filter(|c| c.benchmark == b && c.source == s)
            .collect();
        let worst = group
            .iter()
            .filter(|c| c.reference.abs() > c.tolerance)
            .map(|c| c.rel_error())
            .fold(0.0f64, f64::max);
        let failed = group.iter().filter(|c| !c.pass).count();
        let verdict = if failed == 0 {
            "ok".to_string()
        } else {
            format!("FAIL ({failed})")
        };
        let _ = writeln!(
            out,
            "{b:<34} {s:<9} {:>6} {:>11.5}%  {verdict}",
            group.len(),
            worst * 100.0
        );
    }
    let failed: Vec<&Check> = checks.iter().filter(|c| !c.pass).collect();
    if !failed.is_empty() {
        let _ = writeln!(out, "\nFailed values:");
        for c in failed {
            let _ = writeln!(
                out,
                "  {} [{}] {}: osab {:.6e}, reference {:.6e}, allowed +/- {:.2e}",
                c.benchmark, c.source, c.label, c.osab, c.reference, c.tolerance
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::extrapolate;

    #[test]
    fn extrapolation_removes_the_observed_error_term() {
        // f(h) = 1 + h^2 at h = 1, 1/2, 1/4: rate 2, limit 1.
        let f = |h: f64| 1.0 + h * h;
        assert!((extrapolate([f(1.0), f(0.5), f(0.25)]) - 1.0).abs() < 1e-12);
        // Rate 1.5 is used as observed.
        let g = |h: f64| 2.0 - 0.3 * h.powf(1.5);
        assert!((extrapolate([g(1.0), g(0.5), g(0.25)]) - 2.0).abs() < 1e-12);
        // Rates above 2 are taken as 2, below 1 as 1; no clear rate: the finest value.
        let e = |h: f64| 1.0 + h.powi(3);
        let x = extrapolate([e(1.0), e(0.5), e(0.25)]);
        assert!((x - (e(0.25) - (e(0.5) - e(0.25)) / 3.0)).abs() < 1e-12);
        assert_eq!(extrapolate([1.0, 2.0, 1.5]), 1.5);
    }
}
