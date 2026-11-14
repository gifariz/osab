// Edit operations for areas (floors, walls) and area sections. Like ops.ts, each returns a new model.

import { t } from "../i18n";
import { analysisOrder, isFloor, areaAxes, quadPoint, shapeProblem, triPoint } from "./area";
import { pos } from "./geometry";
import { deckDepth, deckProblem } from "./deck";
import { drawnAreaInsertion } from "./insertion";
import { angleLimitsOf } from "./objectTypes";
import { readModifiers, withModifiers } from "./modifiers";
import { AREA_NAME, EditError, addNode, checkModifiers, defaultDiaphragm, nextNumber, nodeById, oneElement, type LoadMode } from "./ops";
import { AREA_MODIFIERS, type Area, type AreaLoad, type AreaMesh, type AreaModifier, type AreaSection, type Id, type LoadDir, type Model, type Vec3 } from "./types";

const clone = <T>(v: T): T => structuredClone(v);

export const areaById = (m: Model, id: Id) => m.areas.find((a) => a.id === id);

/** Corner coordinates of an area. */
export function areaPoints(m: Model, a: Area): Vec3[] {
  return a.nodes.map((n) => pos(nodeById(m, n)!));
}

/** Local axes of an area as analysed (its stored corner order and angle). */
export const areaAxesOf = (m: Model, a: Area) => areaAxes(areaPoints(m, a), a.angle ?? 0);

/** True for a floor (slope under the model's floor/wall limit, 20 degrees by default), false for a wall. */
export const areaIsFloor = (m: Model, a: Area) => isFloor(areaPoints(m, a), angleLimitsOf(m).beam_floor);

/**
 * Corner order to store: floors run counterclockwise seen from above (local 3 up), walls keep the
 * order given; then the list starts at the first lowest corner.
 */
export function storedOrder(m: Model, nodes: Id[]): Id[] {
  let ids = [...nodes];
  const p = () => ids.map((n) => pos(nodeById(m, n)!));
  if (isFloor(p(), angleLimitsOf(m).beam_floor) && areaAxes(p())[2][2] < 0) ids = [ids[0], ...ids.slice(1).reverse()];
  return analysisOrder(ids, p());
}

/** The next free area name: A and a number. */
const nextAreaId = (m: Model) => AREA_NAME + nextNumber(m.areas.map((a) => a.id), AREA_NAME);

export interface AreaKind {
  /** An opening (no section). */
  opening?: boolean;
}

/**
 * Adds an area on three or more existing joints. `section` "" makes a null area (property
 * "None"), `opening` an opening. When the model is meshed the area may have any number of corners
 * (flat, edges not crossing); otherwise 3 or 4, convex. The name is A plus a number (its label, F or
 * W by its slope, comes from labels.ts).
 */
export function addArea(m: Model, nodes: Id[], section: Id, kind: AreaKind = {}): { model: Model; id: Id } {
  if (nodes.length < 3 || new Set(nodes).size !== nodes.length || nodes.some((n) => !nodeById(m, n))) {
    throw new EditError(t("err.areaCorners"));
  }
  const sec = kind.opening ? "" : section;
  if (sec && !m.area_sections.some((s) => s.id === sec)) throw new EditError(t("err.defineAreaSection"));
  const key = [...nodes].sort().join("|");
  if (m.areas.some((a) => [...a.nodes].sort().join("|") === key)) throw new EditError(t("err.areaExists"));
  const problem = shapeProblem(
    nodes.map((n) => pos(nodeById(m, n)!)),
    !oneElement(m, { section: sec, opening: kind.opening || undefined }),
  );
  if (problem) throw new EditError(t("err.areaShape", { problem }));
  const ordered = storedOrder(m, nodes);
  const id = nextAreaId(m);
  const model = clone(m);
  const area: Area = kind.opening ? { id, nodes: ordered, section: "", opening: true } : { id, nodes: ordered, section: sec };
  if (sec && !kind.opening) Object.assign(area, drawnAreaDefaults(m, ordered.map((n) => pos(nodeById(m, n)!))));
  model.areas.push(area);
  return { model, id };
}

/**
 * What a new area with a section at these corners gets: a floor gets the default diaphragm and is drawn at its
 * top (the insertion point: the drawing only). The example models get it too.
 */
export function drawnAreaDefaults(m: Model, corners: Vec3[]): Pick<Area, "diaphragm" | "insertion"> {
  if (!isFloor(corners, angleLimitsOf(m).beam_floor)) return {};
  const d = defaultDiaphragm(m);
  const ins = drawnAreaInsertion(m, corners);
  return { ...(d ? { diaphragm: d } : {}), ...(ins ? { insertion: ins } : {}) };
}

/** Adds an area at these points, adding joints where there are none yet. */
export function addAreaAt(m: Model, points: Vec3[], section: Id, kind: AreaKind = {}): { model: Model; id: Id } {
  let model = m;
  const nodes: Id[] = [];
  for (const p of points) {
    const r = addNode(model, p);
    model = r.model;
    nodes.push(r.id);
  }
  return addArea(model, nodes, section, kind);
}

/**
 * Makes areas openings (no section; their area loads are removed, as openings carry none) or turns
 * openings back into null areas (an opening switched off keeps the property "None").
 */
export function setOpening(m: Model, ids: Id[], opening: boolean): Model {
  const set = new Set(ids);
  const model = clone(m);
  for (const a of model.areas) {
    if (!set.has(a.id)) continue;
    if (opening) {
      a.opening = true;
      a.section = "";
    } else delete a.opening;
  }
  if (opening) for (const c of model.load_patterns) c.area_loads = c.area_loads.filter((l) => !set.has(l.area));
  return model;
}

/**
 * *Assign > Area Mesh Options*: "default" (the model's settings), "none" (one element) or an own
 * maximum size; and "add restraints on edges if the corners have restraints" (undefined = the default:
 * on for walls, off for floors).
 */
export function setAreaMesh(m: Model, ids: Id[], mesh: "default" | AreaMesh, edgeRestraints?: boolean): Model {
  if (mesh !== "default" && mesh !== "none" && !(mesh.size > 0 && Number.isFinite(mesh.size))) throw new EditError(t("err.meshSize"));
  const set = new Set(ids);
  const model = clone(m);
  for (const a of model.areas) {
    if (!set.has(a.id)) continue;
    if (mesh === "default") delete a.mesh;
    else a.mesh = mesh === "none" ? "none" : { size: mesh.size };
    if (edgeRestraints === undefined) delete a.edge_restraints;
    else a.edge_restraints = edgeRestraints;
  }
  return model;
}

/**
 * Divides each area into n1 x n2 areas (n1 along edge 1-2, n2 along edge 2-3); a triangle into n1 x n1 triangles (n1 pieces along every edge). Joints at the
 * same point are shared; section, angle and loads are kept. Returns the new area ids.
 */
export function divideAreas(m: Model, ids: Id[], n1: number, n2: number): { model: Model; ids: Id[] } {
  const ok = (n: number) => Number.isInteger(n) && n >= 1 && n <= 50;
  if (!ok(n1) || !ok(n2)) throw new EditError(t("err.divideCount"));
  let model = clone(m);
  const created: Id[] = [];
  for (const id of ids) {
    const area = areaById(model, id);
    if (!area || (n1 === 1 && n2 === 1)) {
      if (area) created.push(id);
      continue;
    }
    const p = areaPoints(model, area);
    if (p.length > 4) throw new EditError(t("err.dividePolygon", { id }));
    const tri = p.length === 3;
    const nj = tri ? n1 : n2;
    const grid: Id[][] = [];
    for (let i = 0; i <= n1; i++) {
      grid.push([]);
      for (let j = 0; j <= nj; j++) {
        if (tri && i + j > n1) continue;
        const r = addNode(model, tri ? triPoint(p, 1 - (i + j) / n1, i / n1, j / n1) : quadPoint(p, i / n1, j / n2));
        model = r.model;
        grid[i][j] = r.id;
      }
    }
    // New names follow the largest one in use (the divided area's included).
    const pieces: Area[] = [];
    const add = (nodes: Id[]) => {
      const pts = nodes.map((n) => pos(nodeById(model, n)!));
      // The pieces keep everything but the name and corners: section, axes, opening and mesh options (not
      // a stored mesh, which was the whole area's).
      const piece: Area = { ...clone(area), id: nextAreaId(model), nodes: analysisOrder(nodes, pts) };
      delete piece.user_mesh;
      model.areas.push(piece);
      pieces.push(piece);
    };
    for (let j = 0; j < nj; j++) {
      for (let i = 0; i < n1; i++) {
        if (!tri) add([grid[i][j], grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1]]);
        else if (i + j < n1) {
          // Same turning direction as the parent: (i, j), (i+1, j), (i, j+1), and the one above it.
          add([grid[i][j], grid[i + 1][j], grid[i][j + 1]]);
          if (i + j + 1 < n1) add([grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1]]);
        }
      }
    }
    model.areas = model.areas.filter((a) => a.id !== id);
    for (const c of model.load_patterns) {
      const loads = c.area_loads.filter((l) => l.area === id);
      c.area_loads = c.area_loads.filter((l) => l.area !== id);
      for (const piece of pieces) for (const l of loads) c.area_loads.push({ ...l, area: piece.id });
    }
    created.push(...pieces.map((a) => a.id));
  }
  return { model, ids: created };
}

/**
 * Assigns an area section ("" = null area, "None") and/or local axis angle (0 = none). `reverse`
 * flips the corner order (and so local 3), keeping the first corner. Openings keep no section (make
 * them areas first with `setOpening`).
 */
export function assignArea(m: Model, ids: Id[], patch: { section?: Id; angle?: number; reverse?: boolean }): Model {
  if (patch.section && !m.area_sections.some((s) => s.id === patch.section)) {
    throw new EditError(t("err.defineAreaSection"));
  }
  const set = new Set(ids);
  const model = clone(m);
  for (const a of model.areas) {
    if (!set.has(a.id)) continue;
    if (patch.section !== undefined && !a.opening) a.section = patch.section;
    if (patch.angle !== undefined) {
      if (patch.angle) a.angle = patch.angle;
      else delete a.angle;
    }
    if (patch.reverse) a.nodes = [a.nodes[0], ...a.nodes.slice(1).reverse()];
  }
  return model;
}

/**
 * Sets property modifiers of area objects (Assign > Area > Property Modifiers): the given values
 * replace theirs, undefined ones are kept. They multiply the area section's modifiers in the analysis.
 * Openings have no stiffness and get none.
 */
export function setAreaModifiers(m: Model, ids: Id[], values: Partial<Record<AreaModifier, number | undefined>>): Model {
  const set = new Set(ids);
  const model = clone(m);
  for (const a of model.areas) {
    if (!set.has(a.id) || a.opening) continue;
    const mods = withModifiers(AREA_MODIFIERS, a.modifiers, values);
    checkModifiers("area", mods);
    if (mods) a.modifiers = mods;
    else delete a.modifiers;
  }
  return model;
}

/**
 * Uniform area loads (force per area). "add" sums into a load of the same direction, "replace" removes
 * the areas' loads in the pattern first, "delete" only removes them.
 */
export function setAreaLoads(m: Model, patternId: Id, ids: Id[], spec: { dir: LoadDir; q: number } | null, mode: LoadMode): Model {
  const model = clone(m);
  const c = model.load_patterns.find((x) => x.id === patternId);
  if (!c) throw new EditError(t("err.unknownPattern", { id: patternId }));
  const set = new Set(ids);
  if (mode !== "add") c.area_loads = c.area_loads.filter((l) => !set.has(l.area));
  if (mode === "delete" || !spec) return model;
  for (const id of ids) {
    // Openings carry no loads; null areas do: their loads go to the floors under them.
    if (!areaById(model, id) || areaById(model, id)!.opening) continue;
    const same = c.area_loads.find((l) => l.area === id && l.dir === spec.dir);
    if (same) same.q += spec.q;
    else c.area_loads.push({ area: id, dir: spec.dir, q: spec.q });
  }
  c.area_loads = c.area_loads.filter((l: AreaLoad) => l.q !== 0);
  return model;
}

/** Adds or edits an area section. Renaming updates the areas that use it. */
export function upsertAreaSection(m: Model, oldId: Id | null, sec: AreaSection): Model {
  const w = t("err.what.areaSection");
  if (!sec.id.trim()) throw new EditError(t("err.giveName", { what: w }));
  if (sec.id !== oldId && m.area_sections.some((x) => x.id === sec.id)) throw new EditError(t("err.nameExists", { what: w, id: sec.id }));
  if (!m.materials.some((x) => x.id === sec.material)) throw new EditError(t("err.areaSectionMaterial"));
  const property = sec.property_type === "wall" || (sec.property_type === "deck" && sec.deck) ? sec.property_type : "slab";
  const deck = property === "deck" ? sec.deck : undefined;
  if (deck) {
    if (!m.materials.some((x) => x.id === deck.steel)) throw new EditError(t("err.deckMaterial"));
    const problem = deckProblem(deck);
    if (problem) throw new EditError(t(problem));
  } else if (!(sec.thickness > 0)) throw new EditError(t("err.thicknessPositive"));
  const model = clone(m);
  // A deck is always a one-way Membrane; its thickness is its total depth (for drawing).
  const clean: AreaSection = deck
    ? { id: sec.id, type: "membrane", material: sec.material, thickness: deckDepth(deck), property_type: property, one_way: true, deck: { ...deck } }
    : { id: sec.id, type: sec.type, material: sec.material, thickness: sec.thickness, property_type: property };
  if (property === "slab" && sec.slab_type && sec.slab_type !== "slab") clean.slab_type = sec.slab_type;
  if (!deck && sec.one_way && sec.type === "membrane") clean.one_way = true;
  const mods = readModifiers(AREA_MODIFIERS, sec.modifiers);
  checkModifiers("area", mods);
  if (mods) clean.modifiers = mods;
  const i = oldId === null ? -1 : model.area_sections.findIndex((x) => x.id === oldId);
  if (i >= 0) model.area_sections[i] = clean;
  else model.area_sections.push(clean);
  if (oldId !== null && oldId !== sec.id) for (const a of model.areas) if (a.section === oldId) a.section = sec.id;
  return model;
}

export function deleteAreaSection(m: Model, id: Id): Model {
  const users = m.areas.filter((a) => a.section === id).length;
  if (users) throw new EditError(t("err.areaSectionUsed", { id, n: users }));
  const model = clone(m);
  model.area_sections = model.area_sections.filter((x) => x.id !== id);
  return model;
}

/** Areas that use an area section, or (through their sections) a material. */
export function areasUsing(m: Model, kind: "areaSection" | "material", id: Id): Id[] {
  const secs = kind === "areaSection" ? new Set([id]) : new Set(m.area_sections.filter((s) => s.material === id).map((s) => s.id));
  return m.areas.filter((a) => secs.has(a.section)).map((a) => a.id);
}
