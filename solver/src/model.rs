//! Model file format (JSON). See docs/MODEL_FORMAT.md for the user-facing description.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::Error;

/// Name of a joint, frame, link, material, section or load case.
/// The file may use a number or a string; both are stored as text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Id(pub String);

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Text(String),
            Int(i64),
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Text(s) => Id(s),
            Raw::Int(i) => Id(i.to_string()),
        })
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Id {
    fn from(s: &str) -> Self {
        Id(s.to_string())
    }
}

impl Id {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Dimension {
    /// Plane frame in the global X-Z plane (Z up). UY, RX and RZ are restrained everywhere.
    #[serde(rename = "2D")]
    TwoD,
    #[default]
    #[serde(rename = "3D")]
    ThreeD,
    /// Plane grid in the X-Y plane (a floor with its beams): UX, UY and RZ are restrained everywhere, so only
    /// UZ, RX and RY are active. Every node needs the same Z.
    #[serde(rename = "grid")]
    Grid,
}

impl Dimension {
    /// The DOFs (0..6 = ux, uy, uz, rx, ry, rz) held at every node.
    pub fn inactive(&self) -> &'static [usize] {
        match self {
            Dimension::TwoD => &[1, 3, 5],
            Dimension::Grid => &[0, 1, 5],
            Dimension::ThreeD => &[],
        }
    }

    /// Words for messages ("a 2D model", "a plane grid model").
    pub fn name(&self) -> &'static str {
        match self {
            Dimension::TwoD => "a 2D model",
            Dimension::Grid => "a plane grid model",
            Dimension::ThreeD => "a 3D model",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Units {
    pub force: String,
    pub length: String,
}

impl Units {
    /// Metres per unit of the length unit.
    pub fn metres(&self) -> f64 {
        match self.length.as_str() {
            "mm" => 0.001,
            "cm" => 0.01,
            "in" => 0.0254,
            "ft" => 0.3048,
            _ => 1.0,
        }
    }

    /// How far off an area edge a joint may lie and still be tied to it: 0.1 mm, whatever the edge's
    /// length, in the model's length unit.
    pub fn edge_tie(&self) -> f64 {
        1e-4 / self.metres()
    }

    /// How far apart two frames may pass and still be joined where they cross, and how far a frame may pass
    /// from an area edge and still be split where it crosses it: 1 mm, whatever the lengths, in the model's
    /// length unit.
    pub fn frame_join(&self) -> f64 {
        1e-3 / self.metres()
    }

    /// Standard gravity in the model's length unit per s² (mass = weight / g).
    pub fn gravity(&self) -> f64 {
        9.80665
            / match self.length.as_str() {
                "mm" => 0.001,
                "cm" => 0.01,
                "in" => 0.0254,
                "ft" => 0.3048,
                _ => 1.0,
            }
    }
}

impl Default for Units {
    fn default() -> Self {
        Units {
            force: "kN".into(),
            length: "m".into(),
        }
    }
}

/// The model file format this OSAB reads and writes. Files without `version` are older ones (format 1 and
/// before: the old field names still read). A change to the file's layout raises it, with an upgrade step
/// for the files before (docs/MODEL_FORMAT.md, *File format version*).
pub const MODEL_FORMAT: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// File format version (`MODEL_FORMAT`); missing = an older file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub dimension: Dimension,
    #[serde(default)]
    pub units: Units,
    pub materials: Vec<Material>,
    pub sections: Vec<Section>,
    pub nodes: Vec<Node>,
    /// Frame objects (frames and trusses). Older files call this list `elements`.
    #[serde(default, alias = "elements")]
    pub frames: Vec<Frame>,
    #[serde(default)]
    pub supports: Vec<Support>,
    /// Load patterns: named sets of loads with a type and a self-weight multiplier. Load cases apply
    /// them times scale factors. Files without patterns keep their loads on the load cases.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_patterns: Vec<LoadPattern>,
    #[serde(default)]
    pub load_cases: Vec<LoadCase>,
    /// Load combinations of load cases and other combinations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_combinations: Vec<LoadCombination>,
    /// Joint springs (global axes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub springs: Vec<Spring>,
    /// Line spring properties (stiffness per unit length), assigned to frames by `line_spring`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_spring_props: Vec<LineSpringProp>,
    /// Area spring properties (stiffness per unit area), assigned to areas by `area_spring`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub area_spring_props: Vec<AreaSpringProp>,
    /// Links between two joints (rigid for now).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
    /// Area sections (shell properties) for slabs and walls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub area_sections: Vec<AreaSection>,
    /// Area objects (floors and walls). In Phase 3a every area is analysed as one shell element.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<Area>,
    /// Joints tied to an area's edge (edge constraints). The mesh step finds them; see
    /// [`crate::elements::edge`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edge_constraints: Vec<EdgeConstraint>,
    /// Auto-mesh settings, used by the mesh step (crate osab-mesh) before the analysis; missing =
    /// the defaults, `"off"` (older files) = every area set to no mesh. The solver itself ignores them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<MeshSetting>,
    /// Added joint masses, global axes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masses: Vec<JointMass>,
    /// Where the mass of a modal analysis comes from (the mass source); missing = the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_source: Option<MassSource>,
    /// A modal (eigen) case; missing = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal: Option<ModalCase>,
    /// P-Delta options; missing = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_delta: Option<PDelta>,
    /// What the analysis does when the structure has no stiffness in some direction; missing =
    /// continue with a warning.
    #[serde(default, skip_serializing_if = "OnInstability::is_default")]
    pub on_instability: OnInstability,
    /// Response spectrum functions: a value against the period.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spectrum_functions: Vec<crate::spectrum::SpectrumFunction>,
    /// Response spectrum cases on the modal case; load combinations may name them like load cases.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spectrum_cases: Vec<crate::spectrum::SpectrumCase>,
    /// Cases set not to run (load cases, the modal case, spectrum cases): they have no results, nor do
    /// the combinations that use them. The modal case runs anyway while a spectrum case runs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_run: Vec<Id>,
    /// Rigid diaphragms (the constraint is per diaphragm and story; joints and areas name one).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diaphragms: Vec<Diaphragm>,
    /// Story levels, lowest (the base) first. A diaphragm joint between two levels belongs to the story
    /// above; without stories every height with diaphragm joints is a level of its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stories: Vec<Story>,
    /// The two angle limits for object types (missing = 1 and 20 degrees). The floor/wall
    /// limit decides whether an area is a floor or a wall in the mesh step and the solver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_limits: Option<AngleLimits>,
    /// Groups: named sets of objects (an object may be in several). The analysis does not use them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
    /// Pier labels: walls and columns with the same label at a story make one pier, whose forces the
    /// results give at its top and bottom (see [`crate::piers`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pier_labels: Vec<PierLabel>,
    /// Spandrel labels: walls and beams with the same label make a spandrel, with forces at its left and
    /// right ends.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spandrel_labels: Vec<SpandrelLabel>,
    /// Set by the mesh step: the piers and spandrels found from the objects, with their analysis
    /// elements (missing = found from this model's own areas and frames).
    #[serde(skip)]
    pub wall_sections: Option<Vec<crate::piers::WallSection>>,
    /// Desktop-app data (grid lines, view settings). The solver keeps it on round-trip and ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gui: Option<serde_json::Value>,
}

/// A pier label: a name and a colour for the app (missing = its own).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PierLabel {
    pub id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// A spandrel label. `multi_story`: its walls and beams on every story make one spandrel (otherwise
/// one per story, each wall at the story level nearest its middle height).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpandrelLabel {
    pub id: Id,
    #[serde(default, skip_serializing_if = "is_false")]
    pub multi_story: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// A group: a name, a colour for the app (missing = its own), and the objects in it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Id>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<Id>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Id>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub id: Id,
    #[serde(rename = "E")]
    pub e: f64,
    /// Shear modulus. If missing, computed from `nu`.
    #[serde(rename = "G", default, skip_serializing_if = "Option::is_none")]
    pub g: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nu: Option<f64>,
    /// Weight per unit volume (force / length^3), for self-weight; missing = 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub weight: f64,
    /// Material type (steel, concrete, other): a label with its own strengths; the analysis uses neither.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<MaterialType>,
    /// Steel yield and tensile strength, concrete compressive strength (stress); kept, not used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fy: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fu: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fc: Option<f64>,
}

/// Material type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterialType {
    Steel,
    Concrete,
    Other,
}

impl Material {
    pub fn shear_modulus(&self) -> f64 {
        match (self.g, self.nu) {
            (Some(g), _) => g,
            (None, Some(nu)) => self.e / (2.0 * (1.0 + nu)),
            (None, None) => 0.0,
        }
    }

    /// Poisson's ratio: `nu`, or from G = E / 2(1 + nu) when only G is given.
    pub fn poisson(&self) -> f64 {
        match (self.nu, self.g) {
            (Some(nu), _) => nu,
            (None, Some(g)) => self.e / (2.0 * g) - 1.0,
            (None, None) => 0.0,
        }
    }
}

/// Shell type of an area section, named by the usual shell types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShellType {
    /// Membrane + thin-plate (Kirchhoff, DKQ) bending.
    ShellThin,
    /// Membrane + thick-plate (Mindlin) bending (elements/shell_thick.rs).
    ShellThick,
    /// In-plane forces and the drilling moment only.
    Membrane,
}

/// Area section: shell type, material and thickness (membrane thickness = bending thickness).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AreaSection {
    pub id: Id,
    #[serde(rename = "type")]
    pub kind: ShellType,
    pub material: Id,
    pub thickness: f64,
    /// Property type (slab or wall): a label for users and design; it changes no result (an area is a floor
    /// or a wall by its slope). Missing in older files: the app gives one from the areas using the section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property_type: Option<AreaPropertyType>,
    /// Slab type, used by the mesh step where floors overlap: a Drop wins over a Slab. Slab sections only
    /// (see [`AreaSection::slab_kind`]).
    #[serde(default, skip_serializing_if = "SlabType::is_slab")]
    pub slab_type: SlabType,
    /// Shell property modifiers of the section; times the area object's own. A Stiff section
    /// uses [`AreaModifiers::STIFF`] instead (fixed values, applied at analysis).
    #[serde(default, skip_serializing_if = "AreaModifiers::is_one")]
    pub modifiers: AreaModifiers,
    /// *Use special one-way load distribution* (Membrane only): a Membrane floor's vertical
    /// loads go to its panels' edges as strips along local axis 1, half of each strip to each end,
    /// instead of two-way (docs/elements/ONE_WAY_LOAD.md).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub one_way: bool,
    /// Deck sections only (property type Deck): the deck's geometry and materials. A deck is always a
    /// one-way Membrane floor; `material` is the fill's concrete and `thickness` the total depth (slab depth +
    /// rib depth), for display: its weight and in-plane stiffness come from here (see [`Deck`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck: Option<Deck>,
}

/// Property type of an area section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AreaPropertyType {
    Slab,
    Deck,
    Wall,
}

/// Deck type: concrete on a steel deck, the steel deck alone, or a solid slab analysed as a deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeckType {
    Filled,
    Unfilled,
    Solid,
}

/// A deck section's data (lengths in the model's units; `unit_weight` is the steel deck's weight per area).
/// The ribs run along the area's local axis 1, the span direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
    #[serde(rename = "type")]
    pub kind: DeckType,
    /// The steel deck's material.
    pub steel: Id,
    /// Concrete depth above the ribs (filled), or the slab's depth (solid).
    #[serde(default)]
    pub slab_depth: f64,
    #[serde(default)]
    pub rib_depth: f64,
    #[serde(default)]
    pub rib_width_top: f64,
    #[serde(default)]
    pub rib_width_bottom: f64,
    /// Rib spacing, centre to centre.
    #[serde(default)]
    pub rib_spacing: f64,
    /// The steel deck's thickness for in-plane shear (an unfilled deck's membrane thickness).
    #[serde(default)]
    pub shear_thickness: f64,
    #[serde(default)]
    pub unit_weight: f64,
}

impl Deck {
    /// Concrete volume per unit area: the slab above the ribs plus the ribs (their mean width over the
    /// spacing); none on an unfilled deck.
    pub fn concrete_per_area(&self) -> f64 {
        match self.kind {
            DeckType::Filled => {
                self.slab_depth
                    + self.rib_depth * 0.5 * (self.rib_width_top + self.rib_width_bottom)
                        / self.rib_spacing
            }
            DeckType::Solid => self.slab_depth,
            DeckType::Unfilled => 0.0,
        }
    }

    /// What is wrong with the deck data, if anything. A deck is always a one-way Membrane.
    fn problem(&self, kind: ShellType, one_way: bool) -> Option<String> {
        if kind != ShellType::Membrane || !one_way {
            return Some("a deck is always a Membrane with one-way load distribution".to_string());
        }
        let lengths = [
            self.slab_depth,
            self.rib_depth,
            self.rib_width_top,
            self.rib_width_bottom,
            self.rib_spacing,
            self.shear_thickness,
            self.unit_weight,
        ];
        if lengths.iter().any(|v| !(v.is_finite() && *v >= 0.0)) {
            return Some("deck dimensions and unit weight must be 0 or more".to_string());
        }
        match self.kind {
            DeckType::Filled if !(self.slab_depth > 0.0 && self.rib_spacing > 0.0) => {
                Some("a filled deck needs a slab depth and a rib spacing > 0".to_string())
            }
            DeckType::Solid if self.slab_depth <= 0.0 => {
                Some("a solid slab deck needs a slab depth > 0".to_string())
            }
            DeckType::Unfilled if self.shear_thickness <= 0.0 => {
                Some("an unfilled deck needs a shear thickness > 0".to_string())
            }
            _ => None,
        }
    }

    /// The deck's total depth: slab depth + rib depth.
    pub fn total_depth(&self) -> f64 {
        match self.kind {
            DeckType::Solid => self.slab_depth,
            _ => self.slab_depth + self.rib_depth,
        }
    }
}

impl AreaSection {
    /// The slab type the analysis uses: a wall or deck section has none (a plain Slab).
    pub fn slab_kind(&self) -> SlabType {
        match self.property_type {
            Some(AreaPropertyType::Wall | AreaPropertyType::Deck) => SlabType::Slab,
            _ => self.slab_type,
        }
    }

    /// The material and thickness the shell element is analysed with. A deck acts as a Membrane of the
    /// concrete above its ribs (filled), of its slab (solid), or of its steel's shear thickness (unfilled).
    pub fn analysed(&self) -> (&Id, f64) {
        match &self.deck {
            Some(d) => match d.kind {
                DeckType::Filled | DeckType::Solid => (&self.material, d.slab_depth),
                DeckType::Unfilled => (&d.steel, d.shear_thickness),
            },
            None => (&self.material, self.thickness),
        }
    }
}

/// Slab types that decide who wins where floor objects overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlabType {
    #[default]
    Slab,
    /// A drop panel: wins over a Slab where they overlap, whatever their sizes.
    Drop,
    /// Stiff slab (rigid zones over columns and walls): bending stiffness 100 times the
    /// section's, membrane and transverse shear unchanged (docs/elements/STIFF_AREA.md).
    Stiff,
}

impl SlabType {
    pub fn is_slab(&self) -> bool {
        *self == SlabType::Slab
    }
}

/// An area object. The solver analyses flat, convex quadrilaterals and triangles;
/// the mesh step turns any flat polygon (and openings) into those. Local axes follow the common rule (see
/// [`crate::axes::area_axes`]); `angle` rotates local 1 and 2 about local 3, in degrees.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub id: Id,
    pub nodes: Vec<Id>,
    /// Area section (none for an opening).
    #[serde(default)]
    pub section: Id,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f64,
    /// An opening: cuts a hole in the floor or wall it lies in.
    #[serde(default, skip_serializing_if = "is_false")]
    pub opening: bool,
    /// Mesh option of this area (mesh step); missing = the model's settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<AreaMesh>,
    /// "Add restraints on edge if corners have restraints" (mesh step): mesh joints on an edge
    /// whose two corners have the same restraints get them too. Missing = on for walls, off for floors
    /// (the defaults).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_restraints: Option<bool>,
    /// A stored mesh (user mesh): the mesh step uses these elements as they are instead of
    /// meshing the area, and divides its neighbours' shared edges at their joints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_mesh: Option<UserMesh>,
    /// Shell property modifiers of this object; times its section's. The mesh step gives them to
    /// the area's elements.
    #[serde(default, skip_serializing_if = "AreaModifiers::is_one")]
    pub modifiers: AreaModifiers,
    /// Rigid diaphragm of this area: all its joints (its mesh joints too) join it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diaphragm: Option<Id>,
    /// Insertion point: where the shell's mid-plane sits relative to the plane of its joints; missing =
    /// the middle. The mesh step gives it to the area's elements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<AreaInsertion>,
    /// Area spring property over this area (one of `area_spring_props`). The mesh step gives it to
    /// the area's elements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area_spring: Option<Id>,
    /// Pier label (one of `pier_labels`); used on vertical walls only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pier: Option<Id>,
    /// Spandrel label (one of `spandrel_labels`); used on vertical walls only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spandrel: Option<Id>,
}

/// Which face of a shell lies on the plane of its joints (its cardinal point).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AreaCardinal {
    /// The +3 face: the mid-plane sits half the thickness along -3.
    Top,
    #[default]
    Middle,
    /// The -3 face: the mid-plane sits half the thickness along +3.
    Bottom,
}

/// Insertion point of an area: its cardinal point and whether the offset changes the stiffness.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct AreaInsertion {
    #[serde(default)]
    pub point: AreaCardinal,
    /// The stiffness follows the offset (rigid arms from the joints to the mid-plane: membrane and
    /// bending couple). Off (the default), the offset moves the drawing only.
    #[serde(default, skip_serializing_if = "is_false")]
    pub transform: bool,
}

/// A mesh stored on an area: joint coordinates and elements (3 or 4 indices into `joints`, in order
/// round the element).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserMesh {
    pub joints: Vec<[f64; 3]>,
    pub elements: Vec<Vec<usize>>,
}

impl Area {
    /// A null area (no section and not an opening, property "None"): no stiffness, no
    /// elements, does not cut the mesh; its loads go to the floor elements under it.
    pub fn is_null(&self) -> bool {
        self.section.is_empty() && !self.opening
    }
}

/// Angle limits for object types (*Preferences - Tolerance*), in degrees: a frame within `column`
/// of vertical is a column, within `beam_floor` of horizontal a beam, else a brace; an area sloping less
/// than `beam_floor` is a floor, else a wall.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AngleLimits {
    #[serde(default = "AngleLimits::default_column")]
    pub column: f64,
    #[serde(default = "AngleLimits::default_beam_floor")]
    pub beam_floor: f64,
}

impl AngleLimits {
    fn default_column() -> f64 {
        1.0
    }
    fn default_beam_floor() -> f64 {
        20.0
    }
}

impl Default for AngleLimits {
    fn default() -> Self {
        AngleLimits {
            column: Self::default_column(),
            beam_floor: Self::default_beam_floor(),
        }
    }
}

/// Auto-mesh settings of a model: `"off"`, or the settings (missing fields = the defaults).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MeshSetting {
    Word(MeshWord),
    Auto(MeshSettings),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeshWord {
    /// Older files: every area is set to no mesh (one element each); frames are still split.
    Off,
}

/// Floor mesh types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FloorMesh {
    /// Equal grids on rectangular regions (cut at beams and walls), a quad-dominant mesh elsewhere.
    #[default]
    General,
    /// Lines along the local axes through key points; cells clipped by the edges.
    Rectangular,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshSettings {
    #[serde(default)]
    pub floor: FloorMesh,
    /// Approximate maximum element size of floors and walls (missing = 1.25 m, in model units).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_size: Option<f64>,
    /// Smaller elements near openings and re-entrant corners, in the irregular regions of a General
    /// floor mesh only (not four-sided regions, which keep
    /// their equal grid). Missing = on.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub refine: bool,
}

impl Default for MeshSettings {
    fn default() -> Self {
        MeshSettings {
            floor: FloorMesh::General,
            floor_size: None,
            wall_size: None,
            refine: true,
        }
    }
}

fn yes() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

/// Mesh option of one area: `"none"` (one element, 3 or 4 corners) or its own size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AreaMesh {
    Word(AreaMeshWord),
    Size { size: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AreaMeshWord {
    None,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// A joint lying on an edge of an area (between two of its corners, not at one), tied to that edge
/// (an edge constraint).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeConstraint {
    pub area: Id,
    /// The two corners of the edge (consecutive corners of the area, either order).
    pub edge: [Id; 2],
    pub node: Id,
}

/// Uniform load on an area: `q` force per area, in global X/Y/Z or local 1/2/3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AreaLoad {
    pub area: Id,
    pub dir: LoadDir,
    pub q: f64,
}

/// Section properties, with the usual names: I33 = bending about local 3 (major for a beam),
/// I22 = bending about local 2, I23 = product of inertia (∫ x2 x3 dA, 0 for a section symmetric about
/// either axis), As2/As3 = shear areas (0 or missing = no shear deformation). A section with a `shape`
/// gets all of these from it in the mesh step (`osab_mesh::section`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Section {
    pub id: Id,
    #[serde(default)]
    pub A: f64,
    #[serde(default)]
    pub I22: f64,
    #[serde(default)]
    pub I33: f64,
    #[serde(default)]
    pub J: f64,
    #[serde(default)]
    pub As2: f64,
    #[serde(default)]
    pub As3: f64,
    /// Section property modifiers; times the frame object's own.
    #[serde(default, skip_serializing_if = "FrameModifiers::is_one")]
    pub modifiers: FrameModifiers,
    /// Overall depth along local 2 (t3) and width along local 3 (t2), for automatic end
    /// offsets; missing = 0 (no automatic offset from this section).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Product of inertia about local 2 and 3: bending couples the two planes (an angle pushed along
    /// one leg also moves along the other); the member bends about its principal axes.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub I23: f64,
    /// Where the centroid lies from the middle of the section's box (depth x width), along local 2 and
    /// 3: places the cardinal points of insertion points. Missing = the middle (symmetric sections).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centroid_offset: Option<[f64; 2]>,
    /// Where the shear centre lies from the middle of the box (cardinal point 11). It changes nothing
    /// else: members bend and twist about the centroid. Missing = the centroid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shear_centre_offset: Option<[f64; 2]>,
    /// Self weight per unit length (force / length), for a section of two materials (each counts with
    /// its own area and weight); missing = the frame's material weight x A.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_weight: Option<f64>,
    /// The shape the section was made from (`osab_mesh::section::Shape`); the mesh step fills in the
    /// numbers above from it. The solver itself reads only the numbers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<serde_json::Value>,
}

fn one() -> f64 {
    1.0
}

fn is_one(v: &f64) -> bool {
    *v == 1.0
}

/// Frame property modifiers (*Assign > Frame > Property Modifiers*, and on frame sections):
/// factors on A, As2, As3, J, I22, I33, mass and weight; missing = 1. A frame uses its section's
/// modifiers times its own (the two multiply).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct FrameModifiers {
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub A: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub As2: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub As3: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub J: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub I22: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub I33: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub mass: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub weight: f64,
}

impl Default for FrameModifiers {
    fn default() -> Self {
        Self::from_array([1.0; 8])
    }
}

impl FrameModifiers {
    /// Names in the usual order (also the order of [`FrameModifiers::as_array`]).
    pub const NAMES: [&'static str; 8] = ["A", "As2", "As3", "J", "I22", "I33", "mass", "weight"];

    pub fn as_array(&self) -> [f64; 8] {
        [
            self.A,
            self.As2,
            self.As3,
            self.J,
            self.I22,
            self.I33,
            self.mass,
            self.weight,
        ]
    }

    pub fn from_array(v: [f64; 8]) -> Self {
        FrameModifiers {
            A: v[0],
            As2: v[1],
            As3: v[2],
            J: v[3],
            I22: v[4],
            I33: v[5],
            mass: v[6],
            weight: v[7],
        }
    }

    pub fn is_one(&self) -> bool {
        self.as_array() == [1.0; 8]
    }

    /// These modifiers times `other`, value by value.
    pub fn times(&self, other: &Self) -> Self {
        let (a, b) = (self.as_array(), other.as_array());
        Self::from_array(std::array::from_fn(|k| a[k] * b[k]))
    }

    /// The first problem with the values, if any: stiffness factors must be > 0, mass and weight >= 0.
    fn problem(&self) -> Option<String> {
        modifier_problem(&Self::NAMES, &self.as_array(), 6)
    }
}

/// Shell property modifiers (*Assign > Shell > Property Modifiers*, and on area sections):
/// factors on the membrane (f11, f22, f12), bending (m11, m22, m12) and transverse shear (v13, v23)
/// stiffness, mass and weight; missing = 1. An area uses its section's modifiers times its own. How they
/// act inside the shell element (docs/phases/PHASE4_PLAN.md, probe findings 1b): each scales its
/// own term of the constitutive matrix, the Poisson coupling gets the square root of the two direct
/// factors (sqrt(f11 f22), sqrt(m11 m22)), and f12 also scales the membrane's drilling stiffness.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AreaModifiers {
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub f11: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub f22: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub f12: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub m11: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub m22: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub m12: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub v13: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub v23: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub mass: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub weight: f64,
}

impl Default for AreaModifiers {
    fn default() -> Self {
        Self::from_array([1.0; 10])
    }
}

impl AreaModifiers {
    /// Names in the usual order (also the order of [`AreaModifiers::as_array`]).
    pub const NAMES: [&'static str; 10] = [
        "f11", "f22", "f12", "m11", "m22", "m12", "v13", "v23", "mass", "weight",
    ];

    /// What the Stiff slab type uses in place of the section's modifiers (docs/elements/STIFF_AREA.md):
    /// bending x 100, no mass, no weight.
    pub const STIFF: AreaModifiers = AreaModifiers {
        f11: 1.0,
        f22: 1.0,
        f12: 1.0,
        m11: 100.0,
        m22: 100.0,
        m12: 100.0,
        v13: 1.0,
        v23: 1.0,
        mass: 0.0,
        weight: 0.0,
    };

    pub fn as_array(&self) -> [f64; 10] {
        [
            self.f11,
            self.f22,
            self.f12,
            self.m11,
            self.m22,
            self.m12,
            self.v13,
            self.v23,
            self.mass,
            self.weight,
        ]
    }

    pub fn from_array(v: [f64; 10]) -> Self {
        let [f11, f22, f12, m11, m22, m12, v13, v23, mass, weight] = v;
        AreaModifiers {
            f11,
            f22,
            f12,
            m11,
            m22,
            m12,
            v13,
            v23,
            mass,
            weight,
        }
    }

    pub fn is_one(&self) -> bool {
        self.as_array() == [1.0; 10]
    }

    /// These modifiers times `other`, value by value.
    pub fn times(&self, other: &Self) -> Self {
        let (a, b) = (self.as_array(), other.as_array());
        Self::from_array(std::array::from_fn(|k| a[k] * b[k]))
    }

    /// The eight stiffness factors f11 f22 f12 m11 m22 m12 v13 v23, as the shell elements take them.
    pub fn stiffness(&self) -> [f64; 8] {
        let a = self.as_array();
        std::array::from_fn(|k| a[k])
    }

    /// The first problem with the values, if any: stiffness factors must be > 0, mass and weight >= 0.
    fn problem(&self) -> Option<String> {
        modifier_problem(&Self::NAMES, &self.as_array(), 8)
    }
}

/// Checks modifier values: the first `stiffness` must be > 0, the rest (mass, weight) >= 0.
fn modifier_problem(names: &[&str], values: &[f64], stiffness: usize) -> Option<String> {
    values
        .iter()
        .zip(names)
        .enumerate()
        .find_map(|(k, (v, n))| {
            let ok = v.is_finite() && if k < stiffness { *v > 0.0 } else { *v >= 0.0 };
            (!ok).then(|| {
                format!(
                    "property modifier {n} must be a number {} 0",
                    if k < stiffness { ">" } else { ">=" }
                )
            })
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Id,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    /// *Include in mesh* (mesh step): whether a joint with nothing attached (e.g. one that only
    /// carries a joint load) becomes a floor mesh joint. Missing = no: its loads go to the
    /// joints of the floor element it lies in. Joints with anything attached are always mesh joints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_mesh: Option<bool>,
    /// Rigid diaphragm of this joint (one of `diaphragms`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diaphragm: Option<Id>,
}

/// A rigid diaphragm: the joints of each story that name it move together in their plane (UX, UY
/// and RZ follow one master joint; UZ, RX and RY stay their own). A semi-rigid one ties nothing (the
/// floor's own in-plane stiffness acts); it names the floor for loads given per diaphragm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diaphragm {
    pub id: Id,
    #[serde(default, skip_serializing_if = "is_false")]
    pub semi_rigid: bool,
}

/// A story level: its name and the height of its floor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    pub id: Id,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameKind {
    Frame,
    Truss,
}

/// A frame object: a frame or truss member between two joints. A frame with no
/// section is a null frame (section "None"): no stiffness, no element; the mesh step uses it as a
/// floor meshing line and passes its loads to the floor along it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub id: Id,
    #[serde(rename = "type")]
    pub kind: FrameKind,
    pub nodes: [Id; 2],
    #[serde(default, skip_serializing_if = "Id::is_empty")]
    pub material: Id,
    #[serde(default, skip_serializing_if = "Id::is_empty")]
    pub section: Id,
    /// Rotation of local axes 2 and 3 about local axis 1, in degrees.
    #[serde(default)]
    pub angle: f64,
    /// *Include in mesh* (floor meshing option, mesh step): missing = yes (*Program
    /// Determined*): floors are meshed along the frame. `false`: they are not, and the frame is tied to
    /// the floor elements where it crosses their edges (edge constraints), e.g. for a stiffener.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_mesh: Option<bool>,
    /// Frame property modifiers of this object; times its section's.
    #[serde(default, skip_serializing_if = "FrameModifiers::is_one")]
    pub modifiers: FrameModifiers,
    /// End releases (*Assign > Frame > Releases*): end forces that are zero at end i and j.
    #[serde(default, skip_serializing_if = "Releases::is_none")]
    pub releases: Releases,
    /// End length offsets (*Assign > Frame > End Length Offsets*): automatic from the frames
    /// meeting at the ends, or given lengths; missing = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offsets: Option<EndOffsets>,
    /// Rigid-zone factor (0 to 1): the part of each end offset that is rigid in bending and
    /// shear. The default is 0 (offsets then only move the output stations and the self-weight).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rigid_zone: f64,
    /// Set by the mesh step on the pieces of a frame it split: the frame object. A frame
    /// object's mass (and the mass of its loads) at the object's own two ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<Id>,
    /// Insertion point: where the section sits relative to the line of the joints; missing = the
    /// centroid on that line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<FrameInsertion>,
    /// Line spring property along this frame (one of `line_spring_props`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_spring: Option<Id>,
    /// Pier label (one of `pier_labels`); used on vertical frames only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pier: Option<Id>,
    /// Spandrel label (one of `spandrel_labels`); used on horizontal frames only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spandrel: Option<Id>,
}

fn centroid_point() -> u8 {
    10
}

fn is_zero3(v: &[f64; 3]) -> bool {
    v.iter().all(|x| *x == 0.0)
}

/// Insertion point of a frame (*cardinal point*, mirrors, joint offsets). The cardinal points lie on the
/// box around the section in its local 2-3 plane: 1 to 9 from bottom left (-2 face, -3 side) to top
/// right (+2 face, +3 side), 10 the centroid, 11 the shear centre (the centroid for the doubly
/// symmetric sections here).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrameInsertion {
    #[serde(default = "centroid_point")]
    pub point: u8,
    /// The section mirrored about its local 2 axis (the local-3 side of the offset flips).
    #[serde(default, skip_serializing_if = "is_false")]
    pub mirror2: bool,
    /// The section mirrored about its local 3 axis (the local-2 side of the offset flips).
    #[serde(default, skip_serializing_if = "is_false")]
    pub mirror3: bool,
    /// Joint offsets at end i and end j: from the joint to the cardinal point.
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset_i: [f64; 3],
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset_j: [f64; 3],
    /// The joint offsets are in global X, Y, Z (missing: local 1, 2, 3).
    #[serde(default, skip_serializing_if = "is_false")]
    pub global: bool,
    /// The stiffness follows the offsets (rigid arms from the joints to the element ends). Off (the
    /// default), the insertion point moves the drawing only.
    #[serde(default, skip_serializing_if = "is_false")]
    pub transform: bool,
}

impl Default for FrameInsertion {
    fn default() -> Self {
        FrameInsertion {
            point: 10,
            mirror2: false,
            mirror3: false,
            offset_i: [0.0; 3],
            offset_j: [0.0; 3],
            global: false,
            transform: false,
        }
    }
}

impl FrameInsertion {
    /// The same insertion for the frame drawn the other way: the ends swap, and a local offset along 1
    /// changes sign.
    pub fn reversed(&self) -> FrameInsertion {
        let flip = |mut o: [f64; 3]| {
            if !self.global {
                o[0] = -o[0];
            }
            o
        };
        FrameInsertion {
            offset_i: flip(self.offset_j),
            offset_j: flip(self.offset_i),
            ..*self
        }
    }

    /// The insertion of the piece of the frame from fraction `t0` to `t1` of its length (mesh step):
    /// joint offsets in between follow a straight line from end i to end j; a local offset along 1
    /// stays at the frame's own ends.
    pub fn piece(&self, t0: f64, t1: f64) -> FrameInsertion {
        let at = |t: f64, end: bool| -> [f64; 3] {
            let mut o: [f64; 3] = std::array::from_fn(|k| {
                self.offset_i[k] + t * (self.offset_j[k] - self.offset_i[k])
            });
            if !self.global && !end {
                o[0] = 0.0;
            }
            o
        };
        FrameInsertion {
            offset_i: at(t0, t0 <= 0.0),
            offset_j: at(t1, t1 >= 1.0),
            ..*self
        }
    }

    /// The centroid's position relative to the cardinal point, in local 2 and 3, for a section box
    /// `depth` (along 2) by `width` (along 3) whose centroid and shear centre lie at `centroid` and
    /// `shear_centre` from the box's middle.
    pub fn centroid_from_point(
        &self,
        depth: f64,
        width: f64,
        centroid: [f64; 2],
        shear_centre: [f64; 2],
    ) -> [f64; 2] {
        let (mut d2, mut d3) = match self.point {
            1..=9 => {
                let (row, col) = ((self.point - 1) / 3, (self.point - 1) % 3);
                // Row 0 = bottom (-2 face), column 0 = left (-3 side): the box's middle lies towards +2, +3.
                (
                    (1.0 - row as f64) * depth / 2.0 + centroid[0],
                    (1.0 - col as f64) * width / 2.0 + centroid[1],
                )
            }
            11 => (centroid[0] - shear_centre[0], centroid[1] - shear_centre[1]),
            _ => (0.0, 0.0),
        };
        if self.mirror3 {
            d2 = -d2;
        }
        if self.mirror2 {
            d3 = -d3;
        }
        [d2, d3]
    }
}

/// End length offsets of a frame: `"auto"` (*Automatic from Connectivity*) or lengths at end i and
/// end j (end order).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EndOffsets {
    Auto(AutoWord),
    Lengths { i: f64, j: f64 },
}

/// The word `"auto"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoWord {
    Auto,
}

/// An end force a frame release sets to zero, by name (local axes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Release {
    P,
    V2,
    V3,
    T,
    M2,
    M3,
}

impl Release {
    /// Index in a frame end's local DOFs (u1 u2 u3 r1 r2 r3).
    pub fn dof(self) -> usize {
        self as usize
    }
}

/// End releases of a frame, at end i and end j (end order).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Releases {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub i: Vec<Release>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub j: Vec<Release>,
}

impl Releases {
    pub fn is_none(&self) -> bool {
        self.i.is_empty() && self.j.is_empty()
    }

    /// Released local DOFs of the 12 (end i, then end j).
    pub fn mask(&self) -> [bool; 12] {
        let mut m = [false; 12];
        for r in &self.i {
            m[r.dof()] = true;
        }
        for r in &self.j {
            m[6 + r.dof()] = true;
        }
        m
    }

    /// The same releases with the ends swapped (for a frame turned to the end order).
    pub fn swapped(&self) -> Releases {
        Releases {
            i: self.j.clone(),
            j: self.i.clone(),
        }
    }

    /// Why these releases are refused (they make the frame a mechanism), if they are: an end force
    /// released at both ends (P, V2, V3, T), or a moment released at both ends with the shear that
    /// balances it released at either end.
    pub fn instability(&self) -> Option<String> {
        let m = self.mask();
        let both = |r: Release| m[r.dof()] && m[6 + r.dof()];
        let either = |r: Release| m[r.dof()] || m[6 + r.dof()];
        for r in [Release::P, Release::V2, Release::V3, Release::T] {
            if both(r) {
                return Some(format!("{r:?} is released at both ends"));
            }
        }
        for (mom, shear) in [(Release::M3, Release::V2), (Release::M2, Release::V3)] {
            if both(mom) && either(shear) {
                return Some(format!(
                    "{mom:?} is released at both ends and {shear:?} at an end"
                ));
            }
        }
        None
    }
}

impl Frame {
    /// A null frame (no section, "None").
    pub fn is_null(&self) -> bool {
        self.section.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Fix {
    /// "fixed" (all six) or "pinned" (ux, uy, uz).
    Preset(String),
    /// List of restrained DOFs, e.g. ["ux", "uz", "ry"].
    List(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Support {
    pub node: Id,
    pub fix: Fix,
}

/// Added mass of one joint in global axes: `ux uy uz` mass (force /
/// (length / s²), e.g. t with kN and m), `rx ry rz` mass moments of inertia (mass · length²).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JointMass {
    pub node: Id,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ux: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub uy: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub uz: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rx: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ry: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rz: f64,
}

impl JointMass {
    pub fn as_array(&self) -> [f64; 6] {
        [self.ux, self.uy, self.uz, self.rx, self.ry, self.rz]
    }
}

/// Mass source. Missing fields are the defaults: element self mass and added joint masses,
/// no load patterns, lateral mass only, lumped at story levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassSource {
    /// The elements' own mass (material weight / g, times the mass modifiers).
    #[serde(default = "yes")]
    pub elements: bool,
    /// The added joint masses (`masses`).
    #[serde(default = "yes")]
    pub joint_masses: bool,
    /// Load cases whose downward loads (and self-weight) count as mass, load / g, times `scale`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loads: Vec<MassLoad>,
    /// Only UX, UY and RZ carry mass (the default); false = all six.
    #[serde(default = "yes")]
    pub lateral_only: bool,
    /// Lateral mass at a joint between two story levels goes to the levels (the default).
    #[serde(default = "yes")]
    pub lump_at_stories: bool,
}

impl Default for MassSource {
    fn default() -> Self {
        MassSource {
            elements: true,
            joint_masses: true,
            loads: Vec::new(),
            lateral_only: true,
            lump_at_stories: true,
        }
    }
}

/// Loads that count as mass: a load pattern's (or, in files without patterns, a load case's) times
/// `scale`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassLoad {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case: Option<Id>,
    #[serde(default = "one")]
    pub scale: f64,
    /// The pattern's loads, written here by [`Model::with_patterns`] (the mesh step then treats them
    /// like a load case's). Not for input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loads: Option<Box<LoadCase>>,
}

impl MassLoad {
    /// The pattern or case named, for messages.
    pub fn name(&self) -> &Id {
        self.pattern
            .as_ref()
            .or(self.case.as_ref())
            .expect("validated mass load")
    }
}

/// P-Delta options: a geometric stiffness added to the stiffness of every case (load cases, the modal
/// case and so spectrum cases, combinations).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PDelta {
    #[serde(default)]
    pub method: PDeltaMethod,
    /// The P-Delta load of the `loads` method: load patterns (in files without patterns, load cases)
    /// times scale factors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loads: Vec<MassLoad>,
}

/// What the analysis does when the structure is unstable (a direction without stiffness: a part not
/// held, a mechanism).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnInstability {
    /// A tiny stiffness is put in each such direction and the analysis goes on; the directions are
    /// listed with the results, which are not reliable at and near them.
    #[default]
    Continue,
    /// The analysis stops with the first such joint and direction.
    Stop,
}

impl OnInstability {
    fn is_default(&self) -> bool {
        *self == OnInstability::Continue
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PDeltaMethod {
    /// No P-Delta.
    #[default]
    None,
    /// Non-iterative, from the mass: every joint with mass pulls sideways by its weight over its height
    /// above the base, as a negative spring to the ground (UX, UY).
    Mass,
    /// Iterative, from loads: the axial forces of the P-Delta load (solved with P-Delta) give the frames
    /// and shells their geometric stiffness.
    Loads,
}

/// A modal case: `modes` Eigen vectors (default 12; fewer when the model has fewer mass DOFs), or with
/// `"type": "ritz"` that many load-dependent Ritz vectors from the starting loads `starts`
/// (docs/phases/RITZ_PLAN.md).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalCase {
    #[serde(default = "modal_id")]
    pub id: Id,
    #[serde(default = "twelve")]
    pub modes: usize,
    #[serde(rename = "type", default, skip_serializing_if = "is_default")]
    pub kind: ModalType,
    /// Ritz starting loads, in order (the order matters when the modes run out mid-cycle).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub starts: Vec<RitzStart>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModalType {
    #[default]
    Eigen,
    Ritz,
}

/// One starting load of a Ritz case: a ground acceleration (its load is the mass times a unit
/// acceleration in that direction) or a load pattern's loads. `cycles` limits the vectors it
/// generates (the first static solve is cycle 1); 0 or missing = no limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RitzStart {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accel: Option<Accel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<Id>,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub cycles: usize,
    /// The pattern's loads, written here by [`Model::with_patterns`] (the mesh step then treats them
    /// like a load case's). Not for input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loads: Option<Box<LoadCase>>,
}

/// A ground acceleration direction of a Ritz starting load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accel {
    UX,
    UY,
    UZ,
    RX,
    RY,
    RZ,
}

impl Accel {
    /// The global direction: 0 to 5 (UX UY UZ RX RY RZ).
    pub fn dir(self) -> usize {
        self as usize
    }
}

impl RitzStart {
    /// The name of this start in results: the direction or the pattern.
    pub fn name(&self) -> String {
        match (&self.accel, &self.pattern) {
            (Some(a), _) => format!("{a:?}"),
            (None, Some(p)) => p.to_string(),
            _ => String::new(),
        }
    }
}

fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}

impl Model {
    /// Group names are unique and every object in a group exists.
    pub fn check_groups(&self) -> Result<(), Error> {
        unique("group", self.groups.iter().map(|g| (&g.id, g)))?;
        use std::collections::HashSet;
        let nodes: HashSet<&Id> = self.nodes.iter().map(|n| &n.id).collect();
        let frames: HashSet<&Id> = self.frames.iter().map(|f| &f.id).collect();
        let links: HashSet<&Id> = self.links.iter().map(|l| &l.id).collect();
        let areas: HashSet<&Id> = self.areas.iter().map(|a| &a.id).collect();
        for g in &self.groups {
            let missing = g
                .nodes
                .iter()
                .find(|id| !nodes.contains(id))
                .or_else(|| g.frames.iter().find(|id| !frames.contains(id)))
                .or_else(|| g.links.iter().find(|id| !links.contains(id)))
                .or_else(|| g.areas.iter().find(|id| !areas.contains(id)));
            if let Some(id) = missing {
                return Err(Error::Invalid(format!(
                    "group {}: unknown object {id}",
                    g.id
                )));
            }
        }
        Ok(())
    }

    /// Pier and spandrel label names are unique and every label given to an object is defined.
    pub fn check_labels(&self) -> Result<(), Error> {
        unique("pier label", self.pier_labels.iter().map(|l| (&l.id, l)))?;
        unique(
            "spandrel label",
            self.spandrel_labels.iter().map(|l| (&l.id, l)),
        )?;
        let objects = self
            .areas
            .iter()
            .map(|a| ("area", &a.id, &a.pier, &a.spandrel))
            .chain(
                self.frames
                    .iter()
                    .map(|f| ("frame", &f.id, &f.pier, &f.spandrel)),
            );
        for (what, id, pier, spandrel) in objects {
            if let Some(p) = pier
                .as_ref()
                .filter(|p| !self.pier_labels.iter().any(|l| &l.id == *p))
            {
                return Err(Error::Invalid(format!(
                    "{what} {id}: unknown pier label {p}"
                )));
            }
            if let Some(s) = spandrel
                .as_ref()
                .filter(|s| !self.spandrel_labels.iter().any(|l| &l.id == *s))
            {
                return Err(Error::Invalid(format!(
                    "{what} {id}: unknown spandrel label {s}"
                )));
            }
        }
        Ok(())
    }

    /// Whether `id` names a response spectrum case.
    pub fn is_spectrum(&self, id: &Id) -> bool {
        self.spectrum_cases.iter().any(|c| &c.id == id)
    }

    /// The model as it runs: the cases set not to run left out, combinations with no load cases in them
    /// left out, and every combination that uses one (or a combination left out); the modal case stays
    /// while a spectrum case runs. With a note saying what was left out.
    pub fn for_run(&self) -> Result<(std::borrow::Cow<'_, Model>, Option<String>), Error> {
        let empty: Vec<String> = self
            .load_combinations
            .iter()
            .filter(|c| c.items.is_empty())
            .map(|c| c.id.to_string())
            .collect();
        if self.not_run.is_empty() && empty.is_empty() {
            return Ok((std::borrow::Cow::Borrowed(self), None));
        }
        let off: std::collections::HashSet<&Id> = self.not_run.iter().collect();
        let mut m = self.clone();
        m.not_run.clear();
        m.load_combinations.retain(|c| !c.items.is_empty());
        m.load_cases.retain(|c| !off.contains(&c.id));
        m.spectrum_cases.retain(|c| !off.contains(&c.id));
        let modal_off = m.modal.as_ref().filter(|mc| off.contains(&mc.id)).cloned();
        if modal_off.is_some() && m.spectrum_cases.is_empty() {
            m.modal = None;
        }
        let mut dropped = Vec::new();
        loop {
            let cases: std::collections::HashSet<Id> = m
                .load_cases
                .iter()
                .map(|c| c.id.clone())
                .chain(m.spectrum_cases.iter().map(|c| c.id.clone()))
                .collect();
            let combos: std::collections::HashSet<Id> =
                m.load_combinations.iter().map(|c| c.id.clone()).collect();
            let before = m.load_combinations.len();
            m.load_combinations.retain(|c| {
                let runs = c.items.iter().all(|it| match (&it.case, &it.combo) {
                    (Some(id), _) => cases.contains(id),
                    (None, Some(id)) => combos.contains(id),
                    (None, None) => true,
                });
                if !runs {
                    dropped.push(c.id.to_string());
                }
                runs
            });
            if m.load_combinations.len() == before {
                break;
            }
        }
        if m.load_cases.is_empty() && m.spectrum_cases.is_empty() && m.modal.is_none() {
            return Err(Error::Invalid(
                "not_run: every case is set not to run".into(),
            ));
        }
        let ids: Vec<String> = self.not_run.iter().map(|id| id.to_string()).collect();
        let mut parts = Vec::new();
        if !ids.is_empty() {
            parts.push(format!("Cases not run (no results): {}", ids.join(", ")));
        }
        if !empty.is_empty() {
            parts.push(format!(
                "Warning: load combinations with no load cases in them left out (no results): {}",
                empty.join(", ")
            ));
        }
        let mut note = parts.join("; ");
        if !dropped.is_empty() {
            note += &format!("; combinations that use them: {}", dropped.join(", "));
        }
        if let Some(mc) = modal_off.filter(|_| m.modal.is_some()) {
            note += &format!(
                "; the modal case {} runs anyway for the response spectrum cases",
                mc.id
            );
        }
        Ok((std::borrow::Cow::Owned(m), Some(note)))
    }
}

fn modal_id() -> Id {
    Id("Modal".into())
}

fn twelve() -> usize {
    12
}

/// A line spring property: stiffness per unit length of a frame along its local 1, 2, 3
/// (force/length²) and about local 1 (force·length/radian per length). Lumped at the joints of each
/// frame element: each end gets the stiffness times half the element's length.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LineSpringProp {
    pub id: Id,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub u1: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub u2: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub u3: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub r1: f64,
}

/// An area spring property: stiffness per unit area along an area's local 1, 2, 3 (force/length³;
/// local 3 is the subgrade modulus of a floor). Lumped at the corners of each shell element: each
/// corner gets the stiffness times its share of the element, the integral of its shape function (as a
/// uniform area load is shared).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AreaSpringProp {
    pub id: Id,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub u1: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub u2: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub u3: f64,
}

/// Spring stiffness of one joint in global axes: `ux uy uz` force/length, `rx ry rz`
/// force·length/radian. Missing values are 0.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Spring {
    pub node: Id,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ux: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub uy: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub uz: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rx: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ry: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rz: f64,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl Spring {
    pub fn as_array(&self) -> [f64; 6] {
        [self.ux, self.uy, self.uz, self.rx, self.ry, self.rz]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    /// The two joints move as one rigid body (all six DOFs).
    Rigid,
}

/// A link between two joints. End i/j follow the same end-order rule as frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: Id,
    #[serde(rename = "type")]
    pub kind: LinkKind,
    pub nodes: [Id; 2],
}

pub const DOF_NAMES: [&str; 6] = ["ux", "uy", "uz", "rx", "ry", "rz"];
pub const FORCE_NAMES: [&str; 6] = ["fx", "fy", "fz", "mx", "my", "mz"];

impl Support {
    pub fn restraints(&self) -> Result<[bool; 6], Error> {
        let mut r = [false; 6];
        match &self.fix {
            Fix::Preset(p) => match p.as_str() {
                "fixed" => r = [true; 6],
                "pinned" => r = [true, true, true, false, false, false],
                other => {
                    return Err(Error::Invalid(format!(
                        "support at node {}: unknown preset \"{other}\" (use \"fixed\", \"pinned\" or a list like [\"ux\",\"uz\"])",
                        self.node
                    )))
                }
            },
            Fix::List(list) => {
                for name in list {
                    let i = DOF_NAMES.iter().position(|d| d == name).ok_or_else(|| {
                        Error::Invalid(format!("support at node {}: unknown DOF \"{name}\"", self.node))
                    })?;
                    r[i] = true;
                }
            }
        }
        Ok(r)
    }
}

/// Load pattern types. The type does not change the analysis; it is kept for automatic combinations,
/// live load reduction and design.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatternType {
    Dead,
    SuperDead,
    Live,
    ReducibleLive,
    RoofLive,
    Snow,
    Seismic,
    Wind,
    Temperature,
    Hydrostatic,
    Construction,
    Notional,
    #[default]
    Other,
}

/// Loads a pattern makes by itself from its settings (automatic lateral loads). Only user wind loads
/// for now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoLoad {
    /// Story forces given per story and diaphragm (`wind_loads`), e.g. from a wind tunnel.
    UserWind,
}

/// Where a user wind load acts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindPoint {
    /// At its diaphragm's centre of mass at that story (from the mass source, in the same run).
    #[default]
    CentreOfMass,
    /// At the row's (x, y).
    Coordinates,
}

/// How a user wind load reaches a semi-rigid diaphragm (on a rigid one it acts through the constraint).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindSemiRigid {
    /// Into the floor element under the point, by its shape functions.
    #[default]
    Floor,
    /// Spread over the diaphragm's joints at that story as a rigid floor's inertia forces.
    Mass,
    /// Tied in plane to the diaphragm's joint object nearest its centre of mass.
    NearestJoint,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// A user wind load: Fx, Fy and Mz on the floor of `diaphragm` at `story`, at its centre of mass or at
/// (`x`, `y`) (`point`); on a semi-rigid diaphragm as `semi_rigid` says. The analysis turns it into joint
/// loads ([`crate::diaphragm`]); no joint is added.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindLoad {
    pub story: Id,
    pub diaphragm: Id,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fx: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fy: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mz: f64,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    /// Set from the pattern's `wind_point` when its loads go into a case.
    #[serde(default, skip_serializing_if = "is_default")]
    pub point: WindPoint,
    /// Set from the pattern's `wind_semi_rigid` when its loads go into a case.
    #[serde(default, skip_serializing_if = "is_default")]
    pub semi_rigid: WindSemiRigid,
}

/// A load pattern: joint, frame and area loads and a self-weight multiplier, under one name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPattern {
    pub id: Id,
    #[serde(rename = "type", default)]
    pub kind: PatternType,
    /// Automatic loads of the pattern (a Wind pattern's `"user-wind"`); missing = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<AutoLoad>,
    /// The loads of `"auto": "user-wind"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wind_loads: Vec<WindLoad>,
    /// Where the user wind loads act (missing = their diaphragm's centre of mass).
    #[serde(default, skip_serializing_if = "is_default")]
    pub wind_point: WindPoint,
    /// How the user wind loads reach a semi-rigid diaphragm (missing = into the floor).
    #[serde(default, skip_serializing_if = "is_default")]
    pub wind_semi_rigid: WindSemiRigid,
    /// Whether the loads may act reversed (kept for automatic combinations; no effect now).
    #[serde(default, skip_serializing_if = "is_false")]
    pub reversible: bool,
    /// Self-weight multiplier (usually 1 for Dead); missing = 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub self_weight: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_loads: Vec<NodeLoad>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frame_loads: Vec<FrameLoad>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub area_loads: Vec<AreaLoad>,
}

impl LoadPattern {
    /// The pattern's loads as a load case of the same name.
    pub fn as_case(&self) -> LoadCase {
        LoadCase {
            id: self.id.clone(),
            node_loads: self.node_loads.clone(),
            frame_loads: self.frame_loads.clone(),
            area_loads: self.area_loads.clone(),
            wind_loads: self
                .wind_loads
                .iter()
                .map(|w| WindLoad {
                    point: self.wind_point,
                    semi_rigid: self.wind_semi_rigid,
                    ..w.clone()
                })
                .collect(),
            self_weight: self.self_weight,
            ..Default::default()
        }
    }
}

/// One part of a load case: a load pattern times `scale`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternLoad {
    pub pattern: Id,
    #[serde(default = "one")]
    pub scale: f64,
}

/// A linear static load case: the load patterns in `loads` times their scale factors, plus loads of its
/// own (files written before load patterns keep their loads on the case).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadCase {
    pub id: Id,
    /// Load patterns and scale factors; [`Model::with_patterns`] writes them into the case's own lists.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loads: Vec<PatternLoad>,
    #[serde(default)]
    pub node_loads: Vec<NodeLoad>,
    /// Loads on frames. Older files call this list `member_loads`.
    #[serde(default, alias = "member_loads")]
    pub frame_loads: Vec<FrameLoad>,
    /// Uniform loads on areas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub area_loads: Vec<AreaLoad>,
    /// User wind loads (from the case's patterns).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wind_loads: Vec<WindLoad>,
    /// Self-weight multiplier of the load case (usually 1 for Dead); missing = 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub self_weight: f64,
    /// Set by the mesh step when the case loads a Membrane floor: the same loads as a mass source counts
    /// them. Their mass is taken at the floor's element corners, like element mass, not after the load
    /// transfer to the beams (probe 8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_loads: Option<Box<LoadCase>>,
}

/// Load combination types in common use (Absolute Add and Range Add are left out).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComboType {
    /// Sum of scaled results.
    LinearAdd,
    /// Largest and smallest of the scaled results.
    Envelope,
    /// Square root of the sum of the squared scaled results, as a maximum (+) and a minimum (-). A part
    /// with a maximum and a minimum counts with the larger of their sizes.
    Srss,
}

/// One part of a combination: a load case or another combination, times `scale`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combo: Option<Id>,
    pub scale: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadCombination {
    pub id: Id,
    #[serde(rename = "type")]
    pub kind: ComboType,
    pub items: Vec<ComboItem>,
}

impl LoadCase {
    /// The same loads times `s`.
    pub fn scaled(&self, s: f64) -> LoadCase {
        let mut c = self.clone();
        c.self_weight *= s;
        for l in &mut c.node_loads {
            for v in [
                &mut l.fx, &mut l.fy, &mut l.fz, &mut l.mx, &mut l.my, &mut l.mz,
            ] {
                *v *= s;
            }
        }
        for l in &mut c.frame_loads {
            match l {
                FrameLoad::Uniform { w, .. } => *w *= s,
                FrameLoad::Trapezoidal { w1, w2, .. } => {
                    *w1 *= s;
                    *w2 *= s;
                }
                FrameLoad::Point { p, .. } => *p *= s,
            }
        }
        for l in &mut c.area_loads {
            l.q *= s;
        }
        for l in &mut c.wind_loads {
            l.fx *= s;
            l.fy *= s;
            l.mz *= s;
        }
        c
    }

    /// Adds `other`'s loads and self-weight multiplier times `s` to this case.
    pub fn add(&mut self, other: &LoadCase, s: f64) {
        let part = other.scaled(s);
        self.node_loads.extend(part.node_loads);
        self.frame_loads.extend(part.frame_loads);
        self.area_loads.extend(part.area_loads);
        self.wind_loads.extend(part.wind_loads);
        self.self_weight += part.self_weight;
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeLoad {
    pub node: Id,
    #[serde(default)]
    pub fx: f64,
    #[serde(default)]
    pub fy: f64,
    #[serde(default)]
    pub fz: f64,
    #[serde(default)]
    pub mx: f64,
    #[serde(default)]
    pub my: f64,
    #[serde(default)]
    pub mz: f64,
}

impl NodeLoad {
    pub fn as_array(&self) -> [f64; 6] {
        [self.fx, self.fy, self.fz, self.mx, self.my, self.mz]
    }
}

/// Load direction: global X/Y/Z or local 1/2/3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadDir {
    X,
    Y,
    Z,
    #[serde(rename = "1")]
    L1,
    #[serde(rename = "2")]
    L2,
    #[serde(rename = "3")]
    L3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FrameLoad {
    /// Uniform load `w` (force per length of frame) from `from` to `to` (distances from end i;
    /// default: whole frame).
    Uniform {
        #[serde(alias = "element")]
        frame: Id,
        dir: LoadDir,
        w: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<f64>,
    },
    /// Load varying linearly from `w1` at `from` to `w2` at `to` (force per length; distances from end
    /// i, default: whole frame), one stretch of a trapezoidal load.
    Trapezoidal {
        #[serde(alias = "element")]
        frame: Id,
        dir: LoadDir,
        w1: f64,
        w2: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<f64>,
    },
    /// Point force `p` at distance `at` from end i.
    Point {
        #[serde(alias = "element")]
        frame: Id,
        dir: LoadDir,
        p: f64,
        at: f64,
    },
}

impl FrameLoad {
    pub fn frame(&self) -> &Id {
        match self {
            FrameLoad::Uniform { frame, .. }
            | FrameLoad::Trapezoidal { frame, .. }
            | FrameLoad::Point { frame, .. } => frame,
        }
    }

    /// The part of a distributed load on the stretch `s..e` of its frame (length `len`), as a load on
    /// the frame piece `piece` that starts at `s`. None for point loads and where less than `tol` is
    /// left.
    pub fn cut(&self, piece: &Id, len: f64, s: f64, e: f64, tol: f64) -> Option<FrameLoad> {
        let (from, to) = match self {
            FrameLoad::Uniform { from, to, .. } | FrameLoad::Trapezoidal { from, to, .. } => {
                (from.unwrap_or(0.0), to.unwrap_or(len))
            }
            FrameLoad::Point { .. } => return None,
        };
        let (a, b) = (from.max(s), to.min(e));
        if b - a <= tol {
            return None;
        }
        let full = a - s <= tol && e - b <= tol;
        let (from_p, to_p) = ((!full).then_some(a - s), (!full).then_some(b - s));
        Some(match self {
            FrameLoad::Uniform { dir, w, .. } => FrameLoad::Uniform {
                frame: piece.clone(),
                dir: *dir,
                w: *w,
                from: from_p,
                to: to_p,
            },
            FrameLoad::Trapezoidal { dir, w1, w2, .. } => {
                let at = |x: f64| w1 + (w2 - w1) * (x - from) / (to - from);
                FrameLoad::Trapezoidal {
                    frame: piece.clone(),
                    dir: *dir,
                    w1: at(a),
                    w2: at(b),
                    from: from_p,
                    to: to_p,
                }
            }
            FrameLoad::Point { .. } => unreachable!(),
        })
    }
}

impl Model {
    pub fn from_json(text: &str) -> Result<Model, Error> {
        let model: Model = serde_json::from_str(text)?;
        model.validate()?;
        Ok(model)
    }

    /// The model with its load patterns written into the load cases: each case gets its own loads plus
    /// every pattern in its `loads` times the scale (self-weight multipliers add the same way), and the
    /// mass source's patterns carry their loads (`MassLoad::loads`). The result has no patterns, so a
    /// second call changes nothing; a model without patterns comes back as it is.
    pub fn with_patterns(&self) -> Result<std::borrow::Cow<'_, Model>, Error> {
        let mass_patterns = self
            .mass_source
            .iter()
            .flat_map(|s| &s.loads)
            .chain(self.p_delta.iter().flat_map(|p| &p.loads))
            .any(|l| l.loads.is_none() && l.pattern.is_some())
            || self
                .modal
                .iter()
                .flat_map(|mc| &mc.starts)
                .any(|s| s.loads.is_none() && s.pattern.is_some());
        if self.load_patterns.is_empty()
            && !mass_patterns
            && self.load_cases.iter().all(|c| c.loads.is_empty())
        {
            return Ok(std::borrow::Cow::Borrowed(self));
        }
        let patterns = unique(
            "load pattern",
            self.load_patterns.iter().map(|p| (&p.id, p)),
        )?;
        let pattern = |id: &Id, what: &str| {
            patterns
                .get(id)
                .map(|p| p.as_case())
                .ok_or_else(|| Error::Invalid(format!("{what}: unknown load pattern {id}")))
        };
        let mut m = self.clone();
        for c in &mut m.load_cases {
            for l in std::mem::take(&mut c.loads) {
                c.add(
                    &pattern(&l.pattern, &format!("load case {}", c.id))?,
                    l.scale,
                );
            }
        }
        if let Some(src) = &mut m.mass_source {
            for l in &mut src.loads {
                if let (None, Some(id)) = (&l.loads, &l.pattern) {
                    l.loads = Some(Box::new(pattern(id, "mass source")?));
                }
            }
        }
        if let Some(pd) = &mut m.p_delta {
            for l in &mut pd.loads {
                if let (None, Some(id)) = (&l.loads, &l.pattern) {
                    l.loads = Some(Box::new(pattern(id, "P-Delta")?));
                }
            }
        }
        if let Some(mc) = &mut m.modal {
            let what = format!("modal case {}", mc.id);
            for st in &mut mc.starts {
                if let (None, Some(id)) = (&st.loads, &st.pattern) {
                    st.loads = Some(Box::new(pattern(id, &what)?));
                }
            }
        }
        m.load_patterns.clear();
        Ok(std::borrow::Cow::Owned(m))
    }

    /// Every set of loads the analysis applies or weighs: the load cases, then the mass source's and the
    /// P-Delta load's pattern loads, then the Ritz starting patterns' loads (after
    /// [`Model::with_patterns`]).
    pub fn load_sets(&self) -> impl Iterator<Item = &LoadCase> {
        self.load_cases
            .iter()
            .chain(
                self.mass_source
                    .iter()
                    .flat_map(|s| &s.loads)
                    .chain(self.p_delta.iter().flat_map(|p| &p.loads))
                    .filter_map(|l| l.loads.as_deref()),
            )
            .chain(
                self.modal
                    .iter()
                    .flat_map(|mc| &mc.starts)
                    .filter_map(|s| s.loads.as_deref()),
            )
    }

    /// [`Model::load_sets`], to change them.
    pub fn load_sets_mut(&mut self) -> impl Iterator<Item = &mut LoadCase> {
        self.load_cases
            .iter_mut()
            .chain(
                self.mass_source
                    .iter_mut()
                    .flat_map(|s| &mut s.loads)
                    .chain(self.p_delta.iter_mut().flat_map(|p| &mut p.loads))
                    .filter_map(|l| l.loads.as_deref_mut()),
            )
            .chain(
                self.modal
                    .iter_mut()
                    .flat_map(|mc| &mut mc.starts)
                    .filter_map(|s| s.loads.as_deref_mut()),
            )
    }

    /// The P-Delta load of the `loads` method as one load case: its patterns (or cases) times their
    /// factors (after [`Model::with_patterns`]); None for the other methods.
    pub fn p_delta_load(&self) -> Option<LoadCase> {
        let pd = self.p_delta.as_ref()?;
        if pd.method != PDeltaMethod::Loads {
            return None;
        }
        let mut c = LoadCase {
            id: Id("~P-Delta".into()),
            ..Default::default()
        };
        for l in &pd.loads {
            match (&l.loads, &l.case) {
                (Some(own), _) => c.add(own, l.scale),
                (None, Some(id)) => {
                    let lc = self.load_cases.iter().find(|x| &x.id == id)?;
                    c.add(lc, l.scale);
                }
                (None, None) => {}
            }
        }
        Some(c)
    }

    /// The object-type angle limits (the defaults when the file has none).
    pub fn angle_limits(&self) -> AngleLimits {
        self.angle_limits.unwrap_or_default()
    }

    /// Areas sloping this many degrees or more are walls (20 by default).
    pub fn wall_slope(&self) -> f64 {
        self.angle_limits().beam_floor
    }

    /// Checks references, sizes and property values. Returns the first problem found.
    /// A file from a newer OSAB (a format version above `MODEL_FORMAT`) is refused: its fields may mean
    /// something this version does not know.
    pub fn check_format(&self) -> Result<(), Error> {
        match self.version {
            Some(v) if v > MODEL_FORMAT => Err(Error::Invalid(format!(
                "this file was made by a newer OSAB (file format {v}); this version reads file format {MODEL_FORMAT} and older: update OSAB"
            ))),
            _ => Ok(()),
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        self.check_format()?;
        let bad = |msg: String| Err(Error::Invalid(msg));
        let lim = self.angle_limits();
        if !(lim.column > 0.0 && lim.beam_floor > 0.0 && lim.column + lim.beam_floor <= 90.0) {
            return bad(
                "angle_limits: column and beam_floor must be > 0 degrees and add up to 90 or less"
                    .into(),
            );
        }
        let nodes = unique("node", self.nodes.iter().map(|n| (&n.id, n)))?;
        let frames = unique("frame", self.frames.iter().map(|e| (&e.id, e)))?;
        let materials = unique("material", self.materials.iter().map(|m| (&m.id, m)))?;
        let sections = unique("section", self.sections.iter().map(|s| (&s.id, s)))?;
        let cases = unique("load case", self.load_cases.iter().map(|c| (&c.id, c)))?;
        let patterns = unique(
            "load pattern",
            self.load_patterns.iter().map(|p| (&p.id, p)),
        )?;
        for c in &self.load_cases {
            for l in &c.loads {
                if !patterns.contains_key(&l.pattern) {
                    return bad(format!(
                        "load case {}: unknown load pattern {}",
                        c.id, l.pattern
                    ));
                }
                if !l.scale.is_finite() {
                    return bad(format!(
                        "load case {}: the scale of {} must be a number",
                        c.id, l.pattern
                    ));
                }
            }
        }
        for p in &self.load_patterns {
            if !p.wind_loads.is_empty()
                && (p.kind != PatternType::Wind || p.auto != Some(AutoLoad::UserWind))
            {
                return bad(format!(
                    "load pattern {}: user wind loads need a Wind pattern with \"auto\": \"user-wind\"",
                    p.id
                ));
            }
            if !p.self_weight.is_finite() {
                return bad(format!(
                    "load pattern {}: the self-weight multiplier must be a number",
                    p.id
                ));
            }
        }
        let combos = unique(
            "load combination",
            self.load_combinations.iter().map(|c| (&c.id, c)),
        )?;
        for c in &self.load_combinations {
            if cases.contains_key(&c.id) {
                return bad(format!(
                    "load combination {}: a load case has the same name",
                    c.id
                ));
            }
            for it in &c.items {
                match (&it.case, &it.combo) {
                    (Some(id), None) if !cases.contains_key(id) && !self.is_spectrum(id) => {
                        return bad(format!("load combination {}: unknown load case {id}", c.id))
                    }
                    (None, Some(id)) if !combos.contains_key(id) => {
                        return bad(format!(
                            "load combination {}: unknown load combination {id}",
                            c.id
                        ))
                    }
                    (Some(_), None) | (None, Some(_)) => {}
                    _ => {
                        return bad(format!(
                            "load combination {}: each item names one case or one combo",
                            c.id
                        ))
                    }
                }
                if !it.scale.is_finite() {
                    return bad(format!(
                        "load combination {}: scale factors must be numbers",
                        c.id
                    ));
                }
            }
        }
        // No combination may contain itself (directly or through others).
        fn visit<'a>(
            id: &'a Id,
            combos: &HashMap<&'a Id, &'a LoadCombination>,
            path: &mut Vec<&'a Id>,
            done: &mut Vec<&'a Id>,
        ) -> Result<(), Error> {
            if done.contains(&id) {
                return Ok(());
            }
            if path.contains(&id) {
                return Err(Error::Invalid(format!(
                    "load combination {id} contains itself (through {})",
                    path.iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(" > ")
                )));
            }
            path.push(id);
            for it in &combos[id].items {
                if let Some(child) = &it.combo {
                    visit(child, combos, path, done)?;
                }
            }
            path.pop();
            done.push(id);
            Ok(())
        }
        let mut done = Vec::new();
        for c in &self.load_combinations {
            visit(&c.id, &combos, &mut Vec::new(), &mut done)?;
        }

        for n in &self.nodes {
            if !(n.x.is_finite() && n.y.is_finite() && n.z.is_finite()) {
                return bad(format!("node {}: coordinates must be numbers", n.id));
            }
        }
        if self.dimension == Dimension::TwoD {
            // The plane frame lies in the X-Z plane: every node needs the same Y.
            let y0 = self.nodes.first().map_or(0.0, |n| n.y);
            let size = self
                .nodes
                .iter()
                .fold(1.0f64, |m, n| m.max(n.x.abs()).max(n.z.abs()));
            if let Some(n) = self.nodes.iter().find(|n| (n.y - y0).abs() > 1e-9 * size) {
                return bad(format!(
                    "node {}: a 2D model must lie in the X-Z plane (all nodes need the same y; found {} and {})",
                    n.id, y0, n.y
                ));
            }
        }
        if self.dimension == Dimension::Grid {
            // The plane grid lies in an X-Y plane: every node needs the same Z.
            let z0 = self.nodes.first().map_or(0.0, |n| n.z);
            let size = self
                .nodes
                .iter()
                .fold(1.0f64, |m, n| m.max(n.x.abs()).max(n.y.abs()));
            if let Some(n) = self.nodes.iter().find(|n| (n.z - z0).abs() > 1e-9 * size) {
                return bad(format!(
                    "node {}: a plane grid model must lie in one X-Y plane (all nodes need the same z; found {} and {})",
                    n.id, z0, n.z
                ));
            }
        }

        for m in &self.materials {
            if !(m.e > 0.0) {
                return bad(format!("material {}: E must be > 0", m.id));
            }
            if !(m.shear_modulus() > 0.0) {
                return bad(format!("material {}: give G or nu (G must be > 0)", m.id));
            }
            if !(m.weight.is_finite() && m.weight >= 0.0) {
                return bad(format!("material {}: weight must be a number >= 0", m.id));
            }
        }
        for s in &self.sections {
            if !(s.A > 0.0) {
                return bad(format!("section {}: A must be > 0", s.id));
            }
            if s.As2 < 0.0 || s.As3 < 0.0 {
                return bad(format!("section {}: shear areas cannot be negative", s.id));
            }
            if !s.I23.is_finite() || (s.I23 != 0.0 && !(s.I23 * s.I23 < s.I22 * s.I33)) {
                return bad(format!(
                    "section {}: I23 squared must be less than I22 x I33",
                    s.id
                ));
            }
            if s.self_weight.is_some_and(|w| !(w.is_finite() && w >= 0.0)) {
                return bad(format!(
                    "section {}: self_weight must be a number >= 0",
                    s.id
                ));
            }
            if let Some(p) = s.modifiers.problem() {
                return bad(format!("section {}: {p}", s.id));
            }
        }
        for e in &self.frames {
            let (Some(ni), Some(nj)) = (nodes.get(&e.nodes[0]), nodes.get(&e.nodes[1])) else {
                return bad(format!("frame {}: unknown node", e.id));
            };
            if !materials.contains_key(&e.material) {
                return bad(format!("frame {}: unknown material {}", e.id, e.material));
            }
            if !e.angle.is_finite() {
                return bad(format!("frame {}: angle must be a number", e.id));
            }
            if let Some(p) = &e.line_spring {
                if !self.line_spring_props.iter().any(|x| &x.id == p) {
                    return bad(format!("frame {}: unknown line spring property {p}", e.id));
                }
            }
            if let Some(ins) = &e.insertion {
                if !(1..=11).contains(&ins.point) {
                    return bad(format!("frame {}: insertion point must be 1 to 11", e.id));
                }
                if !ins
                    .offset_i
                    .iter()
                    .chain(&ins.offset_j)
                    .all(|v| v.is_finite())
                {
                    return bad(format!("frame {}: joint offsets must be numbers", e.id));
                }
            }
            let Some(sec) = sections.get(&e.section) else {
                return bad(format!("frame {}: unknown section {}", e.id, e.section));
            };
            if e.kind == FrameKind::Frame && !(sec.I22 > 0.0 && sec.I33 > 0.0 && sec.J > 0.0) {
                return bad(format!(
                    "frame {}: frame sections need I22, I33 and J > 0",
                    e.id
                ));
            }
            if length(ni, nj) < 1e-9 {
                return bad(format!("frame {}: zero length", e.id));
            }
            // A member with a product of inertia bends about its principal axes: an end may release
            // both shears or neither, both moments or neither (the same in any axes).
            if e.kind == FrameKind::Frame && sec.I23 != 0.0 {
                for end in [&e.releases.i, &e.releases.j] {
                    let has = |r: Release| end.contains(&r);
                    if has(Release::V2) != has(Release::V3) || has(Release::M2) != has(Release::M3)
                    {
                        return bad(format!(
                            "frame {}: section {} has a product of inertia (I23): release both V2 and V3 or neither, both M2 and M3 or neither",
                            e.id, e.section
                        ));
                    }
                }
            }
            if let Some(p) = e.modifiers.problem() {
                return bad(format!("frame {}: {p}", e.id));
            }
            if !(0.0..=1.0).contains(&e.rigid_zone) {
                return bad(format!(
                    "frame {}: the rigid-zone factor must be between 0 and 1",
                    e.id
                ));
            }
            if let Some(EndOffsets::Lengths { i, j }) = e.end_offsets {
                if !(i.is_finite() && j.is_finite() && i >= 0.0 && j >= 0.0) {
                    return bad(format!("frame {}: end offsets must be numbers >= 0", e.id));
                }
                if i + j > length(ni, nj) * (1.0 + 1e-12) {
                    return bad(format!(
                        "frame {}: end offsets {i} + {j} leave no clear length",
                        e.id
                    ));
                }
            }
            if !e.releases.is_none() {
                if e.kind == FrameKind::Truss {
                    return bad(format!(
                        "frame {}: a truss is already pinned; releases are for frames",
                        e.id
                    ));
                }
                if let Some(why) = e.releases.instability() {
                    return bad(format!("frame {}: unstable end releases ({why})", e.id));
                }
            }
        }
        for s in &self.supports {
            if !nodes.contains_key(&s.node) {
                return bad(format!("support: unknown node {}", s.node));
            }
            s.restraints()?;
        }
        let mut sprung = std::collections::HashSet::new();
        for s in &self.springs {
            if !nodes.contains_key(&s.node) {
                return bad(format!("spring: unknown node {}", s.node));
            }
            if !sprung.insert(&s.node) {
                return bad(format!(
                    "node {} has more than one spring entry (give all six values in one)",
                    s.node
                ));
            }
            if !s.as_array().iter().all(|k| k.is_finite() && *k >= 0.0) {
                return bad(format!(
                    "spring at node {}: stiffness values must be numbers >= 0",
                    s.node
                ));
            }
        }
        for (what, props) in [
            (
                "line spring",
                self.line_spring_props
                    .iter()
                    .map(|p| (&p.id, [p.u1, p.u2, p.u3, p.r1]))
                    .collect::<Vec<_>>(),
            ),
            (
                "area spring",
                self.area_spring_props
                    .iter()
                    .map(|p| (&p.id, [p.u1, p.u2, p.u3, 0.0]))
                    .collect(),
            ),
        ] {
            let mut seen = std::collections::HashSet::new();
            for (id, k) in props {
                if !seen.insert(id) {
                    return bad(format!("{what} property {id} is given twice"));
                }
                if !k.iter().all(|v| v.is_finite() && *v >= 0.0) {
                    return bad(format!(
                        "{what} property {id}: stiffness values must be numbers >= 0"
                    ));
                }
            }
        }
        for a in &self.areas {
            if let Some(p) = &a.area_spring {
                if !self.area_spring_props.iter().any(|x| &x.id == p) {
                    return bad(format!("area {}: unknown area spring property {p}", a.id));
                }
            }
        }
        unique("link", self.links.iter().map(|l| (&l.id, l)))?;
        for l in &self.links {
            if frames.contains_key(&l.id) {
                return bad(format!("link {}: a frame already has this id", l.id));
            }
            let (Some(ni), Some(nj)) = (nodes.get(&l.nodes[0]), nodes.get(&l.nodes[1])) else {
                return bad(format!("link {}: unknown node", l.id));
            };
            if length(ni, nj) < 1e-9 {
                return bad(format!(
                    "link {}: both ends are at the same point (use one joint instead)",
                    l.id
                ));
            }
        }
        let area_sections = unique(
            "area section",
            self.area_sections.iter().map(|s| (&s.id, s)),
        )?;
        for s in &self.area_sections {
            if !materials.contains_key(&s.material) {
                return bad(format!(
                    "area section {}: unknown material {}",
                    s.id, s.material
                ));
            }
            let is_deck = s.property_type == Some(AreaPropertyType::Deck);
            if is_deck != s.deck.is_some() {
                return bad(format!(
                    "area section {}: a deck section needs both the Deck property type and its deck data",
                    s.id
                ));
            }
            if let Some(d) = &s.deck {
                if let Some(p) = d.problem(s.kind, s.one_way) {
                    return bad(format!("area section {}: {p}", s.id));
                }
                if !materials.contains_key(&d.steel) {
                    return bad(format!(
                        "area section {}: unknown deck material {}",
                        s.id, d.steel
                    ));
                }
            } else if !(s.thickness > 0.0 && s.thickness.is_finite()) {
                return bad(format!("area section {}: thickness must be > 0", s.id));
            }
            let mat = materials[s.analysed().0];
            let nu = mat.poisson();
            if !(nu > -1.0 && nu < 0.5) {
                return bad(format!(
                    "area section {}: material {} needs a Poisson's ratio between -1 and 0.5 (give nu)",
                    s.id, mat.id
                ));
            }
            if let Some(p) = s.modifiers.problem() {
                return bad(format!("area section {}: {p}", s.id));
            }
            if s.one_way && s.kind != ShellType::Membrane {
                return bad(format!(
                    "area section {}: one-way load distribution needs the Membrane type",
                    s.id
                ));
            }
        }
        let areas = unique("area", self.areas.iter().map(|a| (&a.id, a)))?;
        for a in &self.areas {
            if frames.contains_key(&a.id) || self.links.iter().any(|l| l.id == a.id) {
                return bad(format!(
                    "area {}: a frame or link already has this id",
                    a.id
                ));
            }
            if a.opening {
                return bad(format!(
                    "area {}: openings are cut out by the mesh step (turn meshing on)",
                    a.id
                ));
            }
            if a.nodes.len() != 4 && a.nodes.len() != 3 {
                return bad(format!(
                    "area {}: give 3 or 4 corner joints (or turn meshing on for more corners)",
                    a.id
                ));
            }
            let mut pts = Vec::with_capacity(4);
            for (k, id) in a.nodes.iter().enumerate() {
                let Some(n) = nodes.get(id) else {
                    return bad(format!("area {}: unknown node {}", a.id, id));
                };
                if a.nodes[..k].contains(id) {
                    return bad(format!("area {}: joint {} is used twice", a.id, id));
                }
                pts.push(coords(n));
            }
            if !area_sections.contains_key(&a.section) {
                return bad(format!("area {}: unknown area section {}", a.id, a.section));
            }
            if !a.angle.is_finite() {
                return bad(format!("area {}: angle must be a number", a.id));
            }
            if let Some(p) = a.modifiers.problem() {
                return bad(format!("area {}: {p}", a.id));
            }
            if let Some(problem) = crate::area::shape_problem(&pts) {
                return bad(format!("area {}: {problem}", a.id));
            }
        }
        for ec in &self.edge_constraints {
            let Some(a) = areas.get(&ec.area) else {
                return bad(format!("edge constraint: unknown area {}", ec.area));
            };
            let n = a.nodes.len();
            let consecutive = (0..n).any(|k| {
                let (p, q) = (&a.nodes[k], &a.nodes[(k + 1) % n]);
                (p == &ec.edge[0] && q == &ec.edge[1]) || (p == &ec.edge[1] && q == &ec.edge[0])
            });
            if !consecutive {
                return bad(format!(
                    "edge constraint on area {}: joints {} and {} are not the ends of one of its edges",
                    ec.area, ec.edge[0], ec.edge[1]
                ));
            }
            let Some(m) = nodes.get(&ec.node) else {
                return bad(format!(
                    "edge constraint on area {}: unknown node {}",
                    ec.area, ec.node
                ));
            };
            let (pa, pb, pm) = (
                coords(nodes[&ec.edge[0]]),
                coords(nodes[&ec.edge[1]]),
                coords(m),
            );
            let d = crate::axes::sub(pb, pa);
            let len2 = crate::axes::dot(d, d);
            let f = crate::axes::dot(crate::axes::sub(pm, pa), d) / len2;
            let off = crate::axes::sub(pm, crate::axes::add(pa, crate::axes::scale(d, f)));
            if !(f > 1e-9 && f < 1.0 - 1e-9)
                || crate::axes::dot(off, off).sqrt() > self.units.edge_tie()
            {
                return bad(format!(
                    "edge constraint on area {}: joint {} does not lie inside the edge {}-{}",
                    ec.area, ec.node, ec.edge[0], ec.edge[1]
                ));
            }
        }
        crate::rigid::Bodies::new(self)?;
        for m in &self.masses {
            if !nodes.contains_key(&m.node) {
                return bad(format!("masses: unknown node {}", m.node));
            }
            if !m.as_array().iter().all(|v| v.is_finite() && *v >= 0.0) {
                return bad(format!(
                    "masses at joint {}: values must be numbers of 0 or more",
                    m.node
                ));
            }
        }
        if let Some(src) = &self.mass_source {
            for l in &src.loads {
                match (&l.pattern, &l.case) {
                    (Some(p), None) if l.loads.is_none() && !patterns.contains_key(p) => {
                        return bad(format!("mass source: unknown load pattern {p}"));
                    }
                    (None, Some(c)) if !cases.contains_key(c) => {
                        return bad(format!("mass source: unknown load case {c}"));
                    }
                    (Some(_), None) | (None, Some(_)) => {}
                    _ => {
                        return bad(
                            "mass source: each item names one load pattern (or one load case)"
                                .into(),
                        )
                    }
                }
                if !l.scale.is_finite() {
                    return bad(format!(
                        "mass source: the scale of {} must be a number",
                        l.name()
                    ));
                }
            }
        }
        if let Some(pd) = &self.p_delta {
            if pd.method == PDeltaMethod::Loads && pd.loads.is_empty() {
                return bad("P-Delta from loads: at least one load pattern (or load case)".into());
            }
            for l in &pd.loads {
                match (&l.pattern, &l.case) {
                    (Some(p), None) if l.loads.is_none() && !patterns.contains_key(p) => {
                        return bad(format!("P-Delta: unknown load pattern {p}"));
                    }
                    (None, Some(c)) if !cases.contains_key(c) => {
                        return bad(format!("P-Delta: unknown load case {c}"));
                    }
                    (Some(_), None) | (None, Some(_)) => {}
                    _ => {
                        return bad(
                            "P-Delta: each load names one load pattern (or one load case)".into(),
                        )
                    }
                }
                if !l.scale.is_finite() {
                    return bad(format!(
                        "P-Delta: the scale of {} must be a number",
                        l.name()
                    ));
                }
            }
        }
        if let Some(mc) = &self.modal {
            if mc.modes == 0 {
                return bad(format!("modal case {}: at least one mode", mc.id));
            }
            if cases.contains_key(&mc.id) || combos.contains_key(&mc.id) {
                return bad(format!(
                    "modal case {}: a load case or combination has the same name",
                    mc.id
                ));
            }
            match mc.kind {
                ModalType::Eigen if !mc.starts.is_empty() => {
                    return bad(format!(
                        "modal case {}: starting loads are for Ritz vectors (\"type\": \"ritz\")",
                        mc.id
                    ));
                }
                ModalType::Ritz if mc.starts.is_empty() => {
                    return bad(format!(
                        "modal case {}: Ritz vectors need at least one starting load",
                        mc.id
                    ));
                }
                _ => {}
            }
            for st in &mc.starts {
                match (&st.accel, &st.pattern) {
                    (Some(_), None) => {}
                    (None, Some(p)) if st.loads.is_some() || patterns.contains_key(p) => {}
                    (None, Some(p)) => {
                        return bad(format!("modal case {}: unknown load pattern {p}", mc.id));
                    }
                    _ => {
                        return bad(format!(
                            "modal case {}: each starting load is one acceleration (UX, UY, UZ, RX, RY, RZ) or one load pattern",
                            mc.id
                        ));
                    }
                }
            }
        }
        crate::spectrum::validate(
            &self.spectrum_functions,
            &self.spectrum_cases,
            self.modal.is_some(),
            &|id| {
                cases.contains_key(id)
                    || combos.contains_key(id)
                    || self.modal.as_ref().is_some_and(|m| &m.id == id)
            },
        )?;
        for id in &self.not_run {
            if !(cases.contains_key(id)
                || self.is_spectrum(id)
                || self.modal.as_ref().is_some_and(|m| &m.id == id))
            {
                return bad(format!("not_run: unknown case {id}"));
            }
        }
        unique("diaphragm", self.diaphragms.iter().map(|d| (&d.id, d)))?;
        unique("story", self.stories.iter().map(|s| (&s.id, s)))?;
        self.check_groups()?;
        self.check_labels()?;
        if self.stories.windows(2).any(|w| w[1].z <= w[0].z) {
            return bad(
                "stories: levels must be listed from the lowest up, each higher than the last"
                    .into(),
            );
        }
        crate::diaphragm::levels(self)?;
        let sets: Vec<(String, std::borrow::Cow<LoadCase>)> = self
            .load_cases
            .iter()
            .map(|c| (format!("load case {}", c.id), std::borrow::Cow::Borrowed(c)))
            .chain(self.load_patterns.iter().map(|p| {
                (
                    format!("load pattern {}", p.id),
                    std::borrow::Cow::Owned(p.as_case()),
                )
            }))
            .chain(
                self.mass_source
                    .iter()
                    .flat_map(|s| &s.loads)
                    .chain(self.p_delta.iter().flat_map(|p| &p.loads))
                    .filter_map(|l| l.loads.as_deref())
                    .map(|c| {
                        (
                            format!("load pattern {}", c.id),
                            std::borrow::Cow::Borrowed(c),
                        )
                    }),
            )
            .collect();
        for (what, c) in &sets {
            for l in &c.wind_loads {
                if !self.stories.iter().any(|s| s.id == l.story) {
                    return bad(format!(
                        "{what}: user wind load at unknown story {} (user wind loads need the model's stories)",
                        l.story
                    ));
                }
                if !self.diaphragms.iter().any(|d| d.id == l.diaphragm) {
                    return bad(format!(
                        "{what}: user wind load on unknown diaphragm {}",
                        l.diaphragm
                    ));
                }
                if ![l.fx, l.fy, l.mz, l.x, l.y].iter().all(|v| v.is_finite()) {
                    return bad(format!(
                        "{what}: user wind load at {} {} must be numbers",
                        l.story, l.diaphragm
                    ));
                }
            }
            for l in &c.area_loads {
                if !areas.contains_key(&l.area) {
                    return bad(format!("{what}: unknown area {}", l.area));
                }
                if !l.q.is_finite() {
                    return bad(format!("{what}: load on area {} must be a number", l.area));
                }
            }
            for l in &c.node_loads {
                if !nodes.contains_key(&l.node) {
                    return bad(format!("{what}: unknown node {}", l.node));
                }
                if !l.as_array().iter().all(|v| v.is_finite()) {
                    return bad(format!("{what}: load on node {} must be numbers", l.node));
                }
            }
            for l in &c.frame_loads {
                let Some(e) = frames.get(l.frame()) else {
                    return bad(format!("{what}: unknown frame {}", l.frame()));
                };
                if e.kind == FrameKind::Truss {
                    return bad(format!(
                        "{what}: frame loads on truss {} are not supported yet",
                        e.id
                    ));
                }
                let len = length(nodes[&e.nodes[0]], nodes[&e.nodes[1]]);
                let value = match l {
                    FrameLoad::Uniform { w, .. } => *w,
                    FrameLoad::Trapezoidal { w1, w2, .. } => w1 + w2,
                    FrameLoad::Point { p, .. } => *p,
                };
                if !value.is_finite() {
                    return bad(format!("{what}: load on frame {} must be a number", e.id));
                }
                let tol = 1e-9 * len;
                let ok = |x: f64| x >= -tol && x <= len + tol;
                match l {
                    FrameLoad::Uniform { from, to, .. }
                    | FrameLoad::Trapezoidal { from, to, .. } => {
                        let (a, b) = (from.unwrap_or(0.0), to.unwrap_or(len));
                        if !(ok(a) && ok(b) && a < b) {
                            return bad(format!(
                                "{what}: distributed load on frame {} needs 0 <= from < to <= length ({len})", e.id
                            ));
                        }
                    }
                    FrameLoad::Point { at, .. } => {
                        if !ok(*at) {
                            return bad(format!(
                                "{what}: point load on frame {} must be within 0..{len}",
                                e.id
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn node(&self, id: &Id) -> Option<&Node> {
        self.nodes.iter().find(|n| &n.id == id)
    }
    pub fn frame(&self, id: &Id) -> Option<&Frame> {
        self.frames.iter().find(|e| &e.id == id)
    }
    pub fn material(&self, id: &Id) -> Option<&Material> {
        self.materials.iter().find(|m| &m.id == id)
    }
    pub fn section(&self, id: &Id) -> Option<&Section> {
        self.sections.iter().find(|s| &s.id == id)
    }
    pub fn area(&self, id: &Id) -> Option<&Area> {
        self.areas.iter().find(|a| &a.id == id)
    }
    pub fn area_section(&self, id: &Id) -> Option<&AreaSection> {
        self.area_sections.iter().find(|s| &s.id == id)
    }

    /// End length offsets of every frame (in `frames` order), at its first and second joint as given:
    /// given lengths, or automatic ones (docs/phases/PHASE4_PLAN.md, probe findings 2a):
    /// - a beam end where a column ends from below (the column's upper end): (t3/2) cos^2 a + (t2/2) sin^2 a
    ///   of that column (the largest of several), a = angle between the beam and the column's local 2 axis;
    /// - a column's upper end: the largest, over the beams there, of the beam's depth t3 x
    ///   max(cos^2 a, sin^2 a); a column's lower end: 0;
    /// - braces, trusses, null frames and everything else: 0 (slabs and walls never count).
    ///
    /// Columns and beams follow the model's angle limits (object types).
    pub fn end_offsets(&self) -> Vec<[f64; 2]> {
        let nodes: HashMap<&Id, &Node> = self.nodes.iter().map(|n| (&n.id, n)).collect();
        let lim = self.angle_limits();
        let sec = |f: &Frame| self.section(&f.section);
        let dims = |f: &Frame| {
            sec(f).map_or((0.0, 0.0), |s| {
                (s.depth.unwrap_or(0.0), s.width.unwrap_or(0.0))
            })
        };
        // Kind, lower and upper joint, horizontal direction of each frame.
        #[derive(PartialEq)]
        enum Kind {
            Column,
            Beam,
            Other,
        }
        let info: Vec<(Kind, usize, [f64; 2])> = self
            .frames
            .iter()
            .map(|f| {
                let (a, b) = (coords(nodes[&f.nodes[0]]), coords(nodes[&f.nodes[1]]));
                let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let h = (d[0] * d[0] + d[1] * d[1]).sqrt();
                let tilt = d[2].abs().atan2(h).to_degrees(); // from horizontal
                let kind = if f.is_null() || f.kind == FrameKind::Truss {
                    Kind::Other
                } else if 90.0 - tilt <= lim.column + 1e-9 {
                    Kind::Column
                } else if tilt <= lim.beam_floor + 1e-9 {
                    Kind::Beam
                } else {
                    Kind::Other
                };
                let upper = if d[2] >= 0.0 { 1 } else { 0 };
                let dir = if h > 0.0 {
                    [d[0] / h, d[1] / h]
                } else {
                    [0.0, 0.0]
                };
                (kind, upper, dir)
            })
            .collect();
        // cos^2 of the angle between a horizontal direction and a column's local 2 axis.
        let cos2 = |dir: [f64; 2], col: &Frame| {
            let a = col.angle.to_radians();
            let c = dir[0] * a.cos() + dir[1] * a.sin();
            (c * c).min(1.0)
        };
        fn ends_at(f: &Frame, end: usize) -> &Id {
            &f.nodes[end]
        }
        // Columns by their upper joint and beams by their joints, in frame order.
        let mut columns_below: HashMap<&Id, Vec<usize>> = HashMap::new();
        let mut beams_at: HashMap<&Id, Vec<usize>> = HashMap::new();
        for (i, (f, (kind, up, _))) in self.frames.iter().zip(&info).enumerate() {
            match kind {
                Kind::Column => columns_below.entry(ends_at(f, *up)).or_default().push(i),
                Kind::Beam => {
                    for (k, n) in f.nodes.iter().enumerate() {
                        if !f.nodes[..k].contains(n) {
                            beams_at.entry(n).or_default().push(i);
                        }
                    }
                }
                Kind::Other => {}
            }
        }
        self.frames
            .iter()
            .zip(&info)
            .map(|(f, (kind, upper, dir))| {
                let lengths = match f.end_offsets {
                    None => return [0.0, 0.0],
                    Some(EndOffsets::Lengths { i, j }) => return [i, j],
                    Some(EndOffsets::Auto(_)) => [0usize, 1],
                };
                lengths.map(|end| {
                    let joint = ends_at(f, end);
                    match kind {
                        Kind::Beam => {
                            // Pointing from this end into the beam.
                            let d = if end == 0 { *dir } else { [-dir[0], -dir[1]] };
                            columns_below
                                .get(joint)
                                .into_iter()
                                .flatten()
                                .map(|&c| &self.frames[c])
                                .map(|c| {
                                    let (t3, t2) = dims(c);
                                    let k = cos2(d, c);
                                    t3 / 2.0 * k + t2 / 2.0 * (1.0 - k)
                                })
                                .fold(0.0, f64::max)
                        }
                        Kind::Column if end == *upper => beams_at
                            .get(joint)
                            .into_iter()
                            .flatten()
                            .map(|&b| (&self.frames[b], &info[b].2))
                            .map(|(b, bd)| {
                                let k = cos2(*bd, f);
                                dims(b).0 * k.max(1.0 - k)
                            })
                            .fold(0.0, f64::max),
                        _ => 0.0,
                    }
                })
            })
            .collect()
    }

    /// Whether a frame is a beam by the model's angle limits (sloping less than the floor limit).
    pub fn is_beam(&self, f: &Frame) -> bool {
        let (Some(a), Some(b)) = (self.node(&f.nodes[0]), self.node(&f.nodes[1])) else {
            return false;
        };
        self.is_beam_between(a, b)
    }

    /// Whether a frame from `a` to `b` is a beam by the model's angle limits.
    fn is_beam_between(&self, a: &Node, b: &Node) -> bool {
        let (a, b) = (coords(a), coords(b));
        let h = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        (b[2] - a[2]).abs().atan2(h).to_degrees() <= self.angle_limits().beam_floor + 1e-9
    }

    /// Self-weight of a frame per unit length: material weight x A x weight modifiers (the A
    /// modifier does not change the weight). 0 for a null frame.
    pub fn frame_weight(&self, f: &Frame) -> f64 {
        match (self.material(&f.material), self.section(&f.section)) {
            (Some(m), Some(s)) => {
                s.self_weight.unwrap_or(m.weight * s.A) * self.frame_modifiers(f).weight
            }
            _ => 0.0,
        }
    }

    /// Self-weight of an area per unit area: material weight x thickness x weight modifiers (a Stiff
    /// section weighs nothing). 0 for a null area or an opening.
    pub fn area_weight(&self, a: &Area) -> f64 {
        self.area_section(&a.section).map_or(0.0, |s| {
            self.section_weight(s) * self.area_modifiers(a).weight
        })
    }

    /// An area section's weight per unit area: material weight x thickness; a deck's concrete (fill weight
    /// x concrete per area) plus the steel deck's unit weight.
    pub fn section_weight(&self, s: &AreaSection) -> f64 {
        let w = self.material(&s.material).map_or(0.0, |m| m.weight);
        match &s.deck {
            Some(d) => w * d.concrete_per_area() + d.unit_weight,
            None => w * s.thickness,
        }
    }

    /// `case` with its self-weight written out as loads (and its multiplier set to 0): a uniform load
    /// along -Z on every frame (a truss: half its weight at each end joint) and on every area, scaled
    /// by the case's multiplier, the self-weight multiplier of the load case.
    pub fn with_self_weight(&self, case: &LoadCase) -> LoadCase {
        let mut c = case.clone();
        let s = c.self_weight;
        c.self_weight = 0.0;
        if s != 0.0 {
            self.add_gravity(&mut c, s, false);
        }
        c
    }

    /// The structure's own weight (or, with `mass`, its mass times g: the mass modifiers instead of the
    /// weight modifiers) as loads along -Z, scaled by `s`, added to `c`. The rules of
    /// [`Model::with_self_weight`]; element mass is lumped from these loads (probe findings 3a, 4a).
    pub fn add_gravity(&self, c: &mut LoadCase, s: f64, mass: bool) {
        let nodes: HashMap<&Id, &Node> = self.nodes.iter().map(|n| (&n.id, n)).collect();
        let offsets = self.end_offsets();
        for (f, off) in self.frames.iter().zip(&offsets) {
            let w = s * if mass {
                self.frame_mass_weight(f)
            } else {
                self.frame_weight(f)
            };
            if w == 0.0 {
                continue;
            }
            if f.kind == FrameKind::Truss {
                let half = -w * length(nodes[&f.nodes[0]], nodes[&f.nodes[1]]) / 2.0;
                for n in &f.nodes {
                    c.node_loads.push(NodeLoad {
                        node: n.clone(),
                        fz: half,
                        ..Default::default()
                    });
                }
            } else {
                // Auto offsets: beams weigh over their clear length, columns and braces over
                // the whole length (probe findings 3a).
                let (a, b) = (nodes[&f.nodes[0]], nodes[&f.nodes[1]]);
                let clear = self.is_beam_between(a, b) && (off[0] > 0.0 || off[1] > 0.0);
                let l = length(a, b);
                c.frame_loads.push(FrameLoad::Uniform {
                    frame: f.id.clone(),
                    dir: LoadDir::Z,
                    w: -w,
                    from: clear.then_some(off[0]),
                    to: clear.then_some(l - off[1]),
                });
            }
        }
        for a in &self.areas {
            let q = s * if mass {
                self.area_mass_weight(a)
            } else {
                self.area_weight(a)
            };
            if q != 0.0 {
                c.area_loads.push(AreaLoad {
                    area: a.id.clone(),
                    dir: LoadDir::Z,
                    q: -q,
                });
            }
        }
    }

    /// A frame's mass per unit length times g: material weight x A x mass modifiers.
    pub fn frame_mass_weight(&self, f: &Frame) -> f64 {
        match (self.material(&f.material), self.section(&f.section)) {
            (Some(m), Some(s)) => {
                s.self_weight.unwrap_or(m.weight * s.A) * self.frame_modifiers(f).mass
            }
            _ => 0.0,
        }
    }

    /// An area's mass per unit area times g: its section's weight per area x mass modifiers.
    pub fn area_mass_weight(&self, a: &Area) -> f64 {
        self.area_section(&a.section).map_or(0.0, |s| {
            self.section_weight(s) * self.area_modifiers(a).mass
        })
    }

    /// Rigid arms of a frame whose insertion point changes its stiffness: from joint i and joint j
    /// (at `a` and `b`) to the element's ends (the section's centroid), global. None: the element runs
    /// between the joints (no insertion point, or one that moves the drawing only).
    pub fn frame_arms(&self, f: &Frame, a: [f64; 3], b: [f64; 3]) -> Option<[[f64; 3]; 2]> {
        let ins = f.insertion.as_ref().filter(|i| i.transform)?;
        let sec = self.section(&f.section)?;
        let ax = crate::axes::local_axes(a, b, f.angle);
        let centroid = sec.centroid_offset.unwrap_or([0.0; 2]);
        let [d2, d3] = ins.centroid_from_point(
            sec.depth.unwrap_or(0.0),
            sec.width.unwrap_or(0.0),
            centroid,
            sec.shear_centre_offset.unwrap_or(centroid),
        );
        let arm = |o: [f64; 3]| -> [f64; 3] {
            let joint = if ins.global {
                o
            } else {
                std::array::from_fn(|k| o[0] * ax[0][k] + o[1] * ax[1][k] + o[2] * ax[2][k])
            };
            std::array::from_fn(|k| joint[k] + d2 * ax[1][k] + d3 * ax[2][k])
        };
        let arms = [arm(ins.offset_i), arm(ins.offset_j)];
        arms.iter().flatten().any(|v| *v != 0.0).then_some(arms)
    }

    /// Rigid arm of an area whose insertion point changes its stiffness: from each joint to the
    /// shell's mid-plane (along local 3 `axis3`), global. None: the mid-plane is the plane of the joints.
    pub fn area_arm(&self, a: &Area, axis3: [f64; 3]) -> Option<[f64; 3]> {
        let ins = a.insertion.filter(|i| i.transform)?;
        let t = self.area_section(&a.section)?.thickness;
        let s = match ins.point {
            AreaCardinal::Top => -0.5,
            AreaCardinal::Middle => return None,
            AreaCardinal::Bottom => 0.5,
        };
        Some(axis3.map(|v| s * t * v))
    }

    /// The property modifiers a frame is analysed with: its section's times its own.
    pub fn frame_modifiers(&self, f: &Frame) -> FrameModifiers {
        self.section(&f.section)
            .map_or(FrameModifiers::default(), |s| s.modifiers)
            .times(&f.modifiers)
    }

    /// The property modifiers an area is analysed with: its section's (fixed values for a Stiff
    /// section) times its own.
    pub fn area_modifiers(&self, a: &Area) -> AreaModifiers {
        self.area_section(&a.section)
            .map_or(AreaModifiers::default(), |s| match s.slab_kind() {
                SlabType::Stiff => AreaModifiers::STIFF,
                _ => s.modifiers,
            })
            .times(&a.modifiers)
    }
    pub fn link(&self, id: &Id) -> Option<&Link> {
        self.links.iter().find(|l| &l.id == id)
    }

    /// Applies the end-order rule (see [`draw_order_reversed`]) to every frame and link. A reversed
    /// frame keeps the same physical meaning: `angle` changes sign and frame loads are mirrored
    /// (distances measured from the new end i, local 1 and 3 directions negated, since those axes flip).
    /// Returns the model as analysed and the ids of the frames that were reversed.
    pub fn oriented(&self) -> (Model, Vec<Id>) {
        let nodes: HashMap<&Id, &Node> = self.nodes.iter().map(|n| (&n.id, n)).collect();
        let mut m = self.clone();
        let mut reversed = Vec::new();
        // Original length of each reversed frame, for mirroring load positions.
        let mut lengths: HashMap<Id, f64> = HashMap::new();
        for e in &mut m.frames {
            let (Some(a), Some(b)) = (nodes.get(&e.nodes[0]), nodes.get(&e.nodes[1])) else {
                continue;
            };
            if draw_order_reversed(a, b) {
                e.nodes.swap(0, 1);
                e.angle = -e.angle;
                e.releases = e.releases.swapped();
                if let Some(EndOffsets::Lengths { i, j }) = e.end_offsets {
                    e.end_offsets = Some(EndOffsets::Lengths { i: j, j: i });
                }
                if let Some(ins) = &mut e.insertion {
                    *ins = ins.reversed();
                }
                reversed.push(e.id.clone());
                lengths.insert(e.id.clone(), length(a, b));
            }
        }
        // Links carry no loads or angle, so reversing one changes nothing but which end is i.
        for l in &mut m.links {
            if let (Some(a), Some(b)) = (nodes.get(&l.nodes[0]), nodes.get(&l.nodes[1])) {
                if draw_order_reversed(a, b) {
                    l.nodes.swap(0, 1);
                }
            }
        }
        // Areas start at their first lowest corner. The direction is kept, so the local
        // axes, loads and results mean the same.
        for a in &mut m.areas {
            let pts: Option<Vec<[f64; 3]>> = a
                .nodes
                .iter()
                .map(|id| nodes.get(id).map(|n| coords(n)))
                .collect();
            if let Some(pts) = pts {
                let first = crate::area::first_corner(&pts);
                a.nodes.rotate_left(first);
            }
        }
        for case in &mut m.load_cases {
            for ml in &mut case.frame_loads {
                let Some(&len) = lengths.get(ml.frame()) else {
                    continue;
                };
                let flip = |dir: LoadDir, v: f64| match dir {
                    LoadDir::L1 | LoadDir::L3 => -v,
                    _ => v,
                };
                match ml {
                    FrameLoad::Uniform {
                        dir, w, from, to, ..
                    } => {
                        *w = flip(*dir, *w);
                        let (a, b) = (from.unwrap_or(0.0), to.unwrap_or(len));
                        if from.is_some() || to.is_some() {
                            *from = Some(len - b);
                            *to = Some(len - a);
                        }
                    }
                    FrameLoad::Trapezoidal {
                        dir,
                        w1,
                        w2,
                        from,
                        to,
                        ..
                    } => {
                        let (v1, v2) = (flip(*dir, *w2), flip(*dir, *w1));
                        (*w1, *w2) = (v1, v2);
                        let (a, b) = (from.unwrap_or(0.0), to.unwrap_or(len));
                        if from.is_some() || to.is_some() {
                            *from = Some(len - b);
                            *to = Some(len - a);
                        }
                    }
                    FrameLoad::Point { dir, p, at, .. } => {
                        *p = flip(*dir, *p);
                        *at = len - *at;
                    }
                }
            }
        }
        (m, reversed)
    }
}

/// The rule for which end of a newly drawn frame is end i: the end with the lower Z; if both ends
/// are at the same height, the lower X; if X is also equal, the lower Y.
/// Returns true when the frame as given (a -> b) must be reversed.
pub fn draw_order_reversed(a: &Node, b: &Node) -> bool {
    let tol = 1e-9 * length(a, b).max(1.0);
    for (pa, pb) in [(a.z, b.z), (a.x, b.x), (a.y, b.y)] {
        if (pa - pb).abs() > tol {
            return pb < pa;
        }
    }
    false
}

/// Builds an id -> item map, failing on the first duplicate id.
fn unique<'a, T>(
    what: &str,
    items: impl Iterator<Item = (&'a Id, &'a T)>,
) -> Result<HashMap<&'a Id, &'a T>, Error> {
    let mut map = HashMap::new();
    for (id, item) in items {
        if map.insert(id, item).is_some() {
            return Err(Error::Invalid(format!("duplicate {what} id {id}")));
        }
    }
    Ok(map)
}

pub fn coords(n: &Node) -> [f64; 3] {
    [n.x, n.y, n.z]
}

pub fn length(a: &Node, b: &Node) -> f64 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(x: f64, y: f64, z: f64) -> Node {
        Node {
            id: Id::default(),
            x,
            y,
            z,
            in_mesh: None,
            diaphragm: None,
        }
    }


    #[test]
    fn old_key_names_still_load() {
        // Files written before Phase 3a call frames `elements` and frame loads `member_loads`.
        let old = r#"{"materials": [{"id": "m", "E": 1, "nu": 0.3}], "sections": [{"id": "s", "A": 1}],
            "nodes": [{"id": "1"}, {"id": "2", "x": 1}],
            "elements": [{"id": "1", "type": "truss", "nodes": ["1", "2"], "material": "m", "section": "s"}],
            "load_cases": [{"id": "LC", "member_loads": [{"type": "point", "element": "1", "dir": "Z", "p": 1, "at": 0.5}]}]}"#;
        let m: Model = serde_json::from_str(old).unwrap();
        assert_eq!(m.frames.len(), 1);
        assert_eq!(m.load_cases[0].frame_loads[0].frame(), &Id::from("1"));
        let new = serde_json::to_value(&m).unwrap();
        assert!(new.get("elements").is_none() && new["frames"].is_array());
        assert_eq!(new["load_cases"][0]["frame_loads"][0]["frame"], "1");
    }
}
