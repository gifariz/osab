//! Analysis results (written as JSON) and helpers to look values up.

use serde::Serialize;

use crate::elements::{internal_forces, LocalLoad, Side};
use crate::model::{ComboType, Id, Units};

/// A joint and direction without stiffness (see [`Results::instabilities`]). `digits_lost`: about how
/// many of the 16 significant digits had cancelled there (16: nothing was left).
#[derive(Debug, Clone, Serialize)]
pub struct Instability {
    pub node: Id,
    pub dof: &'static str,
    pub digits_lost: f64,
}

/// A case that failed (see [`Results::failed`]) and why.
#[derive(Debug, Clone, Serialize)]
pub struct FailedCase {
    pub case: Id,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Results {
    pub title: String,
    pub units: Units,
    /// Messages worth knowing, e.g. DOFs that were restrained automatically.
    pub notes: Vec<String>,
    /// Directions in which the structure has no stiffness and a tiny stiffness was put in to go on
    /// (see `on_instability` in the model): the results are not reliable at and near them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub instabilities: Vec<Instability>,
    /// Cases that could not be analysed while the others were (the modal case): they have no results,
    /// nor have the response spectrum cases and combinations that need them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failed: Vec<FailedCase>,
    /// Load cases, spectrum cases, then load combinations (enveloped ones twice: step max, then step min).
    pub cases: Vec<CaseResults>,
    /// The analysis mesh made by the mesh step (for drawing); missing when nothing was meshed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<MeshInfo>,
    /// The modal case's results; missing when the model has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modal: Option<ModalResults>,
    /// Per story and diaphragm: masses, centres of mass and of rigidity (missing without diaphragms).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diaphragm_centers: Vec<DiaphragmCentre>,
    /// Piers' axis angles and cut centroids, spandrels' cut centroids (see [`crate::piers`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pier_sections: Vec<crate::piers::PierSection>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spandrel_sections: Vec<crate::piers::SpandrelSection>,
}

/// One row of the centres of mass and rigidity (docs/phases/DIAPHRAGM_CENTERS_PLAN.md): the lateral masses
/// of the diaphragm's joints at the story (after the mass source and lumping), the centre of mass (x
/// weighted by the Y masses, y by the X masses), the same for the diaphragm at this story and every story
/// above (cumulative), and the centre of rigidity: the point where a force on this diaphragm alone (the
/// others free) moves it without turning it. A semi-rigid diaphragm keeps its mass at its joints: masses
/// and cumulative values 0, no cumulative centre and no centre of rigidity, the centre of mass of its joints.
#[derive(Debug, Clone, Serialize)]
pub struct DiaphragmCentre {
    pub story: String,
    pub diaphragm: Id,
    pub mass_x: f64,
    pub mass_y: f64,
    pub xcm: Option<f64>,
    pub ycm: Option<f64>,
    pub cum_mass_x: f64,
    pub cum_mass_y: f64,
    pub xccm: Option<f64>,
    pub yccm: Option<f64>,
    pub xcr: Option<f64>,
    pub ycr: Option<f64>,
}

/// Results of a modal (eigen) case, as in the usual modal tables.
#[derive(Debug, Clone, Serialize)]
pub struct ModalResults {
    pub id: Id,
    pub modes: Vec<Mode>,
    /// Joint masses as assembled (after lumping at stories), UX UY UZ RX RY RZ; joints with none left out.
    pub masses: Vec<crate::model::JointMass>,
    /// Total free mass for each direction of the participating mass ratios (translations; rotations as
    /// mass moments of inertia about the centre of mass).
    pub free_mass: [f64; 6],
    /// Centre of all the model's mass, the point the rotation ratios are about.
    pub centre_of_mass: [f64; 3],
    /// Modal load participation ratios: the accelerations UX, UY, UZ, then a Ritz case's starting load
    /// patterns.
    pub load_participation: Vec<LoadParticipation>,
}

/// How much of a load the modes reach, in percent: static (of its static response's strain energy) and
/// dynamic (of its part on massive coordinates; an acceleration's is its cumulative mass ratio).
#[derive(Debug, Clone, Serialize)]
pub struct LoadParticipation {
    /// `"acceleration"` or `"pattern"`.
    pub kind: String,
    pub name: String,
    pub static_percent: f64,
    pub dynamic_percent: f64,
}

/// One mode: period, frequencies, participation and the shape at unit modal mass (φᵀ M φ = 1 in the
/// model's units; the largest translation positive).
#[derive(Debug, Clone, Serialize)]
pub struct Mode {
    pub mode: usize,
    /// Period (s), frequency (cycles/s), circular frequency (rad/s) and eigenvalue ω² (rad²/s²).
    pub period: f64,
    pub frequency: f64,
    pub circular_frequency: f64,
    pub eigenvalue: f64,
    /// Modal participation factors φᵀ M r, UX UY UZ RX RY RZ.
    pub participation: [f64; 6],
    /// Participating mass ratios (φᵀ M r)² / rᵀ M r over the free mass, and their running sums.
    pub mass_ratio: [f64; 6],
    pub sum_mass_ratio: [f64; 6],
    pub shape: Vec<NodeValues<Disp>>,
    /// A static mode of a Ritz case (a load on massless coordinates): no mass, no period (reported 0),
    /// the shape at unit stiffness (φᵀ K φ = 1); spectrum cases leave it out.
    #[serde(skip_serializing_if = "is_false_ref")]
    pub static_mode: bool,
}

fn is_false_ref(v: &bool) -> bool {
    !*v
}

/// Mesh joints, shell elements and frame pieces with their parent objects (the mesh step's output).
#[derive(Debug, Clone, Default, Serialize)]
pub struct MeshInfo {
    /// Joints made by the mesh step (`~1, ~2…`); object joints are in the model.
    pub joints: Vec<MeshJoint>,
    pub shells: Vec<MeshElement>,
    pub frames: Vec<MeshElement>,
    /// Joints tied to an element's edge (edge constraints).
    pub edge_constraints: Vec<crate::model::EdgeConstraint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeshJoint {
    pub id: Id,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A shell element (`parent` = its area) or frame piece (`parent` = its frame) and its joints.
#[derive(Debug, Clone, Serialize)]
pub struct MeshElement {
    pub id: Id,
    pub parent: Id,
    pub nodes: Vec<Id>,
}

/// Which bound of a combination with a maximum and a minimum (step type Max / Min).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Max,
    Min,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseResults {
    pub id: Id,
    /// Set for a load combination: its type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combination: Option<ComboType>,
    /// Set when the combination has a maximum and a minimum: which of the two this is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<Step>,
    /// A response spectrum case: every value is a size (positive), combined over the modes and the
    /// directions on its own.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub spectrum: bool,
    pub displacements: Vec<NodeValues<Disp>>,
    pub reactions: Vec<NodeValues<Force>>,
    /// Frame element results (`element` holds the frame id; in 3a every frame is one element).
    pub frames: Vec<ElementResults>,
    /// Forces in the links, same fields as `frames` (`element` holds the link id).
    pub links: Vec<ElementResults>,
    /// Shell element forces per corner (one element per area in Phase 3a).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shells: Vec<ShellResults>,
    /// Global checks; not given for an envelope (max/min), where they do not apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equilibrium: Option<Equilibrium>,
    /// Base reactions: the sum of the support reactions and spring forces, about the origin, FX FY FZ MX
    /// MY MZ (a spectrum case combines its modes' sums; a combination's max or min, value by value).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_reaction: Option<[f64; 6]>,
    /// Story results of a model with story levels (see [`crate::story`]): joint drifts (signed, a
    /// spectrum case's as sizes), the largest of each story by size, story forces at each story's top and
    /// bottom.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub joint_drifts: Vec<crate::story::JointDrift>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub story_drifts: Vec<crate::story::StoryDrift>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub story_forces: Vec<crate::story::StoryForce>,
    /// Pier forces at each pier's top and bottom, spandrel forces at each spandrel's left and right end
    /// (P V2 V3 T M2 M3, see [`crate::piers`]; a spectrum case's as sizes).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pier_forces: Vec<crate::piers::PierForce>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spandrel_forces: Vec<crate::piers::SpandrelForce>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeValues<T> {
    pub node: Id,
    #[serde(flatten)]
    pub values: T,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Disp {
    pub ux: f64,
    pub uy: f64,
    pub uz: f64,
    pub rx: f64,
    pub ry: f64,
    pub rz: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Force {
    pub fx: f64,
    pub fy: f64,
    pub fz: f64,
    pub mx: f64,
    pub my: f64,
    pub mz: f64,
}

impl From<[f64; 6]> for Disp {
    fn from(v: [f64; 6]) -> Self {
        Disp {
            ux: v[0],
            uy: v[1],
            uz: v[2],
            rx: v[3],
            ry: v[4],
            rz: v[5],
        }
    }
}
impl Disp {
    pub fn as_array(&self) -> [f64; 6] {
        [self.ux, self.uy, self.uz, self.rx, self.ry, self.rz]
    }
}
impl From<[f64; 6]> for Force {
    fn from(v: [f64; 6]) -> Self {
        Force {
            fx: v[0],
            fy: v[1],
            fz: v[2],
            mx: v[3],
            my: v[4],
            mz: v[5],
        }
    }
}
impl Force {
    pub fn as_array(&self) -> [f64; 6] {
        [self.fx, self.fy, self.fz, self.mx, self.my, self.mz]
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ElementResults {
    pub element: Id,
    /// End i and end j as analysed (after the end-order rule).
    pub nodes: [Id; 2],
    pub length: f64,
    /// End length offsets at end i and j: the stations cover the clear length between them
    /// (frame forces over the clear length).
    #[serde(skip_serializing_if = "no_offsets")]
    pub offsets: [f64; 2],
    /// Unit vectors of local axes 1, 2 and 3 in global coordinates (rows), as analysed.
    pub axes: [[f64; 3]; 3],
    pub stations: Vec<Station>,
    /// Local end forces on the element (nodes acting on it), kept for queries at any x.
    #[serde(skip)]
    pub end_forces: [f64; 12],
    #[serde(skip)]
    pub loads: Vec<LocalLoad>,
    /// Local end displacements (frames), for deflections at any x.
    #[serde(skip)]
    pub end_displacements: [f64; 12],
    /// Hinges inside the member (releases at the clear-length ends of a member with end offsets); see
    /// `elements::local_displacements`.
    #[serde(skip)]
    pub hinges: crate::elements::frame::Hinges,
    /// Values exist only at the stations (a combination's max or min): queries read the stations.
    #[serde(skip)]
    pub enveloped: bool,
    /// A frame object split into pieces by the mesh step: the pieces' own results, in order from end i
    /// (queries then go to the piece at x; `end_forces`, `loads` and `end_displacements` are unused).
    #[serde(skip)]
    pub pieces: Vec<ElementResults>,
    /// With P-Delta from loads: its moments carry the P-Delta axial force times the member's own
    /// deflection.
    #[serde(skip)]
    pub pdelta: Option<Box<PDeltaShape>>,
}

/// What a frame's moments need with P-Delta from loads: its properties (the P-Delta axial force among
/// them), its end displacements and its hinges, in the axes it is analysed in; and the turn from those
/// axes to the axes its results are given in (a section with a product of inertia is analysed in its
/// principal axes).
#[derive(Debug, Clone, Copy)]
pub struct PDeltaShape {
    pub props: crate::elements::Props,
    pub ul: [f64; 12],
    pub hinges: crate::elements::frame::Hinges,
    pub turn: f64,
}

impl PDeltaShape {
    /// The P-Delta part [M2, M3] of the moments at `x` from end i (`frame::pdelta_moments`).
    fn moments(&self, x: f64) -> [f64; 2] {
        let [m2, m3] =
            crate::elements::frame::pdelta_moments(&self.props, &self.ul, &self.hinges, x);
        let (s, c) = self.turn.sin_cos();
        [c * m2 + s * m3, -s * m2 + c * m3]
    }
}

/// Internal forces at distance `x` from end i (common sign convention), and the displacement of the
/// member axis there (global axes, including the deflection caused by member loads).
#[derive(Debug, Clone, Copy, Serialize)]
#[allow(non_snake_case)]
pub struct Station {
    pub x: f64,
    pub P: f64,
    pub V2: f64,
    pub V3: f64,
    pub T: f64,
    pub M2: f64,
    pub M3: f64,
    pub ux: f64,
    pub uy: f64,
    pub uz: f64,
}

pub const STATION_NAMES: [&str; 6] = ["P", "V2", "V3", "T", "M2", "M3"];

/// Shell force names at a corner, in the usual order.
pub const SHELL_NAMES: [&str; 8] = ["F11", "F22", "F12", "M11", "M22", "M12", "V13", "V23"];

/// Forces in one shell element (not averaged between elements): per unit length, in the area's local axes,
/// at each corner (2 x 2 Gauss values extrapolated, not averaged with neighbours).
/// F: membrane forces (tension +); M: moments, positive = tension at the bottom (-3 face);
/// V: transverse shears.
#[derive(Debug, Clone, Serialize)]
pub struct ShellResults {
    /// Shell element id (the area id in Phase 3a).
    pub element: Id,
    /// Parent area object.
    pub area: Id,
    /// Corners as analysed (starting at the first lowest corner).
    pub nodes: Vec<Id>,
    /// Unit vectors of local 1, 2, 3 in global coordinates (rows).
    pub axes: [[f64; 3]; 3],
    pub corners: Vec<ShellCorner>,
}

#[derive(Debug, Clone, Serialize)]
#[allow(non_snake_case)]
pub struct ShellCorner {
    pub node: Id,
    pub F11: f64,
    pub F22: f64,
    pub F12: f64,
    pub M11: f64,
    pub M22: f64,
    pub M12: f64,
    pub V13: f64,
    pub V23: f64,
}

impl ShellCorner {
    pub fn new(node: Id, v: [f64; 8]) -> Self {
        ShellCorner {
            node,
            F11: v[0],
            F22: v[1],
            F12: v[2],
            M11: v[3],
            M22: v[4],
            M12: v[5],
            V13: v[6],
            V23: v[7],
        }
    }

    pub fn as_array(&self) -> [f64; 8] {
        [
            self.F11, self.F22, self.F12, self.M11, self.M22, self.M12, self.V13, self.V23,
        ]
    }
}

impl ElementResults {
    /// [P, V2, V3, T, M2, M3] at distance `x` from end i, taken inside the member: just after end i
    /// at x = 0, otherwise just before x (a point load exactly at x is not yet included).
    pub fn forces_at(&self, x: f64) -> [f64; 6] {
        let side = if x <= 0.0 { Side::Right } else { Side::Left };
        self.forces_at_side(x, side)
    }

    /// Like [`Self::forces_at`], choosing the side of a point load that sits exactly at `x`.
    /// For an envelope, the station at `x` (the first or last of a pair at a point load, by `side`).
    pub fn forces_at_side(&self, x: f64, side: Side) -> [f64; 6] {
        if !self.pieces.is_empty() {
            // The piece holding x; at a joint between two pieces, the one before it (Left) or after.
            let tol = 1e-9 * self.length.max(1.0);
            let mut start = 0.0;
            let last = self.pieces.len() - 1;
            for (k, p) in self.pieces.iter().enumerate() {
                let end = start + p.length;
                let inside = match side {
                    Side::Left => x <= end + tol,
                    Side::Right => x < end - tol,
                };
                if inside || k == last {
                    return p.forces_at_side((x - start).clamp(0.0, p.length), side);
                }
                start = end;
            }
        }
        if !self.enveloped {
            let mut f = internal_forces(&self.end_forces, &self.loads, x, side);
            if let Some(p) = &self.pdelta {
                let [m2, m3] = p.moments(x);
                f[4] += m2;
                f[5] += m3;
            }
            return f;
        }
        let tol = 1e-9 * self.length.max(1.0);
        let at: Vec<&Station> = self
            .stations
            .iter()
            .filter(|s| (s.x - x).abs() <= tol)
            .collect();
        let s = match side {
            Side::Left => at.first(),
            Side::Right => at.last(),
        };
        s.map(|s| [s.P, s.V2, s.V3, s.T, s.M2, s.M3])
            .unwrap_or([f64::NAN; 6])
    }

    /// Positions inside the member of point loads.
    pub fn point_positions(&self) -> Vec<f64> {
        let l = self.length;
        let tol = 1e-9 * l;
        self.loads
            .iter()
            .filter_map(|ld| match *ld {
                LocalLoad::Point { at, .. } if at > tol && at < l - tol => Some(at),
                _ => None,
            })
            .collect()
    }

    fn station_at(&self, x: f64, side: Side) -> Station {
        let f = self.forces_at_side(x, side);
        Station {
            x,
            P: f[0],
            V2: f[1],
            V3: f[2],
            T: f[3],
            M2: f[4],
            M3: f[5],
            ux: 0.0,
            uy: 0.0,
            uz: 0.0,
        }
    }

    /// Fills `stations`: `n` equally spaced points (ends included), plus both sides of every point
    /// load inside the member, so diagrams show the jump and the peak at the load.
    pub fn make_stations(&mut self, n: usize) {
        let at = station_layout(
            self.offsets[0],
            self.length - self.offsets[1],
            self.point_positions(),
            n,
        );
        self.stations = at.into_iter().map(|(x, s)| self.station_at(x, s)).collect();
    }

    /// Forces at the given positions (deflections are filled by the caller).
    pub fn set_stations(&mut self, at: &[(f64, Side)]) {
        self.stations = at.iter().map(|&(x, s)| self.station_at(x, s)).collect();
    }
}

fn no_offsets(o: &[f64; 2]) -> bool {
    *o == [0.0, 0.0]
}

/// Station positions between `a` and `b` (the clear length; ends included): `n` equally spaced points,
/// plus both sides of every point in `points` (point loads) that lies between them.
pub fn station_layout(a: f64, b: f64, mut points: Vec<f64>, n: usize) -> Vec<(f64, Side)> {
    let tol = 1e-9 * b.max(1.0);
    points.retain(|p| *p > a + tol && *p < b - tol);
    points.sort_by(|a, b| a.total_cmp(b));
    points.dedup_by(|a, b| (*a - *b).abs() <= tol);
    let mut at: Vec<(f64, Side)> = (0..n)
        .map(|s| a + (b - a) * s as f64 / (n - 1) as f64)
        .filter(|x| points.iter().all(|p| (p - x).abs() > tol))
        .map(|x| (x, if x <= a { Side::Right } else { Side::Left }))
        .collect();
    for p in points {
        at.push((p, Side::Left));
        at.push((p, Side::Right));
    }
    at.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then((a.1 == Side::Right).cmp(&(b.1 == Side::Right)))
    });
    at
}

/// Global checks for one load case (forces and moments about the global origin).
#[derive(Debug, Clone, Serialize)]
pub struct Equilibrium {
    /// Resultant of all applied loads [Fx, Fy, Fz, Mx, My, Mz].
    pub applied: [f64; 6],
    /// Resultant of all support reactions (every restrained DOF, including automatic ones).
    pub reactions: [f64; 6],
    /// Largest |applied + reactions|, relative to the largest applied force or moment.
    pub relative_error: f64,
    /// Largest out-of-balance force at a free DOF (K u - F), relative to the same scale.
    /// Measures how accurately the equations were solved.
    pub residual: f64,
}

impl Equilibrium {
    /// Both checks pass at this relative level.
    pub const TOLERANCE: f64 = 1e-8;

    pub fn is_ok(&self) -> bool {
        self.relative_error <= Self::TOLERANCE && self.residual <= Self::TOLERANCE
    }
}

impl Results {
    /// A load case or a one-valued combination (the max of an enveloped one).
    pub fn case(&self, id: &Id) -> Option<&CaseResults> {
        self.cases.iter().find(|c| &c.id == id)
    }

    /// A result by case or combination id and, for an enveloped combination, its step.
    pub fn case_step(&self, id: &Id, step: Option<Step>) -> Option<&CaseResults> {
        self.cases.iter().find(|c| &c.id == id && c.step == step)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("results serialize")
    }

    /// Mode `n` (1-based) of the modal case.
    pub fn mode(&self, n: usize) -> Option<&Mode> {
        self.modal.as_ref()?.modes.iter().find(|m| m.mode == n)
    }
}

impl Mode {
    pub fn shape_at(&self, node: &Id) -> Option<&Disp> {
        self.shape
            .iter()
            .find(|d| &d.node == node)
            .map(|d| &d.values)
    }
}

impl CaseResults {
    /// The equilibrium check passed (false for an envelope, which has none).
    pub fn equilibrium_ok(&self) -> bool {
        self.equilibrium.as_ref().is_some_and(|e| e.is_ok())
    }
    pub fn displacement(&self, node: &Id) -> Option<&Disp> {
        self.displacements
            .iter()
            .find(|d| &d.node == node)
            .map(|d| &d.values)
    }
    pub fn reaction(&self, node: &Id) -> Option<&Force> {
        self.reactions
            .iter()
            .find(|r| &r.node == node)
            .map(|r| &r.values)
    }
    pub fn frame(&self, id: &Id) -> Option<&ElementResults> {
        self.frames.iter().find(|e| &e.element == id)
    }
    pub fn link(&self, id: &Id) -> Option<&ElementResults> {
        self.links.iter().find(|e| &e.element == id)
    }
    /// Forces of area `area` at corner `node`: of shell element `element` when given (a meshed area),
    /// else of the area's first element with that corner.
    pub fn shell_corner(&self, area: &Id, element: Option<&Id>, node: &Id) -> Option<&ShellCorner> {
        self.shells
            .iter()
            .filter(|s| &s.area == area && element.is_none_or(|e| &s.element == e))
            .flat_map(|s| s.corners.iter())
            .find(|c| &c.node == node)
    }
}
