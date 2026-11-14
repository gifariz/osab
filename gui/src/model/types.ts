// The model and results, in the same JSON shape as the Rust solver (docs/MODEL_FORMAT.md).

import type { ShapeSpec } from "./sectionShapes";

export type Id = string;
export type Vec3 = [number, number, number];

export const DOFS = ["ux", "uy", "uz", "rx", "ry", "rz"] as const;
export type Dof = (typeof DOFS)[number];
export const FORCES = ["fx", "fy", "fz", "mx", "my", "mz"] as const;
export type ForceName = (typeof FORCES)[number];

/** Active degrees of freedom: space frame, plane frame (X-Z) or plane grid (X-Y). */
export type Dimension = "3D" | "2D" | "grid";

export interface Units {
  force: string;
  length: string;
}

/** Material type: a label with its own strengths; the analysis does not use it. */
export type MaterialType = "steel" | "concrete" | "other";

export interface Material {
  id: Id;
  /** Absent only in files written before material types (the file reader always sets it). */
  type?: MaterialType;
  E: number;
  G?: number;
  nu?: number;
  /** Weight per unit volume, for self-weight (absent = 0). */
  weight?: number;
  /** Steel: yield and tensile strength (stress); not used by the analysis. */
  fy?: number;
  fu?: number;
  /** Concrete: compressive strength f'c (stress); not used by the analysis. */
  fc?: number;
}

/** Frame property modifiers, in the usual order: factors on section values, mass and weight. */
export const FRAME_MODIFIERS = ["A", "As2", "As3", "J", "I22", "I33", "mass", "weight"] as const;
export type FrameModifier = (typeof FRAME_MODIFIERS)[number];
/** Absent values are 1. A frame uses its section's modifiers times its own. */
export type FrameModifiers = Partial<Record<FrameModifier, number>>;

/** End forces an end release sets to zero (local axes), in the usual order. */
export const RELEASES = ["P", "V2", "V3", "T", "M2", "M3"] as const;
export type Release = (typeof RELEASES)[number];
/** End releases of a frame at end i and end j (end order). */
export interface Releases {
  i?: Release[];
  j?: Release[];
}

/** Shell property modifiers, in the usual order: membrane, bending, transverse shear, mass, weight. */
export const AREA_MODIFIERS = ["f11", "f22", "f12", "m11", "m22", "m12", "v13", "v23", "mass", "weight"] as const;
export type AreaModifier = (typeof AREA_MODIFIERS)[number];
/** Absent values are 1. An area uses its section's modifiers times its own. */
export type AreaModifiers = Partial<Record<AreaModifier, number>>;

/** I33 = bending about local 3 (major for a beam), As2/As3 = shear areas. */
export interface Section {
  id: Id;
  A: number;
  I22: number;
  I33: number;
  J: number;
  As2?: number;
  As3?: number;
  /** Section property modifiers. */
  modifiers?: FrameModifiers;
  /** Depth (along local 2) and width (along local 3), for automatic end offsets; written from the shape. */
  depth?: number;
  width?: number;
  /** Product of inertia (angles, L sections): the member bends about its principal axes. */
  I23?: number;
  /** The centroid from the middle of the box, along local 2 and 3 (asymmetric shapes; cardinal points). */
  centroid_offset?: [number, number];
  /** Self weight per length of a composite section (each material's area times its weight). */
  self_weight?: number;
  /** The shape it was made from; the analysis works the numbers out from it again. */
  shape?: ShapeSpec;
}

/** A pier label: walls and columns with the same label at a story make one pier (forces at its top and bottom). */
export interface PierLabel {
  id: Id;
  color?: string;
}

/** A spandrel label: walls and beams with the same label make a spandrel (forces at its left and right end). */
export interface SpandrelLabel {
  id: Id;
  /** One spandrel over every story (otherwise one per story). */
  multi_story?: boolean;
  color?: string;
}

/** A group: its name, its colour, and the objects in it. */
export interface Group {
  id: Id;
  color?: string;
  nodes?: Id[];
  frames?: Id[];
  links?: Id[];
  areas?: Id[];
}

export interface ModelNode {
  id: Id;
  /** Label by plan position, the same on every story (J, C/B/D, L, F/W and a number; see labels.ts). */
  label?: string;
  x: number;
  y: number;
  z: number;
  /**
   * *Include in mesh*: only matters for a joint with nothing attached (e.g. one that only carries
   * a joint load). Absent = no: it is left out of the mesh and its loads go to the floor element it lies in.
   */
  in_mesh?: boolean;
  /**
   * *Special joint*: stays when the objects attached to it are deleted or moved away. A joint that is not
   * special is deleted when an edit leaves it with nothing attached. Joints drawn alone are special.
   */
  special?: boolean;
  /** Rigid diaphragm of this joint. */
  diaphragm?: Id;
}

/**
 * A diaphragm: per story, the joints of a rigid one move together in their plane (UX, UY, RZ); a
 * semi-rigid one ties nothing (the floor's own stiffness acts) and only groups the floor (user wind loads).
 */
export interface Diaphragm {
  id: Id;
  semi_rigid?: boolean;
}

/** A story level as the solver reads it (written from the grid's stories). */
export interface StoryLevel {
  id: Id;
  z: number;
}

export type FrameKind = "frame" | "truss";

export interface Frame {
  id: Id;
  /** Label by plan position, the same on every story (J, C/B/D, L, F/W and a number; see labels.ts). */
  label?: string;
  type: FrameKind;
  /** End i, end j (end order: lower Z, then lower X, then lower Y is end i). */
  nodes: [Id, Id];
  /** "" for a null frame. */
  material: Id;
  /** "" = a null frame (section "None"): no stiffness; floors are meshed along it. */
  section: Id;
  /** Rotation of local axes 2 and 3 about local 1, degrees. */
  angle?: number;
  /** *Include in mesh*. Absent = yes; false = floors are not meshed along it (a stiffener). */
  in_mesh?: boolean;
  /** Property modifiers of this frame, times its section's. */
  modifiers?: FrameModifiers;
  /** End releases (pins, torsion releases…). */
  releases?: Releases;
  /**
   * End length offsets: "auto" (Automatic from Connectivity, the default for a new frame) or
   * given lengths at end i and end j. Absent = none.
   */
  end_offsets?: EndOffsets;
  /** Rigid-zone factor (0 to 1): the part of the offsets that is rigid in bending and shear. */
  rigid_zone?: number;
  /** Insertion point: where the section sits relative to the line of its joints. Absent = the centroid. */
  insertion?: FrameInsertion;
  /** Line spring property along this frame (one of `line_spring_props`). */
  line_spring?: Id;
  /** Pier label (vertical frames). */
  pier?: Id;
  /** Spandrel label (horizontal frames). */
  spandrel?: Id;
}

/** Line spring property: stiffness per unit length along local 1, 2, 3 and about local 1. Absent = 0. */
export interface LineSpringProp {
  id: Id;
  u1?: number;
  u2?: number;
  u3?: number;
  r1?: number;
}

/** Area spring property: stiffness per unit area along local 1, 2, 3 (u3: the subgrade modulus). Absent = 0. */
export interface AreaSpringProp {
  id: Id;
  u1?: number;
  u2?: number;
  u3?: number;
}

export type EndOffsets = "auto" | { i: number; j: number };

/**
 * Insertion point of a frame: cardinal point 1-9 on the box around the section (1 bottom left ... 9 top
 * right, in local 2-3), 10 centroid, 11 shear centre; mirrors; joint offsets from each joint to the
 * cardinal point (local 1, 2, 3 or global X, Y, Z); `transform`: the stiffness follows the offsets
 * (off, the default: they move the drawing only).
 */
export interface FrameInsertion {
  point: number;
  mirror2?: boolean;
  mirror3?: boolean;
  offset_i?: Vec3;
  offset_j?: Vec3;
  global?: boolean;
  transform?: boolean;
}

/** Insertion point of an area: which face lies on the plane of its joints. */
export interface AreaInsertion {
  point: "top" | "middle" | "bottom";
  transform?: boolean;
}

export type Fix = "fixed" | "pinned" | Dof[];

export interface Support {
  node: Id;
  fix: Fix;
}

export type NodeLoad = { node: Id } & Partial<Record<ForceName, number>>;

/** Joint spring stiffness in global axes: ux uy uz force/length, rx ry rz force·length/rad. */
export type Spring = { node: Id } & Partial<Record<Dof, number>>;

/** Added joint mass: translations in mass units, rotations as inertia. */
export type JointMass = { node: Id } & Partial<Record<Dof, number>>;

/** Mass source. Absent fields are the defaults (all true, no load patterns). */
export interface MassSource {
  elements?: boolean;
  joint_masses?: boolean;
  loads?: { pattern: Id; scale: number }[];
  lateral_only?: boolean;
  lump_at_stories?: boolean;
}

/**
 * P-Delta options for the whole model: none, non-iterative from the mass, or iterative from a load (load
 * patterns times factors). Every case then uses the stiffness with P-Delta. Absent = none.
 */
export interface PDelta {
  method: "none" | "mass" | "loads";
  /** The P-Delta load of the `loads` method. */
  loads?: { pattern: Id; scale: number }[];
}

/** Ground accelerations a Ritz case may start from. */
export const RITZ_ACCELS = ["UX", "UY", "UZ", "RX", "RY", "RZ"] as const;

/** A modal case: "Modal" with 12 Eigen vectors in a new model; or Ritz vectors from starting loads. */
export interface ModalCase {
  id: Id;
  modes: number;
  /** Absent = Eigen. */
  type?: "eigen" | "ritz";
  /** Ritz starting loads, in order. */
  starts?: RitzStart[];
}

/**
 * One Ritz starting load: a ground acceleration (a rotation RX, RY, RZ turns about the centre of mass) or a load
 * pattern; `cycles` limits its vectors (absent = no limit).
 */
export interface RitzStart {
  accel?: (typeof RITZ_ACCELS)[number];
  pattern?: Id;
  cycles?: number;
}

/** A rigid link: its two joints move as one rigid body (all DOFs tied). */
export interface Link {
  id: Id;
  /** Label by plan position, the same on every story (J, C/B/D, L, F/W and a number; see labels.ts). */
  label?: string;
  type: "rigid";
  /** End i, end j (same end-order rule as members). */
  nodes: [Id, Id];
}

/** Global X/Y/Z or local 1/2/3. */
export type LoadDir = "X" | "Y" | "Z" | "1" | "2" | "3";

/** Shell types. */
export type ShellType = "shell-thin" | "membrane" | "shell-thick";

/**
 * Slab types: where floors overlap only one gets elements (an opening, then an object inside
 * another, then Stiff over Drop over Slab, then the smaller one). Stiff (rigid zones over columns and
 * walls) has 100 times the bending stiffness of its thickness.
 */
export type SlabType = "slab" | "drop" | "stiff";

/**
 * Property type of an area section: a label for users and design (a slab, deck or wall property). Any type may
 * be used on any area, which is a floor or a wall by its slope. A deck also brings its own rules (see `Deck`).
 */
export type AreaPropertyType = "slab" | "deck" | "wall";

/** Deck type: concrete on a steel deck, the steel deck alone, or a solid slab analysed as a deck. */
export type DeckType = "filled" | "unfilled" | "solid";

/**
 * A deck section's data (lengths in m, `unit_weight` the steel deck's weight per area). The ribs run along
 * the area's local axis 1, the span direction.
 */
export interface Deck {
  type: DeckType;
  /** The steel deck's material. */
  steel: Id;
  /** Concrete above the ribs (filled), or the slab's depth (solid). */
  slab_depth: number;
  rib_depth: number;
  rib_width_top: number;
  rib_width_bottom: number;
  /** Centre to centre. */
  rib_spacing: number;
  /** The steel deck's thickness for in-plane shear (an unfilled deck's membrane thickness). */
  shear_thickness: number;
  unit_weight: number;
}

/** Area section (shell property): membrane thickness = bending thickness. */
export interface AreaSection {
  id: Id;
  type: ShellType;
  material: Id;
  thickness: number;
  /** Absent = "slab" (the file reader always sets it). */
  property_type?: AreaPropertyType;
  /** Absent = "slab". Slab sections only. */
  slab_type?: SlabType;
  /** Section property modifiers (a Stiff section uses fixed values instead). */
  modifiers?: AreaModifiers;
  /** One-way load distribution (Membrane only): loads span along local axis 1. Absent = two-way. */
  one_way?: boolean;
  /**
   * Deck sections only: always Membrane and one-way; `material` is the fill's concrete and `thickness` the
   * total depth (for drawing); weight and stiffness come from the deck (model/deck.ts).
   */
  deck?: Deck;
}

/** Mesh option of one area: "none" (one element: 3 or 4 corners, convex) or its own maximum size. */
export type AreaMesh = "none" | { size: number };

/**
 * A mesh stored on an area (user mesh, Edit > Store Mesh): joint coordinates and elements (3 or 4
 * indices into `joints`). The mesh step uses it as it is.
 */
export interface UserMesh {
  joints: Vec3[];
  elements: number[][];
}

/**
 * A flat area object with three or more corners; corners start at the first lowest
 * one. Meshed areas may have any number of corners (edges not crossing); unmeshed ones 3 or 4,
 * convex.
 */
export interface Area {
  id: Id;
  /** Label by plan position, the same on every story (J, C/B/D, L, F/W and a number; see labels.ts). */
  label?: string;
  nodes: Id[];
  /** "" for an opening or a null area (property "None": no stiffness, does not cut the mesh). */
  section: Id;
  /** Rotation of local axes 1 and 2 about local 3, degrees. */
  angle?: number;
  /** An opening: cuts a hole in the floors and walls it lies in; no section, no loads. */
  opening?: true;
  /** Absent = the model's mesh settings. */
  mesh?: AreaMesh;
  /** "Add restraints on edge if corners have restraints". Absent = on for walls, off for floors. */
  edge_restraints?: boolean;
  /** A stored mesh, used instead of meshing the area. */
  user_mesh?: UserMesh;
  /** Property modifiers of this area, times its section's. */
  modifiers?: AreaModifiers;
  /** Rigid diaphragm of this area: all its joints (mesh joints too) join it. */
  diaphragm?: Id;
  /** Insertion point: which face lies on the plane of the joints. Absent = the middle. */
  insertion?: AreaInsertion;
  /** Area spring property over this area (one of `area_spring_props`). */
  area_spring?: Id;
  /** Pier label (vertical walls). */
  pier?: Id;
  /** Spandrel label (vertical walls). */
  spandrel?: Id;
}

/** Floor mesh types. */
export type FloorMesh = "general" | "rectangular";

/** Auto-mesh settings (missing fields = the defaults: General, 1.25 m). */
export interface MeshSettings {
  floor?: FloorMesh;
  floor_size?: number;
  wall_size?: number;
  /** Grading near openings and re-entrant corners, irregular regions of General floors only. Missing = on. */
  refine?: boolean;
}

/**
 * Two angle limits for object types (degrees): a frame within `column` of vertical is a column,
 * within `beam_floor` of horizontal a beam, else a brace; an area sloping less than `beam_floor` is a
 * floor, else a wall.
 */
export interface AngleLimits {
  column: number;
  beam_floor: number;
}

/** Uniform load on an area, force per area, in global X/Y/Z or the area's local 1/2/3. */
export interface AreaLoad {
  area: Id;
  dir: LoadDir;
  q: number;
}

export interface UniformLoad {
  type: "uniform";
  frame: Id;
  dir: LoadDir;
  w: number;
  from?: number;
  to?: number;
}

/** Load varying linearly from w1 at `from` to w2 at `to` (one stretch of trapezoidal loads). */
export interface TrapezoidalLoad {
  type: "trapezoidal";
  frame: Id;
  dir: LoadDir;
  w1: number;
  w2: number;
  from?: number;
  to?: number;
}

export interface PointLoad {
  type: "point";
  frame: Id;
  dir: LoadDir;
  p: number;
  at: number;
}

export type FrameLoad = UniformLoad | TrapezoidalLoad | PointLoad;

/** Load pattern types (they do not change the analysis; kept for later design features). */
export const PATTERN_TYPES = [
  "dead",
  "super-dead",
  "live",
  "reducible-live",
  "roof-live",
  "snow",
  "seismic",
  "wind",
  "temperature",
  "hydrostatic",
  "construction",
  "notional",
  "other",
] as const;
export type PatternType = (typeof PATTERN_TYPES)[number];

/** A user wind load: Fx, Fy, Mz at (x, y) on the floor of `diaphragm` at `story`. */
export interface WindLoad {
  story: Id;
  diaphragm: Id;
  fx?: number;
  fy?: number;
  mz?: number;
  x: number;
  y: number;
}

/** A load pattern: the loads, a type and a self-weight multiplier. */
export interface LoadPattern {
  id: Id;
  type: PatternType;
  /** Self-weight multiplier (1 for Dead in a new model; absent = 0). */
  self_weight?: number;
  node_loads: NodeLoad[];
  frame_loads: FrameLoad[];
  area_loads: AreaLoad[];
  /** "user-wind" on a Wind pattern: its `wind_loads` (story forces). */
  auto?: "user-wind";
  wind_loads?: WindLoad[];
  /** Where the user wind loads act (absent = each diaphragm's centre of mass). */
  wind_point?: "coordinates";
  /** How they reach a semi-rigid diaphragm (absent = into the floor under the point). */
  wind_semi_rigid?: "mass" | "nearest-joint";
  /** Kept for automatic combinations; no effect now. */
  reversible?: boolean;
}

/** A linear static load case: load patterns times scale factors. */
export interface LoadCase {
  id: Id;
  loads: { pattern: Id; scale: number }[];
}

export const SITE_CLASSES = ["A", "B", "C", "D", "E", "F"] as const;

/**
 * The ASCE 7-16 design spectrum: Ss and S1 in g, TL in s, the site class; `fa` and `fv` replace the site
 * coefficients of the code's tables (needed for class F and where the tables ask for a site study).
 */
export interface Asce716 {
  ss: number;
  s1: number;
  tl: number;
  site_class: (typeof SITE_CLASSES)[number];
  fa?: number;
  fv?: number;
}

/** A response spectrum function: value (no units) against period, a table or ASCE 7-16; damping absent = 0.05. */
export interface SpectrumFunction {
  id: Id;
  damping?: number;
  points?: [number, number][];
  asce7_16?: Asce716;
}

export const SPECTRUM_DIRS = ["X", "Y", "Z"] as const;
export const MODAL_COMBINATIONS = ["cqc", "srss", "absolute"] as const;
export const DIRECTIONAL_COMBINATIONS = ["srss", "absolute", "cqc3"] as const;

/** A ground acceleration of a spectrum case: the function times `scale` (m/s²) along X / Y (turned by `angle`, degrees about Z) or Z. */
export interface SpectrumLoad {
  dir: (typeof SPECTRUM_DIRS)[number];
  function: Id;
  scale: number;
  angle?: number;
}

/** A response spectrum case on the modal case; absent fields are the defaults (0.05, CQC, SRSS, factor 1, no eccentricity). */
export interface SpectrumCase {
  id: Id;
  loads: SpectrumLoad[];
  damping?: number;
  modal_combination?: (typeof MODAL_COMBINATIONS)[number];
  directional_combination?: (typeof DIRECTIONAL_COMBINATIONS)[number];
  abs_factor?: number;
  eccentricity?: number;
}

/** Load combination types in common use. */
export const COMBO_TYPES = ["linear-add", "envelope", "srss"] as const;
export type ComboType = (typeof COMBO_TYPES)[number];

/** A part of a combination: a load case or another combination, times `scale`. */
export type ComboItem = { case: Id; combo?: undefined; scale: number } | { combo: Id; case?: undefined; scale: number };

export interface LoadCombination {
  id: Id;
  type: ComboType;
  items: ComboItem[];
}

export interface GridLine {
  label: string;
  coord: number;
}

export interface Story {
  label: string;
  z: number;
}

/** Grid: X lines A, B, C…; Y lines 1, 2, 3…; stories Base, Story1… */
export interface Grid {
  x: GridLine[];
  y: GridLine[];
  stories: Story[];
}

/** Section shape a frame section was made from (for drawing it in the extruded view). */
export type SectionShape =
  | { shape: "rectangle"; depth: number; width: number }
  | { shape: "i"; depth: number; flangeWidth: number; flangeThickness: number; webThickness: number };

/** Desktop-app data kept in the model file. The solver ignores it. */
export interface GuiData {
  grid?: Grid;
  /** Shapes of frame sections defined from dimensions, by section id. */
  sectionShapes?: Record<Id, SectionShape>;
  /** Display unit system (values are always stored in kN, m, °C); missing = SI. */
  units?: "SI" | "MKS" | "US";
  /** View > Set Display Options (see scene/viewOptions.ts); missing = the defaults. */
  viewOptions?: unknown;
  [key: string]: unknown;
}

export interface Model {
  /** File format version (`MODEL_FORMAT`); the app writes the current one. */
  version?: number;
  title: string;
  /** "2D": plane frame in X-Z (UX, UZ, RY); "grid": plane grid in X-Y (UZ, RX, RY, a floor with its beams). */
  dimension: Dimension;
  units: Units;
  materials: Material[];
  sections: Section[];
  nodes: ModelNode[];
  /** Frame objects (older files: `elements`). */
  frames: Frame[];
  supports: Support[];
  load_patterns: LoadPattern[];
  load_cases: LoadCase[];
  load_combinations: LoadCombination[];
  /** Groups: named sets of objects (absent = none; "All" is every object and not stored). */
  groups?: Group[];
  /** Pier and spandrel labels (objects name them by `pier` and `spandrel`). */
  pier_labels?: PierLabel[];
  spandrel_labels?: SpandrelLabel[];
  /** Response spectrum functions and cases (absent = none). */
  spectrum_functions?: SpectrumFunction[];
  spectrum_cases?: SpectrumCase[];
  /** Cases set not to run: load cases, the modal case, spectrum cases (absent = every case runs). */
  not_run?: Id[];
  springs: Spring[];
  /** Line spring properties (frames refer to them by `line_spring`). */
  line_spring_props?: LineSpringProp[];
  /** Area spring properties (areas refer to them by `area_spring`). */
  area_spring_props?: AreaSpringProp[];
  /** Added joint masses. */
  masses?: JointMass[];
  /** Where the modal case's mass comes from; absent = the default. */
  mass_source?: MassSource;
  /** The modal case; absent = none. */
  modal?: ModalCase;
  /** P-Delta options; absent = none. */
  p_delta?: PDelta;
  /** When the structure has no stiffness in some direction: go on with a warning (absent) or stop. */
  on_instability?: "continue" | "stop";
  links: Link[];
  area_sections: AreaSection[];
  areas: Area[];
  /** Absent = the defaults. Areas analysed as one element are set per area (`Area.mesh` "none"). */
  mesh?: MeshSettings;
  /** Absent = 1 and 20 degrees. */
  angle_limits?: AngleLimits;
  /** Rigid diaphragms (a new model has D1, unassigned). */
  diaphragms?: Diaphragm[];
  /** Story levels for the solver (diaphragms are per story); written from the grid. */
  stories?: StoryLevel[];
  gui?: GuiData;
}

// ---- Results (written by the solver) ----

export interface Disp {
  ux: number;
  uy: number;
  uz: number;
  rx: number;
  ry: number;
  rz: number;
}

export interface Force {
  fx: number;
  fy: number;
  fz: number;
  mx: number;
  my: number;
  mz: number;
}

export const COMPONENTS = ["P", "V2", "V3", "T", "M2", "M3"] as const;
export type Component = (typeof COMPONENTS)[number];

export interface Station extends Record<Component, number> {
  x: number;
  ux: number;
  uy: number;
  uz: number;
}

export interface ElementResult {
  element: Id;
  nodes: [Id, Id];
  length: number;
  /** Unit vectors of local 1, 2, 3 (global coordinates). */
  axes: [Vec3, Vec3, Vec3];
  stations: Station[];
}

export const SHELL_COMPONENTS = ["F11", "F22", "F12", "M11", "M22", "M12", "V13", "V23"] as const;
export type ShellComponent = (typeof SHELL_COMPONENTS)[number];

export type ShellCorner = { node: Id } & Record<ShellComponent, number>;

/** What a shell contour shows: a component, or a principal / resultant value made from them. */
export const SHELL_VIEWS = ["F11", "F22", "F12", "FMAX", "FMIN", "M11", "M22", "M12", "MMAX", "MMIN", "V13", "V23", "VMAX"] as const;
export type ShellView = (typeof SHELL_VIEWS)[number];

/** Forces of one shell element (not averaged between elements): per unit length in the area's local axes. */
export interface ShellResult {
  element: Id;
  area: Id;
  nodes: Id[];
  axes: [Vec3, Vec3, Vec3];
  corners: ShellCorner[];
}

export interface CaseResult {
  id: Id;
  /** Set for a load combination: its type. */
  combination?: ComboType;
  /** For a combination with a maximum and a minimum: which one this is. */
  step?: "max" | "min";
  /** A response spectrum case: every value is a size (positive). */
  spectrum?: true;
  /** Sum of the support reactions and spring forces about the origin: FX FY FZ MX MY MZ. */
  base_reaction?: number[];
  /** Models with stories: drift X, Y of joints at a level over the joint below it, per story height. */
  joint_drifts?: JointDrift[];
  /** The largest joint drift of each story and direction, by size. */
  story_drifts?: StoryDrift[];
  /** P VX VY T MX MY that the structure below puts on the part above, at each story's top and bottom cut. */
  story_forces?: StoryForce[];
  /** P V2 V3 T M2 M3 of each pier at its top and bottom, of each spandrel at its left and right end. */
  pier_forces?: PierForce[];
  spandrel_forces?: SpandrelForce[];
  displacements: ({ node: Id } & Disp)[];
  reactions: ({ node: Id } & Force)[];
  frames: ElementResult[];
  /** Forces in the rigid links (same fields as members; `element` is the link id). */
  links?: ElementResult[];
  /** Shell forces per element corner (one element per area in Phase 3a). */
  shells?: ShellResult[];
  /** Its values are not loaded yet (results on demand, `ipc/resultCases.ts`): the tables are empty. */
  pending?: true;
  /** The case has shell forces (set in a summary, where `shells` is empty). */
  has_shells?: boolean;
  /** Global checks; none for an envelope (max/min). */
  equilibrium?: {
    applied: number[];
    reactions: number[];
    relative_error: number;
    residual: number;
  };
}

export interface JointDrift {
  story: string;
  node: Id;
  below: Id;
  drift: [number, number];
}

export interface StoryDrift {
  story: string;
  dir: "X" | "Y";
  drift: number;
  node: Id;
}

export interface StoryForce {
  story: string;
  top: number[];
  bottom: number[];
}

export interface PierForce {
  story: string;
  pier: Id;
  top: number[];
  bottom: number[];
}

export interface SpandrelForce {
  story: string;
  spandrel: Id;
  left: number[];
  right: number[];
}

/** A pier's axis angle (degrees from X) and the centroids of its bottom and top cut. */
export interface PierSection {
  story: string;
  pier: Id;
  angle: number;
  bottom: Vec3;
  top: Vec3;
}

/** The centroids of a spandrel's left and right cut. */
export interface SpandrelSection {
  story: string;
  spandrel: Id;
  left: Vec3;
  right: Vec3;
}

/** A joint the mesh step added (`~1`, `~2`…). */
export interface MeshJoint {
  id: Id;
  x: number;
  y: number;
  z: number;
}

/** A shell element (`F1-1`…) or frame piece (`B1-1`…) of the analysis mesh, with its parent object. */
export interface MeshElement {
  id: Id;
  parent: Id;
  nodes: Id[];
}

/** The analysis model the mesh step made (when it made anything). */
export interface MeshInfo {
  joints: MeshJoint[];
  shells: MeshElement[];
  frames: MeshElement[];
  edge_constraints: { area: Id; edge: [Id, Id]; node: Id }[];
}

export interface Results {
  title: string;
  units: Units;
  notes: string[];
  cases: CaseResult[];
  /** The analysis mesh, when the model was meshed. */
  mesh?: MeshInfo;
  /** The modal case's results, when the model has one. */
  modal?: ModalResults;
  /** Per story and diaphragm: masses, centres of mass and of rigidity. */
  diaphragm_centers?: DiaphragmCentre[];
  /** Piers' axis angles and cut centroids, spandrels' cut centroids. */
  pier_sections?: PierSection[];
  spandrel_sections?: SpandrelSection[];
}

/** One row of the centres of mass and rigidity (semi-rigid: masses 0 and the centre of mass only). */
export interface DiaphragmCentre {
  story: string;
  diaphragm: Id;
  mass_x: number;
  mass_y: number;
  xcm: number | null;
  ycm: number | null;
  cum_mass_x: number;
  cum_mass_y: number;
  xccm: number | null;
  yccm: number | null;
  xcr: number | null;
  ycr: number | null;
}

/** One mode of the modal case (vectors are UX UY UZ RX RY RZ). */
export interface ModeResult {
  mode: number;
  period: number;
  frequency: number;
  circular_frequency: number;
  eigenvalue: number;
  participation: number[];
  mass_ratio: number[];
  sum_mass_ratio: number[];
  /** A static mode of a Ritz case (a load on joints without mass): no period (0), shape at unit stiffness. */
  static_mode?: boolean;
  /** Joint displacements at unit modal mass. */
  shape: ModeShape;
  /** The shape is not here yet (the desktop app sends it when a view shows the mode): empty until loaded. */
  pending?: boolean;
}

export interface LoadParticipation {
  kind: "acceleration" | "pattern";
  name: string;
  static_percent: number;
  dynamic_percent: number;
}

/** A mode's joint displacements. */
export type ModeShape = ({ node: Id } & Disp)[];

export interface ModalResults {
  id: Id;
  modes: ModeResult[];
  masses: JointMass[];
  free_mass: number[];
  centre_of_mass: Vec3;
  /** Modal load participation ratios (%): accelerations UX, UY, UZ, then a Ritz case's starting patterns. */
  load_participation?: LoadParticipation[];
  /** The desktop app's summary: the joints of every shape, in the order its numbers come. */
  joints?: Id[];
}

/** An error from the solver or the file commands. */
export interface SolverError {
  kind: string;
  message: string;
  node?: Id;
  dof?: string;
  case?: Id;
  /** Kind "too_big" (the browser demo's size limit): the analysis model's joints and the limit. */
  joints?: number;
  limit?: number;
  /** Kind "too_big_for_memory": the memory the analysis needs and the memory free, in GB. */
  needed_gb?: number;
  free_gb?: number;
}
