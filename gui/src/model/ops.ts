// Pure edit operations: each takes a model and returns a new one (the input is never changed).

import { t, type Key } from "../i18n";
import { isFloor, shapeProblem } from "./area";
import { deckDepth, newDeck, readDeck } from "./deck";
import { MATERIAL_TYPES, STRENGTHS, guessMaterialType } from "./materials";
import { drawOrderReversed, dist, pos } from "./geometry";
import { letters, makeGrid } from "./grid";
import { modifierProblem, readModifiers, withModifiers } from "./modifiers";
import { drawnFrameInsertion } from "./insertion";
import { DEFAULT_ANGLE_LIMITS } from "./objectTypes";
import type { UnitSystem } from "../units/units";
import { rectangle } from "./sections";
import type { ShapeSpec } from "./sectionShapes";
import { pruneGroups, readGroup } from "./groups";
import { readWallLabel } from "./wallLabels";
import { readSpectrumCase, readSpectrumFunction } from "./spectrum";
import {
  AREA_MODIFIERS,
  DOFS,
  FORCES,
  FRAME_MODIFIERS,
  RELEASES,
  type AngleLimits,
  type Results,
  type Area,
  type AreaLoad,
  type AreaSection,
  type ComboItem,
  type Dimension,
  type Dof,
  type LoadCombination,
  type Diaphragm,
  type Frame,
  type FrameKind,
  type FrameModifier,
  type Release,
  type Releases,
  type EndOffsets,
  type FrameModifiers,
  type AreaModifiers,
  type Fix,
  type ForceName,
  type Grid,
  type Id,
  type Link,
  type LoadCase,
  type LoadDir,
  type LoadPattern,
  type NodeLoad,
  type PatternType,
  type WindLoad,
  PATTERN_TYPES,
  type Material,
  type FrameLoad,
  type FrameInsertion,
  type AreaInsertion,
  type LineSpringProp,
  type AreaSpringProp,
  type MeshSettings,
  type Model,
  type ModelNode,
  type Section,
  type SectionShape,
  type Spring,
  type JointMass,
  type MassSource,
  type ModalCase,
  RITZ_ACCELS,
  type RitzStart,
  type PDelta,
  type Vec3,
} from "./types";

/** Names of new objects: joints plain numbers, frames M (member), links K, areas A, each plus a number. */
export const FRAME_NAME = "M";
export const LINK_NAME = "K";
export const AREA_NAME = "A";

/** Two joints closer than this (model length units) are the same joint. */
export const SAME_POINT = 1e-6;

export class EditError extends Error {}

const clone = <T>(v: T): T => structuredClone(v);


/** A frame's insertion point from a file; absent when it is the default (centroid, nothing else). */
export function readFrameInsertion(raw: unknown): FrameInsertion | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const r = raw as Record<string, unknown>;
  const p = Math.round(Number(r.point ?? 10));
  const out: FrameInsertion = { point: p >= 1 && p <= 11 ? p : 10 };
  if (r.mirror2 === true) out.mirror2 = true;
  if (r.mirror3 === true) out.mirror3 = true;
  const vec = (v: unknown): Vec3 | undefined => {
    if (!Array.isArray(v)) return undefined;
    const o = [0, 1, 2].map((k) => Number(v[k]) || 0) as Vec3;
    return o.some((x) => x !== 0) ? o : undefined;
  };
  const oi = vec(r.offset_i);
  const oj = vec(r.offset_j);
  if (oi) out.offset_i = oi;
  if (oj) out.offset_j = oj;
  if (r.global === true) out.global = true;
  if (r.transform === true) out.transform = true;
  return Object.keys(out).length === 1 && out.point === 10 ? undefined : out;
}

/** An area's insertion point from a file; absent when it is the default (middle, no transform). */
export function readAreaInsertion(raw: unknown): AreaInsertion | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const r = raw as Record<string, unknown>;
  const point = r.point === "top" || r.point === "bottom" ? r.point : "middle";
  const out: AreaInsertion = { point, ...(r.transform === true ? { transform: true } : {}) };
  return point === "middle" && !out.transform ? undefined : out;
}

/**
 * The model file format this app reads and writes (the solver's `MODEL_FORMAT`). Files without `version` are
 * older ones; their old field names are read below. A change to the file's layout raises it, with an upgrade
 * step for the files before (docs/MODEL_FORMAT.md, *File format version*).
 */
export const MODEL_FORMAT = 1;

/**
 * Fills defaults and turns numeric ids into text, like the solver does when it reads a file. The result has
 * the current `version`, so a saved file carries it. A file from a newer OSAB is refused.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function normalizeModel(raw: any): Model {
  const id = (v: unknown) => String(v ?? "");
  const num = (v: unknown) => (typeof v === "number" ? v : Number(v ?? 0));
  const version = raw?.version;
  if (typeof version === "number" && version > MODEL_FORMAT) throw new EditError(t("err.newerFormat", { v: version, max: MODEL_FORMAT }));
  const m: Model = {
    version: MODEL_FORMAT,
    title: raw?.title ?? "",
    dimension: raw?.dimension === "2D" ? "2D" : raw?.dimension === "grid" ? "grid" : "3D",
    units: { force: raw?.units?.force ?? "kN", length: raw?.units?.length ?? "m" },
    materials: (raw?.materials ?? []).map((x: Material) => {
      const mat: Material = { id: id(x.id), type: MATERIAL_TYPES.some(([k]) => k === x.type) ? x.type : guessMaterialType(num(x.E)), E: num(x.E) };
      if (x.G !== undefined && x.G !== null) mat.G = num(x.G);
      if (x.nu !== undefined && x.nu !== null) mat.nu = num(x.nu);
      if (x.weight) mat.weight = num(x.weight);
      for (const k of STRENGTHS[mat.type!]) if (Number(x[k]) > 0) mat[k] = num(x[k]);
      return mat;
    }),
    sections: (raw?.sections ?? []).map((s: Section) => {
      const sec: Section = { id: id(s.id), A: num(s.A), I22: num(s.I22), I33: num(s.I33), J: num(s.J) };
      if (s.As2) sec.As2 = num(s.As2);
      if (s.As3) sec.As3 = num(s.As3);
      const mods = readModifiers(FRAME_MODIFIERS, s.modifiers);
      if (mods) sec.modifiers = mods;
      if (num(s.depth) > 0) sec.depth = num(s.depth);
      if (num(s.width) > 0) sec.width = num(s.width);
      if (num(s.I23)) sec.I23 = num(s.I23);
      if (Array.isArray(s.centroid_offset)) sec.centroid_offset = [num(s.centroid_offset[0]), num(s.centroid_offset[1])];
      if (num(s.self_weight) > 0) sec.self_weight = num(s.self_weight);
      if (s.shape && typeof s.shape === "object" && typeof (s.shape as ShapeSpec).type === "string") sec.shape = clone(s.shape);
      return sec;
    }),
    nodes: (raw?.nodes ?? []).map((n: ModelNode) => {
      const node: ModelNode = { id: id(n.id), x: num(n.x), y: num(n.y), z: num(n.z) };
      if (typeof n.in_mesh === "boolean") node.in_mesh = n.in_mesh;
      if (n.special === true) node.special = true;
      if (n.diaphragm) node.diaphragm = id(n.diaphragm);
      if (n.label) node.label = id(n.label);
      return node;
    }),
    frames: (raw?.frames ?? raw?.elements ?? []).map((e: Frame) => {
      const el: Frame = {
        id: id(e.id),
        type: e.type === "truss" ? "truss" : "frame",
        nodes: [id(e.nodes?.[0]), id(e.nodes?.[1])],
        material: id(e.material),
        section: id(e.section),
      };
      if (e.angle) el.angle = num(e.angle);
      if (typeof e.in_mesh === "boolean") el.in_mesh = e.in_mesh;
      const mods = readModifiers(FRAME_MODIFIERS, e.modifiers);
      if (mods) el.modifiers = mods;
      const rel = cleanReleases(e.releases);
      if (rel) el.releases = rel;
      if (e.end_offsets === "auto") el.end_offsets = "auto";
      else if (e.end_offsets && typeof e.end_offsets === "object") el.end_offsets = { i: num(e.end_offsets.i), j: num(e.end_offsets.j) };
      if (e.rigid_zone) el.rigid_zone = num(e.rigid_zone);
      const ins = readFrameInsertion(e.insertion);
      if (ins) el.insertion = ins;
      if (e.line_spring) el.line_spring = id(e.line_spring);
      if (e.pier) el.pier = id(e.pier);
      if (e.spandrel) el.spandrel = id(e.spandrel);
      if (e.label) el.label = id(e.label);
      return el;
    }),
    supports: (raw?.supports ?? []).map((s: { node: unknown; fix: Fix }) => ({ node: id(s.node), fix: s.fix })),
    ...readPatternsAndCases(raw),
    load_combinations: (raw?.load_combinations ?? []).map((c: LoadCombination) => ({
      id: id(c.id),
      type: c.type,
      items: (c.items ?? []).map((it: ComboItem) =>
        it.combo !== undefined && it.combo !== null ? { combo: id(it.combo), scale: num(it.scale) } : { case: id(it.case), scale: num(it.scale) },
      ),
    })),
    springs: (raw?.springs ?? []).map((sp: Spring) => {
      const out: Spring = { node: id(sp.node) };
      for (const d of DOFS) if (sp[d]) out[d] = num(sp[d]);
      return out;
    }),
    ...(Array.isArray(raw?.masses) && raw.masses.length
      ? {
          masses: raw.masses.map((sp: JointMass) => {
            const out: JointMass = { node: id(sp.node) };
            for (const d of DOFS) if (sp[d]) out[d] = num(sp[d]);
            return out;
          }),
        }
      : {}),
    ...(raw?.mass_source && typeof raw.mass_source === "object" ? { mass_source: cleanMassSource(readMassSource(raw.mass_source)) } : {}),
    ...(raw?.modal && typeof raw.modal === "object" ? { modal: readModal(raw.modal) } : {}),
    links: (raw?.links ?? []).map((l: Link) => ({ id: id(l.id), type: "rigid" as const, nodes: [id(l.nodes?.[0]), id(l.nodes?.[1])] as [Id, Id], ...(l.label ? { label: id(l.label) } : {}) })),
    area_sections: (raw?.area_sections ?? []).map((s: AreaSection) => {
      const mods = readModifiers(AREA_MODIFIERS, s.modifiers);
      const deck = s.property_type === "deck" ? readDeck(s.deck) : null;
      return {
        id: id(s.id),
        type: s.type,
        material: id(s.material),
        thickness: num(s.thickness),
        ...(s.property_type === "wall" || s.property_type === "slab" ? { property_type: s.property_type } : {}),
        ...((s.slab_type === "drop" || s.slab_type === "stiff") && s.property_type !== "wall" && !deck ? { slab_type: s.slab_type } : {}),
        ...(s.one_way === true && s.type === "membrane" ? { one_way: true } : {}),
        ...(mods ? { modifiers: mods } : {}),
        // A deck (with its data) is always a one-way Membrane.
        ...(deck ? { type: "membrane" as const, property_type: "deck" as const, one_way: true, deck } : {}),
      };
    }),
    areas: (raw?.areas ?? []).map((a: Area) => {
      const out: Area = { id: id(a.id), nodes: (a.nodes ?? []).map(id), section: a.opening ? "" : id(a.section) };
      if (a.label) out.label = id(a.label);
      if (a.angle) out.angle = num(a.angle);
      if (a.opening) out.opening = true;
      if (a.mesh === "none") out.mesh = "none";
      else if (a.mesh && typeof a.mesh === "object") out.mesh = { size: num(a.mesh.size) };
      if (typeof a.edge_restraints === "boolean") out.edge_restraints = a.edge_restraints;
      const mods = readModifiers(AREA_MODIFIERS, a.modifiers);
      if (mods) out.modifiers = mods;
      if (a.diaphragm) out.diaphragm = id(a.diaphragm);
      const ins = readAreaInsertion(a.insertion);
      if (ins) out.insertion = ins;
      if (a.area_spring && !a.opening) out.area_spring = id(a.area_spring);
      if (a.pier && !a.opening) out.pier = id(a.pier);
      if (a.spandrel && !a.opening) out.spandrel = id(a.spandrel);
      if (a.user_mesh && Array.isArray(a.user_mesh.joints) && Array.isArray(a.user_mesh.elements)) {
        out.user_mesh = {
          joints: a.user_mesh.joints.map((p) => [num(p[0]), num(p[1]), num(p[2])] as Vec3),
          elements: a.user_mesh.elements.map((e) => e.map(num)),
        };
      }
      return out;
    }),
  };
  // Spring properties: stiffness values above 0 (zero and negative ones left out).
  const springProps = <T extends { id: Id }>(list: unknown, keys: readonly string[]): T[] | undefined => {
    if (!Array.isArray(list) || !list.length) return undefined;
    return list.map((p: Record<string, unknown>) => {
      const out: Record<string, unknown> = { id: id(p.id) };
      for (const k of keys) if (num(p[k]) > 0) out[k] = num(p[k]);
      return out as T;
    });
  };
  const lsp = springProps<LineSpringProp>(raw?.line_spring_props, ["u1", "u2", "u3", "r1"]);
  if (lsp) m.line_spring_props = lsp;
  const asp = springProps<AreaSpringProp>(raw?.area_spring_props, ["u1", "u2", "u3"]);
  if (asp) m.area_spring_props = asp;
  if (Array.isArray(raw?.spectrum_functions) && raw.spectrum_functions.length) m.spectrum_functions = raw.spectrum_functions.map(readSpectrumFunction);
  if (Array.isArray(raw?.spectrum_cases) && raw.spectrum_cases.length) m.spectrum_cases = raw.spectrum_cases.map(readSpectrumCase);
  if (Array.isArray(raw?.not_run) && raw.not_run.length) m.not_run = raw.not_run.map(id);
  const pDelta = raw?.p_delta && typeof raw.p_delta === "object" ? cleanPDelta(readPDelta(raw.p_delta)) : undefined;
  if (pDelta) m.p_delta = pDelta;
  if (Array.isArray(raw?.groups) && raw.groups.length) m.groups = raw.groups.map(readGroup);
  if (Array.isArray(raw?.pier_labels) && raw.pier_labels.length) m.pier_labels = raw.pier_labels.map((l: Record<string, unknown>) => readWallLabel(l, "pier"));
  if (Array.isArray(raw?.spandrel_labels) && raw.spandrel_labels.length) m.spandrel_labels = raw.spandrel_labels.map((l: Record<string, unknown>) => readWallLabel(l, "spandrel"));
  if (Array.isArray(raw?.diaphragms)) {
    m.diaphragms = raw.diaphragms.map((d: { id: unknown; semi_rigid?: unknown }) => ({ id: id(d.id), ...(d.semi_rigid === true ? { semi_rigid: true } : {}) }));
  }
  if (Array.isArray(raw?.stories) && raw.stories.length) m.stories = raw.stories.map((s: { id: unknown; z: unknown }) => ({ id: id(s.id), z: num(s.z) }));
  // Older files: "off" (no meshing for the whole model) is read as every area set to no mesh.
  if (raw?.mesh === "off") for (const a of m.areas) if (a.section && !a.opening && !a.user_mesh) a.mesh = "none";
  if (raw?.mesh && typeof raw.mesh === "object") {
    const s: MeshSettings = {};
    if (raw.mesh.floor === "rectangular" || raw.mesh.floor === "general") s.floor = raw.mesh.floor;
    if (raw.mesh.floor_size !== undefined && raw.mesh.floor_size !== null) s.floor_size = num(raw.mesh.floor_size);
    if (raw.mesh.wall_size !== undefined && raw.mesh.wall_size !== null) s.wall_size = num(raw.mesh.wall_size);
    if (raw.mesh.refine === false) s.refine = false;
    m.mesh = s;
  }
  if (raw?.angle_limits && typeof raw.angle_limits === "object") {
    m.angle_limits = {
      column: num(raw.angle_limits.column ?? DEFAULT_ANGLE_LIMITS.column),
      beam_floor: num(raw.angle_limits.beam_floor ?? DEFAULT_ANGLE_LIMITS.beam_floor),
    };
  }
  if (raw?.gui && typeof raw.gui === "object") m.gui = clone(raw.gui);
  setMissingPropertyTypes(m);
  return m;
}

/**
 * Files saved before area sections had a property type: a section only walls use becomes a wall section,
 * every other one a slab section (the type changes no result, so a section used by both stays one section).
 */
function setMissingPropertyTypes(m: Model) {
  if (m.area_sections.every((s) => s.property_type)) return;
  const at = new Map(m.nodes.map((n) => [n.id, pos(n)]));
  const limit = (m.angle_limits ?? DEFAULT_ANGLE_LIMITS).beam_floor;
  const floors = new Map<Id, boolean>();
  for (const a of m.areas) {
    if (a.opening || !a.section) continue;
    const p = a.nodes.map((n) => at.get(n)).filter((x) => x !== undefined);
    if (p.length < 3) continue;
    floors.set(a.section, (floors.get(a.section) ?? false) || isFloor(p, limit));
  }
  for (const s of m.area_sections) if (!s.property_type) s.property_type = floors.get(s.id) === false ? "wall" : "slab";
}

/** The loads of a pattern (or of a case in files written before load patterns), cleaned. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function readLoads(c: any): Pick<LoadPattern, "node_loads" | "frame_loads" | "area_loads"> {
  const id = (v: unknown) => String(v ?? "");
  return {
    node_loads: (c.node_loads ?? []).map((l: NodeLoad) => ({ ...l, node: id(l.node) })),
    // Older files: `member_loads` with `element`.
    frame_loads: (c.frame_loads ?? c.member_loads ?? []).map((l: FrameLoad & { element?: Id }) => {
      const { element, ...rest } = l;
      return { ...rest, frame: id(l.frame ?? element) };
    }),
    area_loads: (c.area_loads ?? []).map((l: AreaLoad) => ({ area: id(l.area), dir: l.dir, q: Number(l.q ?? 0) })),
  };
}

/** The pattern type a file without patterns most likely meant for a case of this name. */
export function guessPatternType(name: string, selfWeight: number): PatternType {
  if (/dead/i.test(name) || selfWeight) return "dead";
  if (/live/i.test(name)) return "live";
  if (/wind/i.test(name)) return "wind";
  return "other";
}

/**
 * Load patterns and cases. A file written before load patterns keeps its loads on the load cases: each
 * case becomes a pattern of the same name (type from its name) and a case applying it once; the results
 * are the same.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function readPatternsAndCases(raw: any): Pick<Model, "load_patterns" | "load_cases"> {
  const id = (v: unknown) => String(v ?? "");
  const num = (v: unknown) => (typeof v === "number" ? v : Number(v ?? 0));
  const cases = raw?.load_cases ?? [];
  if (!Array.isArray(raw?.load_patterns)) {
    return {
      load_patterns: cases.map((c: { id: unknown; self_weight?: unknown }) => {
        const sw = num(c.self_weight ?? 0);
        return { id: id(c.id), type: guessPatternType(id(c.id), sw), ...(sw ? { self_weight: sw } : {}), ...readLoads(c) };
      }),
      load_cases: cases.map((c: { id: unknown }) => ({ id: id(c.id), loads: [{ pattern: id(c.id), scale: 1 }] })),
    };
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const load_patterns: LoadPattern[] = raw.load_patterns.map((p: any) => {
    const out: LoadPattern = { id: id(p.id), type: PATTERN_TYPES.includes(p.type) ? p.type : "other", ...readLoads(p) };
    if (p.self_weight) out.self_weight = num(p.self_weight);
    if (p.auto === "user-wind") out.auto = "user-wind";
    if (Array.isArray(p.wind_loads) && p.wind_loads.length) {
      out.wind_loads = p.wind_loads.map((w: WindLoad) => {
        const r: WindLoad = { story: id(w.story), diaphragm: id(w.diaphragm), x: num(w.x), y: num(w.y) };
        for (const k of ["fx", "fy", "mz"] as const) if (w[k]) r[k] = num(w[k]);
        return r;
      });
    }
    if (p.wind_point === "coordinates") out.wind_point = "coordinates";
    if (p.wind_semi_rigid === "mass" || p.wind_semi_rigid === "nearest-joint") out.wind_semi_rigid = p.wind_semi_rigid;
    if (p.reversible === true) out.reversible = true;
    return out;
  });
  const names = new Set(load_patterns.map((p) => p.id));
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const load_cases: LoadCase[] = cases.map((c: any) => {
    const loads = (c.loads ?? []).map((l: { pattern: unknown; scale?: unknown }) => ({ pattern: id(l.pattern), scale: num(l.scale ?? 1) }));
    // A case with loads of its own as well: they become a pattern of their own.
    const own = readLoads(c);
    if (own.node_loads.length || own.frame_loads.length || own.area_loads.length || c.self_weight) {
      let name = id(c.id);
      for (let k = 1; names.has(name); k++) name = `${id(c.id)}-loads${k > 1 ? k : ""}`;
      names.add(name);
      load_patterns.push({ id: name, type: "other", ...(c.self_weight ? { self_weight: num(c.self_weight) } : {}), ...own });
      loads.push({ pattern: name, scale: 1 });
    }
    return { id: id(c.id), loads };
  });
  return { load_patterns, load_cases };
}

/** A mass source as read from a file: files without patterns name load cases (pattern of the same name). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function readMassSource(raw: any): MassSource {
  return {
    ...raw,
    loads: (raw.loads ?? []).map((l: { pattern?: unknown; case?: unknown; scale?: unknown }) => ({ pattern: String(l.pattern ?? l.case ?? ""), scale: Number(l.scale ?? 1) })),
  };
}

/** P-Delta options as read from a file: its loads like the mass source's (load cases name patterns). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function readPDelta(raw: any): PDelta {
  const method = raw.method === "mass" || raw.method === "loads" ? raw.method : "none";
  return { method, loads: readMassSource(raw).loads };
}

export interface NewModelOptions {
  xSpacings: number[];
  ySpacings: number[];
  storyHeights: number[];
  title?: string;
  /** Display units of the new model (missing = SI); the model itself is always in kN, m. */
  units?: UnitSystem;
}

/**
 * An empty 3D model with a grid, steel and concrete, a frame section, a slab and a wall section and the
 * load patterns Dead and Live with their cases, ready to draw. (Plane-frame analysis is an option set later: see `setPlaneFrame`.)
 */
export function newModel(o: NewModelOptions): Model {
  const grid: Grid = makeGrid(o.xSpacings, o.ySpacings, o.storyHeights);
  return {
    version: MODEL_FORMAT,
    title: o.title ?? "",
    dimension: "3D",
    units: { force: "kN", length: "m" },
    materials: [
      // Default unit weights (490 and 150 lb/ft^3) in kN/m^3.
      // Strengths: Fy 345 MPa, Fu 450 MPa (a common structural steel); f'c 28 MPa (4000 psi).
      { id: "Steel", type: "steel", E: 200e6, nu: 0.3, weight: 76.9729, fy: 345e3, fu: 450e3 },
      { id: "Concrete", type: "concrete", E: 25e6, nu: 0.2, weight: 23.5631, fc: 28e3 },
    ],
    // A 300 x 300 mm square, so members can be drawn straight away.
    sections: [{ id: "FSEC1", ...rectangle(0.3, 0.3), depth: 0.3, width: 0.3 }],
    // Slab1 and Wall1, so areas can be drawn straight away.
    area_sections: [
      { id: "Slab1", type: "shell-thin", material: "Concrete", thickness: 0.2, property_type: "slab" },
      { id: "Deck1", type: "membrane", material: "Concrete", thickness: deckDepth(newDeck("Steel")), property_type: "deck", one_way: true, deck: newDeck("Steel") },
      { id: "Wall1", type: "shell-thin", material: "Concrete", thickness: 0.2, property_type: "wall" },
    ],
    areas: [],
    nodes: [],
    frames: [],
    supports: [],
    // Patterns Dead (self-weight multiplier 1) and Live, each with its own case.
    load_patterns: [
      { id: "Dead", type: "dead", self_weight: 1, node_loads: [], frame_loads: [], area_loads: [] },
      { id: "Live", type: "live", node_loads: [], frame_loads: [], area_loads: [] },
    ],
    load_cases: [
      { id: "Dead", loads: [{ pattern: "Dead", scale: 1 }] },
      { id: "Live", loads: [{ pattern: "Live", scale: 1 }] },
    ],
    load_combinations: [],
    springs: [],
    links: [],
    // Modal case (Eigen, 12 modes) with its default mass source.
    modal: { id: "Modal", modes: 12 },
    diaphragms: newDiaphragms(),
    gui: { grid, sectionShapes: { FSEC1: { shape: "rectangle", depth: 0.3, width: 0.3 } }, ...(o.units && o.units !== "SI" ? { units: o.units } : {}) },
  };
}

export const nodeById = (m: Model, id: Id) => m.nodes.find((n) => n.id === id);
export const frameById = (m: Model, id: Id) => m.frames.find((e) => e.id === id);

export function memberLength(m: Model, id: Id): number {
  const e = frameById(m, id);
  const a = e && nodeById(m, e.nodes[0]);
  const b = e && nodeById(m, e.nodes[1]);
  return a && b ? dist(pos(a), pos(b)) : 0;
}

/**
 * Frame loads keep their fraction of the length on frames whose length changed (`before`: lengths by
 * frame before the edit). Changes `m` in place: call it on the edit's own copy.
 */
export function keepLoadFractions(m: Model, before: Map<Id, number>) {
  const round = (v: number) => Math.round(v * 1e9) / 1e9;
  for (const p of m.load_patterns) {
    for (const l of p.frame_loads) {
      const [a, b] = [before.get(l.frame), memberLength(m, l.frame)];
      if (!a || !b || Math.abs(a - b) <= SAME_POINT) continue;
      const s = (v: number) => round((v * b) / a);
      if (l.type === "point") l.at = s(l.at);
      else {
        if (l.from !== undefined) l.from = s(l.from);
        if (l.to !== undefined) l.to = s(l.to);
      }
    }
  }
}

/**
 * Joins joints into others (from -> to): objects, loads, supports, springs and masses move to the kept
 * joint (a support, spring or mass only where the kept joint has none).
 */
export function mergeJoints(m: Model, into: Map<Id, Id>): Model {
  const to = (id: Id) => into.get(id) ?? id;
  const model = clone(m);
  for (const o of [...model.frames, ...model.links]) o.nodes = [to(o.nodes[0]), to(o.nodes[1])];
  for (const a of model.areas) a.nodes = a.nodes.map(to);
  for (const p of model.load_patterns) for (const l of p.node_loads) l.node = to(l.node);
  for (const key of ["supports", "springs", "masses"] as const) {
    const list = model[key] as { node: Id }[] | undefined;
    if (!list) continue;
    const has = new Set(list.filter((x) => !into.has(x.node)).map((x) => x.node));
    const out: { node: Id }[] = [];
    for (const x of list) {
      if (!into.has(x.node)) out.push(x);
      else if (!has.has(to(x.node))) {
        has.add(to(x.node));
        out.push({ ...x, node: to(x.node) });
      }
    }
    (model as unknown as Record<string, unknown>)[key] = out;
  }
  model.nodes = model.nodes.filter((n) => !into.has(n.id));
  return model;
}

export function linkLength(m: Model, id: Id): number {
  const l = m.links.find((x) => x.id === id);
  const a = l && nodeById(m, l.nodes[0]);
  const b = l && nodeById(m, l.nodes[1]);
  return a && b ? dist(pos(a), pos(b)) : 0;
}

export function findNodeAt(m: Model, p: Vec3): ModelNode | undefined {
  return m.nodes.find((n) => dist(pos(n), p) <= SAME_POINT);
}

/** Largest number used by ids like "12" or "C12" (prefix ""), plus one. */
export function nextNumber(ids: Id[], prefix: string): number {
  let max = 0;
  const re = new RegExp(`^${prefix}(\\d+)$`);
  for (const id of ids) {
    const m = re.exec(id);
    if (m) max = Math.max(max, parseInt(m[1], 10));
  }
  return max + 1;
}

/** The diaphragm new floors and story joints get: the model's first one (D1 in a new model). */
export const defaultDiaphragm = (m: Model): Id | undefined => m.diaphragms?.[0]?.id;

/** A new model's diaphragms: D1, semi-rigid (new floors and story joints get it; it ties nothing, make it rigid to tie them). */
export const newDiaphragms = (): Diaphragm[] => [{ id: "D1", semi_rigid: true }];

/** Whether a height is a story level of the grid above the base. */
export function aboveBaseLevel(m: Model, z: number): boolean {
  const stories = gridOf(m).stories;
  const base = Math.min(...stories.map((s) => s.z));
  return z > base + SAME_POINT && stories.some((s) => Math.abs(s.z - z) <= SAME_POINT);
}

/**
 * Adds a joint at p, or returns the joint already there. A new joint at a story level above the base
 * gets the default diaphragm; a joint drawn on its own (`free`: the Joint tool, Joint by Coordinates) is
 * special and included in the mesh.
 */
export function addNode(m: Model, p: Vec3, opts: { free?: boolean } = {}): { model: Model; id: Id; created: boolean } {
  const found = findNodeAt(m, p);
  if (found) return { model: m, id: found.id, created: false };
  const id = String(nextNumber(m.nodes.map((n) => n.id), ""));
  const model = clone(m);
  const c = p.map((v) => Math.round(v * 1e9) / 1e9) as Vec3;
  const node: ModelNode = { id, x: c[0], y: c[1], z: c[2] };
  if (opts.free) {
    node.in_mesh = true;
    node.special = true;
  }
  Object.assign(node, drawnNodeDefaults(m, c));
  model.nodes.push(node);
  return { model, id, created: true };
}

/** What a new joint at p gets: the default diaphragm at a story level above the base. */
export function drawnNodeDefaults(m: Model, p: Vec3): Pick<ModelNode, "diaphragm"> {
  const d = defaultDiaphragm(m);
  return d && aboveBaseLevel(m, p[2]) ? { diaphragm: d } : {};
}

/**
 * What a new frame with a section between a and b gets: automatic end offsets (rigid-zone factor 0) and the
 * drawn insertion point (a column at 5, a beam at 8; the drawing only). The example models get it too.
 */
export function drawnFrameDefaults(m: Model, a: Vec3, b: Vec3): Pick<Frame, "end_offsets" | "insertion"> {
  const ins = drawnFrameInsertion(m, a, b);
  return { end_offsets: "auto", ...(ins ? { insertion: ins } : {}) };
}

export interface FrameProps {
  type: FrameKind;
  material: Id;
  /** "" draws a null frame (section "None"). */
  section: Id;
}

/**
 * Adds a member between two joints. End i follows the end-order rule (whatever the click order); the
 * name is M plus a number (its label, C / B / D by its type, comes from labels.ts). Section "" draws a
 * null frame (no material, no stiffness).
 */
export function addFrame(m: Model, a: Id, b: Id, props: FrameProps): { model: Model; id: Id } {
  const na = nodeById(m, a);
  const nb = nodeById(m, b);
  if (!na || !nb) throw new EditError(t("err.unknownJoint"));
  if (a === b || dist(pos(na), pos(nb)) <= SAME_POINT) throw new EditError(t("err.memberSameJoint"));
  if (m.frames.some((e) => (e.nodes[0] === a && e.nodes[1] === b) || (e.nodes[0] === b && e.nodes[1] === a))) {
    throw new EditError(t("err.memberExists"));
  }
  const isNull = props.section === "";
  if (!isNull && !m.sections.some((s) => s.id === props.section)) throw new EditError(t("err.defineSection"));
  if (!isNull && !m.materials.some((s) => s.id === props.material)) throw new EditError(t("err.defineMaterial"));
  const nodes: [Id, Id] = drawOrderReversed(pos(na), pos(nb)) ? [b, a] : [a, b];
  const id = FRAME_NAME + nextNumber(m.frames.map((e) => e.id), FRAME_NAME);
  const model = clone(m);
  model.frames.push(
    isNull
      ? { id, type: "frame", nodes, material: "", section: "" }
      : {
          id,
          type: props.type,
          nodes,
          material: compositeMaterial(m, props.section) ?? props.material,
          section: props.section,
          ...drawnFrameDefaults(m, pos(na), pos(nb)),
        },
  );
  return { model, id };
}

/**
 * Two angle limits for object types (degrees; null = the defaults, 1 and 20). Types follow
 * at once; names stay. The floor/wall limit also decides floor or wall in the analysis.
 */
export function setAngleLimits(m: Model, limits: AngleLimits | null): Model {
  const model = clone(m);
  if (!limits || (limits.column === DEFAULT_ANGLE_LIMITS.column && limits.beam_floor === DEFAULT_ANGLE_LIMITS.beam_floor)) {
    delete model.angle_limits;
    return model;
  }
  const { column, beam_floor } = limits;
  if (!(column > 0 && beam_floor > 0 && column + beam_floor <= 90)) throw new EditError(t("err.angleLimits"));
  model.angle_limits = { column, beam_floor };
  return model;
}

/**
 * Auto-mesh settings of the model: settings (missing fields = the defaults) or null for the defaults (no
 * `mesh` in the file). Areas analysed as one element are set per area (*Assign > Area Mesh Options*).
 */
export function setMeshSettings(m: Model, mesh: MeshSettings | null): Model {
  if (mesh) {
    for (const v of [mesh.floor_size, mesh.wall_size]) if (v !== undefined && !(v > 0 && Number.isFinite(v))) throw new EditError(t("err.meshSize"));
  }
  const model = clone(m);
  if (mesh === null) delete model.mesh;
  else model.mesh = clone(mesh);
  return model;
}

/**
 * *Include in mesh* on joints (only matters for joints with nothing attached; undefined = the
 * default, no) and on frames (undefined = the default, yes; false = floors are not meshed along it).
 */
export function setInMesh(m: Model, nodes: Id[], frames: Id[], value: boolean | undefined): Model {
  const ns = new Set(nodes);
  const fs = new Set(frames);
  const model = clone(m);
  for (const n of model.nodes) {
    if (!ns.has(n.id)) continue;
    if (value === undefined || value === false) delete n.in_mesh;
    else n.in_mesh = true;
  }
  for (const f of model.frames) {
    if (!fs.has(f.id)) continue;
    if (value === undefined || value === true) delete f.in_mesh;
    else f.in_mesh = false;
  }
  return model;
}

/**
 * Special joint on or off for joints (Assign > Special Joint). Turned off, a joint with nothing attached
 * is deleted at once, with whatever is assigned to it.
 */
export function setSpecial(m: Model, nodes: Id[], value: boolean): Model {
  const ns = new Set(nodes);
  const model = clone(m);
  for (const n of model.nodes) {
    if (!ns.has(n.id)) continue;
    if (value) n.special = true;
    else delete n.special;
  }
  if (value) return model;
  const used = new Set([...m.frames, ...m.links, ...m.areas].flatMap((o) => o.nodes));
  const free = nodes.filter((id) => !used.has(id));
  return free.length ? deleteObjects(model, free, []) : model;
}

/**
 * Joints of the given frames, links and areas that nothing else uses and that are not special: an edit
 * that deletes or moves those objects away deletes these joints too, whatever is assigned to them.
 */
export function leftAlone(m: Model, frames: Set<Id>, links: Set<Id>, areas: Set<Id>): Id[] {
  const freed = new Set<Id>();
  const used = new Set<Id>();
  for (const e of m.frames) for (const n of e.nodes) (frames.has(e.id) ? freed : used).add(n);
  for (const l of m.links) for (const n of l.nodes) (links.has(l.id) ? freed : used).add(n);
  for (const a of m.areas) for (const n of a.nodes) (areas.has(a.id) ? freed : used).add(n);
  return m.nodes.filter((n) => freed.has(n.id) && !used.has(n.id) && !n.special).map((n) => n.id);
}

/**
 * Deletes joints, members, links and areas. A deleted joint takes its members, links, areas, support,
 * springs and loads with it; joints left with nothing attached go too unless special.
 */
export function deleteObjects(m: Model, nodes: Id[], frames: Id[], links: Id[] = [], areas: Id[] = []): Model {
  const nodeSet = new Set(nodes);
  const elemSet = new Set(frames);
  const linkSet = new Set(links);
  const areaSet = new Set(areas);
  for (const e of m.frames) if (nodeSet.has(e.nodes[0]) || nodeSet.has(e.nodes[1])) elemSet.add(e.id);
  for (const l of m.links) if (nodeSet.has(l.nodes[0]) || nodeSet.has(l.nodes[1])) linkSet.add(l.id);
  for (const a of m.areas) if (a.nodes.some((n) => nodeSet.has(n))) areaSet.add(a.id);
  for (const n of leftAlone(m, elemSet, linkSet, areaSet)) nodeSet.add(n);
  const model = clone(m);
  model.nodes = model.nodes.filter((n) => !nodeSet.has(n.id));
  model.frames = model.frames.filter((e) => !elemSet.has(e.id));
  model.links = model.links.filter((l) => !linkSet.has(l.id));
  model.areas = model.areas.filter((a) => !areaSet.has(a.id));
  model.supports = model.supports.filter((s) => !nodeSet.has(s.node));
  model.springs = model.springs.filter((s) => !nodeSet.has(s.node));
  if (model.masses) model.masses = model.masses.filter((s) => !nodeSet.has(s.node));
  for (const c of model.load_patterns) {
    c.node_loads = c.node_loads.filter((l) => !nodeSet.has(l.node));
    c.frame_loads = c.frame_loads.filter((l) => !elemSet.has(l.frame));
    c.area_loads = c.area_loads.filter((l) => !areaSet.has(l.area));
  }
  pruneGroups(model);
  return model;
}

/** Restrained DOFs of a joint, in the order ux, uy, uz, rx, ry, rz. */
export function restraintsOf(m: Model, node: Id): boolean[] {
  const s = m.supports.find((x) => x.node === node);
  if (!s) return [false, false, false, false, false, false];
  if (s.fix === "fixed") return [true, true, true, true, true, true];
  if (s.fix === "pinned") return [true, true, true, false, false, false];
  return DOFS.map((d) => (s.fix as Dof[]).includes(d));
}

export function fixFromFlags(flags: boolean[]): Fix | null {
  if (flags.every((f) => !f)) return null;
  if (flags.every((f) => f)) return "fixed";
  if (flags.slice(0, 3).every((f) => f) && flags.slice(3).every((f) => !f)) return "pinned";
  return DOFS.filter((_, i) => flags[i]);
}

/** Sets the restraints of joints (all false = no support). */
export function setRestraints(m: Model, nodes: Id[], flags: boolean[]): Model {
  const fix = fixFromFlags(flags);
  const set = new Set(nodes);
  const model = clone(m);
  model.supports = model.supports.filter((s) => !set.has(s.node));
  if (fix) for (const n of nodes) model.supports.push({ node: n, fix: Array.isArray(fix) ? [...fix] : fix });
  checkBodyRestraints(model);
  return model;
}

/** Spring stiffness of a joint, in the order ux, uy, uz, rx, ry, rz (0 = none). */
export function springsOf(m: Model, node: Id): number[] {
  const s = m.springs.find((x) => x.node === node);
  return DOFS.map((d) => s?.[d] ?? 0);
}

/**
 * Joint springs (global axes): "add" sums into the joints'
 * springs, "replace" sets them, "delete" removes them.
 */
export function setSprings(m: Model, nodes: Id[], values: number[], mode: LoadMode): Model {
  if (mode !== "delete" && !values.every((v) => Number.isFinite(v) && v >= 0)) {
    throw new EditError(t("err.springsNegative"));
  }
  const model = clone(m);
  for (const n of nodes) {
    const base = mode === "add" ? springsOf(model, n) : DOFS.map(() => 0);
    const next = mode === "delete" ? base : base.map((v, i) => v + values[i]);
    const i = model.springs.findIndex((x) => x.node === n);
    const spring: Spring = { node: n };
    DOFS.forEach((d, k) => {
      if (next[k]) spring[d] = next[k];
    });
    const keep = next.some((v) => v > 0);
    if (i >= 0 && keep) model.springs[i] = spring;
    else if (i >= 0) model.springs.splice(i, 1);
    else if (keep) model.springs.push(spring);
  }
  return model;
}

/** Added mass of a joint, in the order ux, uy, uz, rx, ry, rz (0 = none). */
export function massesOf(m: Model, node: Id): number[] {
  const s = m.masses?.find((x) => x.node === node);
  return DOFS.map((d) => s?.[d] ?? 0);
}

/** Added joint masses: "add" sums, "replace" sets, "delete" removes. */
export function setJointMasses(m: Model, nodes: Id[], values: number[], mode: LoadMode): Model {
  if (mode !== "delete" && !values.every((v) => Number.isFinite(v) && v >= 0)) throw new EditError(t("err.massNegative"));
  const model = clone(m);
  const list = [...(model.masses ?? [])];
  for (const n of nodes) {
    const base = mode === "add" ? massesOf(model, n) : DOFS.map(() => 0);
    const next = mode === "delete" ? base.map(() => 0) : base.map((v, i) => v + values[i]);
    const i = list.findIndex((x) => x.node === n);
    const jm: JointMass = { node: n };
    DOFS.forEach((d, k) => {
      if (next[k]) jm[d] = next[k];
    });
    const keep = next.some((v) => v > 0);
    if (i >= 0 && keep) list[i] = jm;
    else if (i >= 0) list.splice(i, 1);
    else if (keep) list.push(jm);
  }
  if (list.length) model.masses = list;
  else delete model.masses;
  return model;
}

/** A mass source with only the fields that differ from the defaults. */
export function cleanMassSource(raw: MassSource): MassSource | undefined {
  const out: MassSource = {};
  for (const k of ["elements", "joint_masses", "lateral_only", "lump_at_stories"] as const) if (raw[k] === false) out[k] = false;
  const loads = (raw.loads ?? []).filter((l) => l && l.pattern).map((l) => ({ pattern: String(l.pattern), scale: Number(l.scale ?? 1) }));
  if (loads.length) out.loads = loads;
  return Object.keys(out).length ? out : undefined;
}

/** P-Delta options as the model keeps them: none is left out, the mass method has no loads. */
export function cleanPDelta(raw: PDelta): PDelta | undefined {
  if (raw.method === "mass") return { method: "mass" };
  if (raw.method !== "loads") return undefined;
  const loads = (raw.loads ?? []).filter((l) => l && l.pattern).map((l) => ({ pattern: String(l.pattern), scale: Number(l.scale ?? 1) }));
  return { method: "loads", loads };
}

/** Define > P-Delta Options: the method and, for the iterative one, its load (at least one pattern). */
export function setPDelta(m: Model, pd: PDelta): Model {
  if (pd.method === "loads") {
    if (!pd.loads?.length) throw new EditError(t("err.pDeltaNoLoads"));
    for (const l of pd.loads) {
      if (!m.load_patterns.some((c) => c.id === l.pattern)) throw new EditError(t("err.massPattern", { id: l.pattern }));
      if (!Number.isFinite(l.scale)) throw new EditError(t("err.pDeltaScale"));
    }
  }
  const model = clone(m);
  const clean = cleanPDelta(pd);
  if (clean) model.p_delta = clean;
  else delete model.p_delta;
  return model;
}

/** A modal case from a file: Eigen, or Ritz with its starting loads. */
function readModal(raw: Record<string, unknown>): ModalCase {
  const id = (v: unknown) => String(v ?? "");
  const num = (v: unknown) => (typeof v === "number" ? v : Number(v ?? 0));
  const out: ModalCase = { id: id(raw.id ?? "Modal"), modes: Math.max(1, Math.round(num(raw.modes ?? 12))) };
  if (raw.type === "ritz") {
    out.type = "ritz";
    out.starts = (Array.isArray(raw.starts) ? raw.starts : []).map((s: Record<string, unknown>) => {
      const accel = RITZ_ACCELS.find((a) => a === s.accel);
      const st: RitzStart = accel ? { accel } : { pattern: id(s.pattern) };
      const c = Math.round(num(s.cycles ?? 0));
      if (c > 0) st.cycles = c;
      return st;
    });
  }
  return out;
}

/** Define > Mass Source and Modal Case: the mass source, and the modal case (null = none). */
export function setMassSource(m: Model, source: MassSource, modal: ModalCase | null): Model {
  for (const l of source.loads ?? []) {
    if (!m.load_patterns.some((c) => c.id === l.pattern)) throw new EditError(t("err.massPattern", { id: l.pattern }));
    if (!Number.isFinite(l.scale)) throw new EditError(t("err.massScale"));
  }
  if (modal && !(Number.isInteger(modal.modes) && modal.modes >= 1)) throw new EditError(t("err.modes"));
  if (modal && caseAndComboIds(m).includes(modal.id)) throw new EditError(t("err.modalName", { id: modal.id }));
  if (modal?.type === "ritz") {
    if (!modal.starts?.length) throw new EditError(t("err.ritzStarts"));
    for (const s of modal.starts) {
      if (s.pattern !== undefined && !m.load_patterns.some((p) => p.id === s.pattern)) throw new EditError(t("err.massPattern", { id: s.pattern }));
      if (s.cycles !== undefined && !(Number.isInteger(s.cycles) && s.cycles >= 0)) throw new EditError(t("err.ritzCycles"));
    }
  }
  if (!modal && m.spectrum_cases?.length) throw new EditError(t("err.modalForSpectrum", { list: m.spectrum_cases.map((c) => c.id).join(", ") }));
  const model = clone(m);
  const src = cleanMassSource(source);
  if (src) model.mass_source = src;
  else delete model.mass_source;
  if (modal) {
    model.modal = { id: modal.id.trim() || "Modal", modes: modal.modes };
    if (modal.type === "ritz") {
      model.modal.type = "ritz";
      model.modal.starts = modal.starts!.map((s) => ({ ...(s.accel ? { accel: s.accel } : { pattern: s.pattern }), ...(s.cycles ? { cycles: s.cycles } : {}) }));
    }
  }
  else delete model.modal;
  if (m.modal && m.modal.id !== model.modal?.id) renameNotRun(model, m.modal.id, model.modal?.id ?? null);
  return model;
}

/** Joints joined by rigid links, one group per rigid body (only joints that have links). */
export function rigidBodies(m: Model): Id[][] {
  const root = new Map<Id, Id>();
  const find = (x: Id): Id => {
    let r = x;
    while (root.get(r) !== r) r = root.get(r)!;
    return r;
  };
  for (const l of m.links) {
    for (const n of l.nodes) if (!root.has(n)) root.set(n, n);
    root.set(find(l.nodes[0]), find(l.nodes[1]));
  }
  const groups = new Map<Id, Id[]>();
  for (const n of m.nodes) {
    if (!root.has(n.id)) continue;
    const r = find(n.id);
    groups.set(r, [...(groups.get(r) ?? []), n.id]);
  }
  return [...groups.values()];
}

/** Joints of one rigid body that both have restraints (the solver allows one), or null. */
export function bodyRestraintConflict(m: Model): [Id, Id] | null {
  for (const g of rigidBodies(m)) {
    const held = g.filter((n) => restraintsOf(m, n).some(Boolean));
    if (held.length > 1) return [held[0], held[1]];
  }
  return null;
}

function checkBodyRestraints(m: Model) {
  const c = bodyRestraintConflict(m);
  if (c) {
    throw new EditError(t("err.bodyConflict", { a: c[0], b: c[1] }));
  }
}

/**
 * Adds a rigid link between two joints (end order, names K1, K2…). The joints then move as one
 * rigid body.
 */
export function addLink(m: Model, a: Id, b: Id): { model: Model; id: Id } {
  const na = nodeById(m, a);
  const nb = nodeById(m, b);
  if (!na || !nb) throw new EditError(t("err.unknownJoint"));
  if (a === b || dist(pos(na), pos(nb)) <= SAME_POINT) throw new EditError(t("err.linkSamePoint"));
  if (rigidBodies(m).some((g) => g.includes(a) && g.includes(b))) {
    throw new EditError(t("err.linkLoop", { a, b }));
  }
  const nodes: [Id, Id] = drawOrderReversed(pos(na), pos(nb)) ? [b, a] : [a, b];
  const id = LINK_NAME + nextNumber([...m.links.map((l) => l.id), ...m.frames.map((e) => e.id)], LINK_NAME);
  const model = clone(m);
  model.links.push({ id, type: "rigid", nodes });
  checkBodyRestraints(model);
  return { model, id };
}

export function assignFrame(
  m: Model,
  frames: Id[],
  patch: Partial<Pick<Frame, "section" | "material" | "type" | "angle">>,
): Model {
  const set = new Set(frames);
  const model = clone(m);
  for (const e of model.frames) {
    if (!set.has(e.id)) continue;
    if (patch.section !== undefined) e.section = patch.section;
    if (patch.material !== undefined) e.material = patch.material;
    // A composite section is transformed to its steel: its frames have that material.
    const steel = compositeMaterial(model, e.section);
    if (steel) e.material = steel;
    // A null frame (section "") has no material; a frame given a section needs one.
    if (patch.section === "") e.material = "";
    else if (e.section && !e.material) throw new EditError(t("err.frameMaterial", { id: e.id }));
    if (patch.type !== undefined) e.type = patch.type;
    if (e.type === "truss") delete e.releases; // a truss is pinned already
    if (patch.angle !== undefined) {
      if (patch.angle) e.angle = patch.angle;
      else delete e.angle;
    }
  }
  return model;
}

/** Refuses modifier values the solver would refuse (stiffness factors > 0, mass and weight >= 0). */
export function checkModifiers(kind: "frame" | "area", mods: FrameModifiers | AreaModifiers | undefined) {
  const bad = modifierProblem(kind, mods);
  if (bad) throw new EditError(t(bad.zeroAllowed ? "err.modifierZero" : "err.modifierPositive", { name: bad.name }));
}

/**
 * Sets property modifiers of frame objects (Assign > Frame > Property Modifiers): the given values
 * replace theirs, undefined ones are kept. They multiply the section's modifiers in the analysis.
 */
export function setFrameModifiers(m: Model, frames: Id[], values: Partial<Record<FrameModifier, number | undefined>>): Model {
  const set = new Set(frames);
  const model = clone(m);
  for (const e of model.frames) {
    if (!set.has(e.id)) continue;
    const mods = withModifiers(FRAME_MODIFIERS, e.modifiers, values);
    checkModifiers("frame", mods);
    if (mods) e.modifiers = mods;
    else delete e.modifiers;
  }
  return model;
}

/** Releases in P V2 V3 T M2 M3 order without repeats, or undefined when there are none. */
export function cleanReleases(raw: Releases | undefined): Releases | undefined {
  const out: Releases = {};
  for (const end of ["i", "j"] as const) {
    const list = RELEASES.filter((r) => raw?.[end]?.includes(r));
    if (list.length) out[end] = list;
  }
  return out.i || out.j ? out : undefined;
}

/** Why a set of end releases is refused (the frame would be a mechanism), or null. Same rule as the solver. */
export function releaseProblem(rel: Releases | undefined): string | null {
  const at = (end: "i" | "j", r: Release) => !!rel?.[end]?.includes(r);
  const both = (r: Release) => at("i", r) && at("j", r);
  for (const r of ["P", "V2", "V3", "T"] as const) if (both(r)) return t("err.releaseBoth", { r });
  for (const [mom, shear] of [
    ["M3", "V2"],
    ["M2", "V3"],
  ] as const) {
    if (both(mom) && (at("i", shear) || at("j", shear))) return t("err.releaseShear", { m: mom, v: shear });
  }
  return null;
}

/**
 * Sets end length offsets and rigid-zone factor of frames (Assign > Frame End Length Offsets):
 * `offsets` "auto", given lengths or null (none). Given lengths must leave some clear length.
 */
export function setFrameOffsets(m: Model, frames: Id[], offsets: EndOffsets | null, rigidZone: number): Model {
  if (!(rigidZone >= 0 && rigidZone <= 1)) throw new EditError(t("err.rigidZone"));
  if (offsets && offsets !== "auto" && !(offsets.i >= 0 && offsets.j >= 0)) throw new EditError(t("err.offsetNegative"));
  const set = new Set(frames);
  const model = clone(m);
  for (const e of model.frames) {
    if (!set.has(e.id)) continue;
    if (offsets && offsets !== "auto") {
      const [a, b] = e.nodes.map((n) => nodeById(model, n)!);
      if (offsets.i + offsets.j >= dist(pos(a), pos(b))) throw new EditError(t("err.offsetLong", { id: e.id }));
    }
    if (offsets) e.end_offsets = structuredClone(offsets);
    else delete e.end_offsets;
    if (offsets && rigidZone) e.rigid_zone = rigidZone;
    else delete e.rigid_zone;
  }
  return model;
}

/** Sets end releases of frames (Assign > Frame Releases), replacing theirs. Trusses take none. */
export function setFrameReleases(m: Model, frames: Id[], releases: Releases): Model {
  const rel = cleanReleases(releases);
  const problem = releaseProblem(rel);
  if (problem) throw new EditError(problem);
  const set = new Set(frames);
  const model = clone(m);
  for (const e of model.frames) {
    if (!set.has(e.id)) continue;
    if (rel && e.type === "truss") throw new EditError(t("err.releaseTruss", { id: e.id }));
    if (rel) e.releases = structuredClone(rel);
    else delete e.releases;
  }
  return model;
}

export type LoadMode = "add" | "replace" | "delete";

export function patternOf(model: Model, patternId: Id): LoadPattern {
  const c = model.load_patterns.find((x) => x.id === patternId);
  if (!c) throw new EditError(t("err.unknownPattern", { id: patternId }));
  return c;
}

/** Joint forces and moments (global axes). "add" sums into existing loads of the pattern. */
export function setNodeLoads(
  m: Model,
  patternId: Id,
  nodes: Id[],
  values: Partial<Record<ForceName, number>>,
  mode: LoadMode,
): Model {
  const model = clone(m);
  const c = patternOf(model, patternId);
  const set = new Set(nodes);
  if (mode !== "add") c.node_loads = c.node_loads.filter((l) => !set.has(l.node));
  if (mode === "delete") return model;
  for (const n of nodes) {
    let load = c.node_loads.find((l) => l.node === n);
    if (!load) {
      load = { node: n };
      c.node_loads.push(load);
    }
    for (const f of FORCES) {
      const v = (load[f] ?? 0) + (values[f] ?? 0);
      if (v !== 0) load[f] = v;
      else delete load[f];
    }
    if (FORCES.every((f) => !load![f])) c.node_loads = c.node_loads.filter((l) => l !== load);
  }
  return model;
}

/** A frame load without its frame (the same load is put on every selected frame). */
export type FrameLoadSpec =
  | { type: "uniform"; dir: LoadDir; w: number; from?: number; to?: number }
  | { type: "trapezoidal"; dir: LoadDir; w1: number; w2: number; from?: number; to?: number }
  | { type: "point"; dir: LoadDir; p: number; at: number };

/**
 * Member loads of one type (uniform, trapezoidal or point). "replace" and "delete" act on loads of the
 * same type on the same members in the same pattern. `relative` positions are fractions of the
 * length. A trapezoidal load varies linearly from w1 at `from` to w2 at `to` (one stretch of a trapezoidal load).
 */
export function setFrameLoads(
  m: Model,
  patternId: Id,
  frames: Id[],
  type: FrameLoadSpec["type"],
  spec: FrameLoadSpec | null,
  mode: LoadMode,
  relative = false,
): Model {
  const model = clone(m);
  const c = patternOf(model, patternId);
  const set = new Set(frames);
  if (mode !== "add") c.frame_loads = c.frame_loads.filter((l) => !(set.has(l.frame) && l.type === type));
  if (mode === "delete" || !spec) return model;
  for (const id of frames) {
    const e = frameById(model, id);
    if (!e) continue;
    const l = memberLength(model, id);
    const at = (v: number) => (relative ? v * l : v);
    let load: FrameLoad;
    if (spec.type === "uniform" || spec.type === "trapezoidal") {
      load = spec.type === "uniform" ? { type: "uniform", frame: id, dir: spec.dir, w: spec.w } : { type: "trapezoidal", frame: id, dir: spec.dir, w1: spec.w1, w2: spec.w2 };
      const from = spec.from === undefined ? 0 : at(spec.from);
      const to = spec.to === undefined ? l : at(spec.to);
      if (!(from >= 0 && to <= l * (1 + 1e-9) && from < to)) {
        throw new EditError(t("err.loadRange", { id, length: fmtLen(l) }));
      }
      if (from > 1e-12) load.from = from;
      if (to < l * (1 - 1e-12)) load.to = to;
    } else {
      const x = at(spec.at);
      if (!(x >= 0 && x <= l * (1 + 1e-9))) throw new EditError(t("err.loadPoint", { id, length: fmtLen(l) }));
      load = { type: "point", frame: id, dir: spec.dir, p: spec.p, at: Math.min(x, l) };
    }
    if (e.type === "truss") throw new EditError(t("err.trussLoad", { id }));
    c.frame_loads.push(load);
  }
  return model;
}

const fmtLen = (l: number) => String(Math.round(l * 1e6) / 1e6);

/** Adds or edits a material. Renaming updates the members that use it. */
export function upsertMaterial(m: Model, oldId: Id | null, mat: Material): Model {
  checkNewId(m.materials.map((x) => x.id), oldId, mat.id, "material");
  if (!(mat.E > 0)) throw new EditError(t("err.EPositive"));
  if (!((mat.G ?? 0) > 0 || (mat.nu !== undefined && mat.nu > -1 && mat.nu < 0.5))) {
    throw new EditError(t("err.GorNu"));
  }
  if (mat.weight !== undefined && !(Number.isFinite(mat.weight) && mat.weight >= 0)) throw new EditError(t("err.weightNegative"));
  const type = mat.type ?? guessMaterialType(mat.E);
  for (const k of STRENGTHS[type]) if (mat[k] !== undefined && !(Number.isFinite(mat[k]) && mat[k]! > 0)) throw new EditError(t("err.strengthPositive"));
  const model = clone(m);
  // Only the strengths of its type are kept.
  const clean: Material = { id: mat.id, type, E: mat.E, ...(mat.G !== undefined ? { G: mat.G } : {}), ...(mat.nu !== undefined ? { nu: mat.nu } : {}), ...(mat.weight ? { weight: mat.weight } : {}) };
  for (const k of STRENGTHS[type]) if (mat[k] !== undefined) clean[k] = mat[k];
  const i = oldId === null ? -1 : model.materials.findIndex((x) => x.id === oldId);
  if (i >= 0) model.materials[i] = clean;
  else model.materials.push(clean);
  if (oldId !== null && oldId !== mat.id) {
    for (const e of model.frames) if (e.material === oldId) e.material = mat.id;
    for (const a of model.area_sections) {
      if (a.material === oldId) a.material = mat.id;
      if (a.deck?.steel === oldId) a.deck.steel = mat.id;
    }
  }
  return model;
}

export function deleteMaterial(m: Model, id: Id): Model {
  const users = m.frames.filter((e) => e.material === id).length;
  if (users) throw new EditError(t("err.materialUsed", { id, n: users }));
  const areaUsers = m.area_sections.filter((a) => a.material === id || a.deck?.steel === id).length;
  if (areaUsers) throw new EditError(t("err.materialUsedAreas", { id, n: areaUsers }));
  const model = clone(m);
  model.materials = model.materials.filter((x) => x.id !== id);
  return model;
}

/** The steel material a section's composite shape is transformed to (its frames take it), if any. */
export function compositeMaterial(m: Model, sectionId: Id): Id | undefined {
  return m.sections.find((s) => s.id === sectionId)?.shape?.material;
}

/**
 * Adds or edits a frame section. Renaming updates the members that use it. `shape` records the
 * dimensions it was made from: a shape of `sectionShapes.ts` (its numbers in `sec`, from the solver),
 * an older app-only rectangle or I record, null for general properties, undefined to keep what is
 * recorded. Frames with a composite section take its steel material.
 */
export function upsertSection(m: Model, oldId: Id | null, sec: Section, shape?: SectionShape | ShapeSpec | null): Model {
  checkNewId(m.sections.map((x) => x.id), oldId, sec.id, "section");
  for (const k of ["A", "I22", "I33", "J"] as const) {
    if (!(sec[k] > 0)) throw new EditError(t("err.propPositive", { prop: k }));
  }
  if ((sec.As2 ?? 0) < 0 || (sec.As3 ?? 0) < 0) throw new EditError(t("err.shearNegative"));
  const model = clone(m);
  const clean: Section = { id: sec.id, A: sec.A, I22: sec.I22, I33: sec.I33, J: sec.J };
  if (sec.As2) clean.As2 = sec.As2;
  if (sec.As3) clean.As3 = sec.As3;
  const mods = readModifiers(FRAME_MODIFIERS, sec.modifiers);
  checkModifiers("frame", mods);
  if (mods) clean.modifiers = mods;
  const spec: ShapeSpec | undefined =
    shape && "type" in shape ? shape : shape === undefined && oldId !== null ? m.sections.find((x) => x.id === oldId)?.shape : undefined;
  if (spec) {
    clean.shape = clone(spec);
    if (sec.I23) clean.I23 = sec.I23;
    if (sec.centroid_offset && (sec.centroid_offset[0] || sec.centroid_offset[1])) clean.centroid_offset = [...sec.centroid_offset];
    if (sec.self_weight) clean.self_weight = sec.self_weight;
    if (sec.depth && sec.depth > 0) clean.depth = sec.depth;
    if (sec.width && sec.width > 0) clean.width = sec.width;
    for (const k of ["material", "fill", "encasement"] as const) {
      const id = spec[k];
      if (id !== undefined && !m.materials.some((x) => x.id === id)) throw new EditError(t("err.unknownMaterial", { id }));
    }
    const i = oldId === null ? -1 : model.sections.findIndex((x) => x.id === oldId);
    if (i >= 0) model.sections[i] = clean;
    else model.sections.push(clean);
    for (const e of model.frames) {
      if (oldId !== null && oldId !== sec.id && e.section === oldId) e.section = sec.id;
      if (e.section === sec.id && spec.material) e.material = spec.material;
    }
    if (model.gui?.sectionShapes) {
      const shapes = { ...model.gui.sectionShapes };
      if (oldId !== null) delete shapes[oldId];
      delete shapes[sec.id];
      model.gui = { ...model.gui, sectionShapes: shapes };
    }
    return model;
  }
  // Depth and width (automatic end offsets): from the shape, else as given.
  const legacy = shape && !("type" in shape) ? shape : shape;
  const recorded = shape === undefined && oldId !== null ? m.gui?.sectionShapes?.[oldId] : ((legacy as SectionShape | null) ?? undefined);
  const [depth, width] = recorded
    ? [recorded.depth, recorded.shape === "rectangle" ? recorded.width : recorded.flangeWidth]
    : [sec.depth, sec.width];
  if (depth && depth > 0) clean.depth = depth;
  if (width && width > 0) clean.width = width;
  const i = oldId === null ? -1 : model.sections.findIndex((x) => x.id === oldId);
  if (i >= 0) model.sections[i] = clean;
  else model.sections.push(clean);
  if (oldId !== null && oldId !== sec.id) for (const e of model.frames) if (e.section === oldId) e.section = sec.id;
  const shapes = { ...(model.gui?.sectionShapes ?? {}) };
  const kept = shape === undefined && oldId !== null ? shapes[oldId] : ((shape as SectionShape | null) ?? undefined);
  if (oldId !== null) delete shapes[oldId];
  if (kept) shapes[sec.id] = clone(kept);
  model.gui = { ...(model.gui ?? {}), sectionShapes: shapes };
  return model;
}

export function deleteSection(m: Model, id: Id): Model {
  const users = m.frames.filter((e) => e.section === id).length;
  if (users) throw new EditError(t("err.sectionUsed", { id, n: users }));
  const model = clone(m);
  model.sections = model.sections.filter((x) => x.id !== id);
  if (model.gui?.sectionShapes) delete model.gui.sectionShapes[id];
  return model;
}

/** Names of load cases and combinations (they share one name space). */
/** Names of load cases (linear static and response spectrum) and combinations: one list, no repeats. */
export const caseAndComboIds = (m: Model) => [...m.load_cases.map((x) => x.id), ...(m.spectrum_cases ?? []).map((x) => x.id), ...m.load_combinations.map((x) => x.id)];

/** A load pattern's settings (Define > Load Patterns). */
export interface PatternSpec {
  id: Id;
  type: PatternType;
  self_weight?: number;
}

/** The case that belongs to a pattern: the same name, applying only that pattern once. */
const ownCase = (m: Model, pattern: Id) =>
  m.load_cases.find((c) => c.id === pattern && c.loads.length === 1 && c.loads[0].pattern === pattern && c.loads[0].scale === 1);

/**
 * Adds a load pattern (with a load case of the same name applying it, when that name is free) or edits
 * one. Renaming follows into the cases and the mass source, and renames the pattern's own case too when
 * the new name is free. A pattern that is no longer of type Wind loses its user wind loads.
 */
export function upsertLoadPattern(m: Model, oldId: Id | null, spec: PatternSpec): Model {
  const id = spec.id.trim();
  checkNewId(m.load_patterns.map((x) => x.id), oldId, id, "pattern");
  if (!PATTERN_TYPES.includes(spec.type)) throw new EditError(t("err.patternType"));
  if (spec.self_weight !== undefined && !Number.isFinite(spec.self_weight)) throw new EditError(t("err.selfWeightNumber"));
  const model = clone(m);
  let p = oldId === null ? undefined : model.load_patterns.find((x) => x.id === oldId);
  const free = (name: Id) => !caseAndComboIds(model).includes(name) && model.modal?.id !== name;
  if (p) {
    if (oldId !== id) {
      const own = ownCase(model, oldId!);
      for (const c of model.load_cases) for (const l of c.loads) if (l.pattern === oldId) l.pattern = id;
      for (const l of model.mass_source?.loads ?? []) if (l.pattern === oldId) l.pattern = id;
      for (const l of model.p_delta?.loads ?? []) if (l.pattern === oldId) l.pattern = id;
      for (const s of model.modal?.starts ?? []) if (s.pattern === oldId) s.pattern = id;
      if (own && free(id)) {
        own.id = id;
        for (const k of model.load_combinations) for (const it of k.items) if (it.case === oldId) it.case = id;
      }
      p.id = id;
    }
  } else {
    p = { id, type: spec.type, node_loads: [], frame_loads: [], area_loads: [] };
    model.load_patterns.push(p);
    if (free(id)) model.load_cases.push({ id, loads: [{ pattern: id, scale: 1 }] });
  }
  p.type = spec.type;
  if (spec.self_weight !== undefined) {
    if (spec.self_weight) p.self_weight = spec.self_weight;
    else delete p.self_weight;
  }
  if (p.type !== "wind") {
    delete p.auto;
    delete p.wind_loads;
    delete p.wind_point;
    delete p.wind_semi_rigid;
    delete p.reversible;
  }
  return model;
}

/**
 * Deletes a load pattern and its own case. Refused while another case applies it, the mass source takes
 * it, or a combination uses its own case.
 */
export function deleteLoadPattern(m: Model, id: Id): Model {
  const own = ownCase(m, id);
  const users = m.load_cases.filter((c) => c !== own && c.loads.some((l) => l.pattern === id)).map((c) => c.id);
  if (users.length) throw new EditError(t("err.patternInCase", { id, list: users.join(", ") }));
  if (m.mass_source?.loads?.some((l) => l.pattern === id)) throw new EditError(t("err.patternInMass", { id }));
  if (m.p_delta?.loads?.some((l) => l.pattern === id)) throw new EditError(t("err.patternInPDelta", { id }));
  if (m.modal?.starts?.some((s) => s.pattern === id)) throw new EditError(t("err.patternInRitz", { id }));
  const combos = own ? combosUsing(m, "case", own.id) : [];
  if (combos.length) throw new EditError(t("err.caseInCombo", { id, list: combos.join(", ") }));
  const model = clone(m);
  model.load_patterns = model.load_patterns.filter((x) => x.id !== id);
  if (own) model.load_cases = model.load_cases.filter((x) => x.id !== own.id);
  return model;
}

/** Adds or edits a linear static load case: patterns times scale factors. Renaming follows into the combinations. */
export function setLoadCase(m: Model, oldId: Id | null, c: LoadCase): Model {
  const id = c.id.trim();
  checkNewId(caseAndComboIds(m), oldId, id, "case");
  if (m.modal?.id === id) throw new EditError(t("err.modalName", { id }));
  for (const l of c.loads) {
    if (!m.load_patterns.some((p) => p.id === l.pattern)) throw new EditError(t("err.unknownPattern", { id: l.pattern }));
    if (!Number.isFinite(l.scale)) throw new EditError(t("err.caseScale"));
  }
  const model = clone(m);
  const clean: LoadCase = { id, loads: c.loads.map((l) => ({ pattern: l.pattern, scale: l.scale })) };
  const i = oldId === null ? -1 : model.load_cases.findIndex((x) => x.id === oldId);
  if (i >= 0) model.load_cases[i] = clean;
  else model.load_cases.push(clean);
  if (oldId !== null && oldId !== id) for (const k of model.load_combinations) for (const it of k.items) if (it.case === oldId) it.case = id;
  if (oldId !== null && oldId !== id) renameNotRun(model, oldId, id);
  return model;
}

/** Where user wind loads act and how they reach a semi-rigid diaphragm (absent = the defaults). */
export interface WindSettings {
  wind_point?: "coordinates";
  wind_semi_rigid?: "mass" | "nearest-joint";
}

/**
 * The user wind loads of a Wind pattern (story forces Fx, Fy, Mz per story and diaphragm, at its centre
 * of mass or at (x, y)) and their settings; an empty list removes them.
 */
export function setWindLoads(m: Model, patternId: Id, rows: WindLoad[], settings: WindSettings = {}): Model {
  const model = clone(m);
  const p = patternOf(model, patternId);
  if (p.type !== "wind") throw new EditError(t("err.windNotWind", { id: patternId }));
  const stories = new Set((withStories(m).stories ?? []).map((s) => s.id));
  const dias = new Set((m.diaphragms ?? []).map((d) => d.id));
  const clean: WindLoad[] = [];
  for (const r of rows) {
    if (!stories.has(r.story)) throw new EditError(t("err.windStory", { id: r.story }));
    if (!dias.has(r.diaphragm)) throw new EditError(t("err.diaphragmUnknown", { id: r.diaphragm }));
    const vals = [r.fx ?? 0, r.fy ?? 0, r.mz ?? 0, r.x, r.y];
    if (!vals.every(Number.isFinite)) throw new EditError(t("err.windNumbers"));
    const w: WindLoad = { story: r.story, diaphragm: r.diaphragm, x: r.x, y: r.y };
    for (const k of ["fx", "fy", "mz"] as const) if (r[k]) w[k] = r[k];
    clean.push(w);
  }
  delete p.wind_point;
  delete p.wind_semi_rigid;
  if (clean.length) {
    p.auto = "user-wind";
    p.wind_loads = clean;
    if (settings.wind_point) p.wind_point = settings.wind_point;
    if (settings.wind_semi_rigid) p.wind_semi_rigid = settings.wind_semi_rigid;
  } else {
    delete p.auto;
    delete p.wind_loads;
  }
  return model;
}

/** Combinations that use a load case or combination directly. */
export function combosUsing(m: Model, kind: "case" | "combo", id: Id): Id[] {
  return m.load_combinations.filter((k) => k.items.some((it) => (kind === "case" ? it.case : it.combo) === id)).map((k) => k.id);
}

export function deleteLoadCase(m: Model, id: Id): Model {
  const users = combosUsing(m, "case", id);
  if (users.length) throw new EditError(t("err.caseInCombo", { id, list: users.join(", ") }));
  const model = clone(m);
  model.load_cases = model.load_cases.filter((x) => x.id !== id);
  renameNotRun(model, id, null);
  return model;
}

/** The cases a run can leave out, in the order they run: load cases, the modal case, spectrum cases. */
export function runCaseIds(m: Model): Id[] {
  return [...m.load_cases.map((c) => c.id), ...(m.modal ? [m.modal.id] : []), ...(m.spectrum_cases ?? []).map((c) => c.id)];
}

/** A case renamed (or deleted: `id` null) in the list of cases not to run; changes `m` in place. */
export function renameNotRun(m: Model, oldId: Id, id: Id | null) {
  if (!m.not_run?.includes(oldId)) return;
  m.not_run = m.not_run.flatMap((x) => (x !== oldId ? [x] : id === null ? [] : [id]));
  if (!m.not_run.length) delete m.not_run;
}

/** Analyze > Load Cases to Run: the cases set not to run (at least one case must run). */
export function setNotRun(m: Model, notRun: Id[]): Model {
  const all = runCaseIds(m);
  for (const id of notRun) if (!all.includes(id)) throw new EditError(t("err.unknownCase", { id }));
  if (all.length && all.every((id) => notRun.includes(id))) throw new EditError(t("err.nothingToRun"));
  const model = clone(m);
  const list = all.filter((id) => notRun.includes(id));
  if (list.length) model.not_run = list;
  else delete model.not_run;
  return model;
}

/** The cases (of `runCaseIds`) that have results: load and spectrum cases among them, the modal case with modal results. */
export function finishedCases(m: Model, r: Results | null): Set<Id> {
  const ids = new Set<Id>();
  if (!r) return ids;
  for (const c of r.cases) if (!c.combination) ids.add(c.id);
  if (r.modal && m.modal) ids.add(m.modal.id);
  return ids;
}

/**
 * The model a run analyses: the cases set not to run are left out, except those that have results
 * already, which run again so that their results stay (and combinations over old and new cases work).
 */
export function modelForRun(m: Model, r: Results | null): Model {
  if (!m.not_run?.length) return m;
  const done = finishedCases(m, r);
  const notRun = m.not_run.filter((id) => !done.has(id));
  const { not_run: _, ...rest } = m;
  return notRun.length ? { ...rest, not_run: notRun } : rest;
}

/**
 * What deleting the results of `ids` deletes: those cases, the spectrum cases with the modal case (they
 * are made from its modes), and every combination that uses one. `modal`: the modal results go too.
 */
export function resultsToDelete(m: Model, ids: Id[]): { ids: Set<Id>; modal: boolean } {
  const modal = m.modal !== undefined && ids.includes(m.modal.id);
  const cases = [...ids, ...(modal ? (m.spectrum_cases ?? []).map((c) => c.id) : [])];
  return { ids: new Set([...cases, ...combosNotRun(m, cases)]), modal };
}

/**
 * Combinations without results when `notRun` is left out: those with a part that does not run, nested
 * too. The modal case runs anyway while a spectrum case runs.
 */
export function combosNotRun(m: Model, notRun: Id[]): Id[] {
  const off = new Set(notRun);
  const dropped = new Set<Id>();
  for (let changed = true; changed; ) {
    changed = false;
    for (const k of m.load_combinations) {
      if (dropped.has(k.id)) continue;
      if (k.items.some((it) => (it.case !== undefined ? off.has(it.case) : it.combo !== undefined && dropped.has(it.combo)))) {
        dropped.add(k.id);
        changed = true;
      }
    }
  }
  return m.load_combinations.filter((k) => dropped.has(k.id)).map((k) => k.id);
}

/**
 * Adds or edits a load combination. Renaming updates the combinations that use it. Every part must
 * exist, scale factors must be numbers, and a combination may not contain itself.
 */
export function upsertCombination(m: Model, oldId: Id | null, combo: LoadCombination): Model {
  checkNewId(caseAndComboIds(m), oldId, combo.id, "combo");
  if (!combo.items.length) throw new EditError(t("err.comboEmpty"));
  for (const it of combo.items) {
    const ok =
      it.case !== undefined
        ? m.load_cases.some((c) => c.id === it.case) || (m.spectrum_cases ?? []).some((c) => c.id === it.case)
        : m.load_combinations.some((c) => c.id === it.combo);
    if (!ok) throw new EditError(t("err.comboUnknownPart", { id: (it.case ?? it.combo)! }));
    if (!Number.isFinite(it.scale)) throw new EditError(t("err.comboScale"));
  }
  const model = clone(m);
  const clean: LoadCombination = { id: combo.id, type: combo.type, items: combo.items.map((it) => ({ ...it })) };
  const i = oldId === null ? -1 : model.load_combinations.findIndex((x) => x.id === oldId);
  if (i >= 0) model.load_combinations[i] = clean;
  else model.load_combinations.push(clean);
  if (oldId !== null && oldId !== combo.id) {
    for (const k of model.load_combinations) for (const it of k.items) if (it.combo === oldId) it.combo = combo.id;
  }
  // No loops: walk down from the edited combination.
  const byId = new Map(model.load_combinations.map((k) => [k.id, k]));
  const reaches = (from: Id, seen: Set<Id>): boolean =>
    (byId.get(from)?.items ?? []).some((it) => it.combo !== undefined && (it.combo === combo.id || (!seen.has(it.combo) && (seen.add(it.combo), reaches(it.combo, seen)))));
  if (reaches(combo.id, new Set())) throw new EditError(t("err.comboLoop", { id: combo.id }));
  return model;
}

export function deleteCombination(m: Model, id: Id): Model {
  const users = combosUsing(m, "combo", id);
  if (users.length) throw new EditError(t("err.comboInCombo", { id, list: users.join(", ") }));
  const model = clone(m);
  model.load_combinations = model.load_combinations.filter((x) => x.id !== id);
  return model;
}

export function checkNewId(
  existing: Id[],
  oldId: Id | null,
  id: Id,
  what: "material" | "section" | "areaSection" | "pattern" | "case" | "combo" | "diaphragm" | "lineSpring" | "areaSpring" | "spectrumFunction" | "group" | "pierLabel" | "spandrelLabel",
) {
  const w = t(`err.what.${what}` as Key);
  if (!id.trim()) throw new EditError(t("err.giveName", { what: w }));
  if (id !== oldId && existing.includes(id)) throw new EditError(t("err.nameExists", { what: w, id }));
}

/** Moves a joint. The caller re-applies the end-order rule (members may need reversing). */
export function moveNode(m: Model, id: Id, p: Vec3): Model {
  const other = findNodeAt(m, p);
  if (other && other.id !== id) throw new EditError(t("err.jointAtPoint", { id: other.id }));
  const model = clone(m);
  const n = model.nodes.find((x) => x.id === id);
  if (!n) throw new EditError(t("err.unknownJoint"));
  [n.x, n.y, n.z] = p;
  for (const e of model.frames) {
    if (!e.nodes.includes(id)) continue;
    const [a, b] = e.nodes.map((x) => nodeById(model, x)!);
    if (dist(pos(a), pos(b)) <= SAME_POINT) throw new EditError(t("err.zeroLength", { id: e.id }));
  }
  for (const a of model.areas) {
    if (!a.nodes.includes(id)) continue;
    const problem = shapeProblem(a.nodes.map((x) => pos(nodeById(model, x)!)), !oneElement(model, a));
    if (problem) throw new EditError(t("err.areaShape", { problem }));
  }
  return model;
}

/** Default maximum mesh size, 1.25 m, in the model's length unit (as the mesh step). */
export function defaultMeshSize(length: string): number {
  return { mm: 1250, cm: 125, ft: 4, in: 48 }[length.toLowerCase()] ?? 1.25;
}

/**
 * An area set to no auto mesh that has more than 4 corners: one element needs 3 or 4, so the mesh step meshes it
 * with the model's settings (and says so in its notes).
 */
export const noMeshPolygon = (a: Pick<Area, "mesh"> & { nodes?: Id[] }): boolean => a.mesh === "none" && (a.nodes?.length ?? 0) > 4;

/** Whether an area is set to no auto mesh and is analysed so (one element). */
export const unmeshedArea = (a: Pick<Area, "mesh" | "nodes">): boolean => a.mesh === "none" && a.nodes.length <= 4;

/**
 * Whether an area becomes exactly one shell element (3 or 4 corners, convex). Openings and null areas
 * never become elements, and an area with a stored mesh gets its stored elements, so they may be any flat
 * polygon.
 */
export const oneElement = (_m: Model, a: Pick<Area, "section" | "opening" | "mesh" | "user_mesh"> & { nodes?: Id[] }): boolean =>
  !!a.section && !a.opening && !a.user_mesh && a.mesh === "none" && !noMeshPolygon(a);

/**
 * Active degrees of freedom: a space frame (all six), a plane frame in the X-Z plane (UX, UZ,
 * RY only; the solver's `"dimension": "2D"`) or a plane grid in the X-Y plane (UZ, RX, RY only, a floor with
 * its beams; `"grid"`). Models are always drawn in 3D.
 */
export function setDimension(m: Model, dimension: Dimension): Model {
  const model = clone(m);
  model.dimension = dimension;
  return model;
}

/** Plane frame (X-Z) on or off (space frame). */
export const setPlaneFrame = (m: Model, plane: boolean): Model => setDimension(m, plane ? "2D" : "3D");

/** Language-file key naming a model's active degrees of freedom. */
export const dimensionKey = (m: Model): Key => (m.dimension === "2D" ? "status.planeFrame" : m.dimension === "grid" ? "status.planeGrid" : "status.spaceFrame");

export function setGrid(m: Model, grid: Grid): Model {
  const model = clone(m);
  model.gui = { ...(model.gui ?? {}), grid: clone(grid) };
  return model;
}

/**
 * The model as the solver and the file get it: the grid's stories written as `stories` (diaphragms are per
 * story; a joint between levels belongs to the story above). A model without a grid keeps its own.
 */
export function withStories(m: Model): Model {
  const g = m.gui?.grid;
  if (!g?.stories.length) return m;
  return { ...m, stories: g.stories.map((s) => ({ id: s.label, z: s.z })) };
}

/** Adds a diaphragm, rigid or semi-rigid (Define > Diaphragms). */
export function addDiaphragm(m: Model, name: Id, semiRigid = false): Model {
  const id = name.trim();
  checkNewId((m.diaphragms ?? []).map((d) => d.id), null, id, "diaphragm");
  const model = clone(m);
  model.diaphragms = [...(model.diaphragms ?? []), { id, ...(semiRigid ? { semi_rigid: true } : {}) }];
  return model;
}

/** Makes a diaphragm rigid or semi-rigid. */
export function setDiaphragmSemiRigid(m: Model, id: Id, semiRigid: boolean): Model {
  const model = clone(m);
  const d = (model.diaphragms ?? []).find((x) => x.id === id);
  if (!d) throw new EditError(t("err.diaphragmUnknown", { id }));
  if (semiRigid) d.semi_rigid = true;
  else delete d.semi_rigid;
  return model;
}

/** Deletes a diaphragm and its assignments (refused while user wind loads act on it). */
export function deleteDiaphragm(m: Model, id: Id): Model {
  const users = m.load_patterns.filter((p) => p.wind_loads?.some((w) => w.diaphragm === id)).map((p) => p.id);
  if (users.length) throw new EditError(t("err.diaphragmInWind", { id, list: users.join(", ") }));
  const model = clone(m);
  model.diaphragms = (model.diaphragms ?? []).filter((d) => d.id !== id);
  for (const o of [...model.nodes, ...model.areas]) if (o.diaphragm === id) delete o.diaphragm;
  return model;
}

/** Assigns a diaphragm (or none) to joints and areas (Assign > Diaphragm). */
export function setDiaphragm(m: Model, nodes: Id[], areas: Id[], diaphragm: Id | null): Model {
  if (diaphragm && !(m.diaphragms ?? []).some((d) => d.id === diaphragm)) throw new EditError(t("err.diaphragmUnknown", { id: diaphragm }));
  const [ns, as] = [new Set(nodes), new Set(areas)];
  const model = clone(m);
  for (const o of [...model.nodes.filter((n) => ns.has(n.id)), ...model.areas.filter((a) => as.has(a.id))]) {
    if (diaphragm) o.diaphragm = diaphragm;
    else delete o.diaphragm;
  }
  return model;
}

/** The model's grid, or one made from the joints when the file has none. */
export function gridOf(m: Model): Grid {
  if (m.gui?.grid) return m.gui.grid;
  const uniq = (v: number[]) => [...new Set(v.map((x) => Math.round(x * 1e6) / 1e6))].sort((a, b) => a - b);
  const xs = uniq(m.nodes.map((n) => n.x));
  const ys = uniq(m.nodes.map((n) => n.y));
  const zs = uniq(m.nodes.map((n) => n.z));
  const g = makeGrid([], [], []);
  g.x = (xs.length ? xs : [0]).map((coord, i) => ({ label: letters(i), coord }));
  g.y = (ys.length ? ys : [0]).map((coord, i) => ({ label: String(i + 1), coord }));
  g.stories = (zs.length ? zs : [0]).map((z, i) => ({ label: i === 0 ? "Base" : `Story${i}`, z }));
  return g;
}

/** A name not used yet: "NAME-copy", then "NAME-copy2", "NAME-copy3", … */
export function copyName(existing: Id[], id: Id): Id {
  const used = new Set(existing);
  let name = `${id}-copy`;
  for (let k = 2; used.has(name); k++) name = `${id}-copy${k}`;
  return name;
}

export type DefinitionKind = "material" | "section" | "areaSection" | "pattern" | "case" | "combo";

/** Duplicates a material, section, area section, load pattern (with all its loads) or load case. Returns the new id. */
export function copyDefinition(m: Model, kind: DefinitionKind, id: Id): { model: Model; id: Id } {
  const model = clone(m);
  const lists: Record<DefinitionKind, { id: Id }[]> = {
    material: model.materials,
    section: model.sections,
    areaSection: model.area_sections,
    pattern: model.load_patterns,
    case: model.load_cases,
    combo: model.load_combinations,
  };
  const list = lists[kind];
  const src = list.find((x) => x.id === id);
  if (!src) throw new EditError(t("err.unknownDefinition", { what: t(`err.what.${kind}` as Key), id }));
  const copy = { ...clone(src), id: copyName(kind === "case" || kind === "combo" ? caseAndComboIds(model) : list.map((x) => x.id), id) };
  list.splice(list.indexOf(src) + 1, 0, copy);
  const shape = kind === "section" ? model.gui?.sectionShapes?.[id] : undefined;
  if (shape) model.gui!.sectionShapes![copy.id] = clone(shape);
  return { model, id: copy.id };
}

/** Members that use a material or section. */
export function membersUsing(m: Model, kind: "material" | "section", id: Id): Id[] {
  return m.frames.filter((e) => (kind === "material" ? e.material : e.section) === id).map((e) => e.id);
}
