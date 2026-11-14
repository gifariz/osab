// Results on demand: a run hands the window a summary (every case's name and checks, the mesh, modal
// results, centres) and the values of a case are fetched when a view or table shows it. The desktop app
// keeps the results in its Rust process and sends a case's numbers as binary, read back here with the
// summary's geometry, and a mode's shape the same way when a view shows the mode; the browser keeps them
// in the solver worker. Only a few cases and shapes stay loaded.

import { DOFS, FORCES, SHELL_COMPONENTS, type CaseResult, type ElementResult, type Id, type ModeShape, type Results, type ShellResult, type Station, type Vec3 } from "../model/types";

/** A case of the summary as it comes from the solver side: no values yet. */
export type CaseHead = Pick<CaseResult, "id" | "combination" | "step" | "spectrum" | "equilibrium" | "base_reaction" | "story_drifts" | "story_forces" | "pier_forces" | "spandrel_forces"> & {
  has_shells?: boolean;
};

/** The head of a case: everything but its values (joint drifts are values: one per joint at a level; story,
 * pier and spandrel forces are few and come with the head). */
export function headOf(c: CaseResult): CaseHead {
  return {
    id: c.id,
    ...(c.combination ? { combination: c.combination } : {}),
    ...(c.step ? { step: c.step } : {}),
    ...(c.spectrum ? { spectrum: c.spectrum } : {}),
    ...(c.equilibrium ? { equilibrium: c.equilibrium } : {}),
    ...(c.base_reaction ? { base_reaction: c.base_reaction } : {}),
    ...(c.story_drifts?.length ? { story_drifts: c.story_drifts } : {}),
    ...(c.story_forces?.length ? { story_forces: c.story_forces } : {}),
    ...(c.pier_forces?.length ? { pier_forces: c.pier_forces } : {}),
    ...(c.spandrel_forces?.length ? { spandrel_forces: c.spandrel_forces } : {}),
    ...(c.shells?.length || c.has_shells ? { has_shells: true } : {}),
  };
}

/** The results without the cases' values (as the solver side sends them; the desktop app adds the geometry). */
export type ResultsSummary = Omit<Results, "cases"> & { cases: CaseHead[]; geometry?: Geometry };

/** The summary of full results (the browser worker sends this). */
export function summarize(r: Results): ResultsSummary {
  return {
    ...r,
    cases: r.cases.map(headOf),
  };
}

/** A case whose values are not here yet: empty tables until it is loaded. */
export function pendingCase(h: CaseHead): CaseResult {
  return { ...h, displacements: [], reactions: [], frames: [], links: [], shells: [], pending: true };
}

/** Results from a summary: every case pending, and every mode shape the summary leaves out. */
export function fromSummary(s: ResultsSummary): Results {
  const { geometry: _, ...rest } = s;
  const modal = s.modal && { ...s.modal, modes: s.modal.modes.map((m) => (m.shape ? m : { ...m, shape: [], pending: true })) };
  return { ...rest, ...(modal ? { modal } : {}), cases: s.cases.map(pendingCase) };
}

/** How many cases keep their values in the window at once (the rest are fetched again when shown). */
export const LOADED_CASES = 4;
/** How many mode shapes stay loaded at once. */
export const LOADED_MODES = 2;

// ---- Values read where they arrived ----

/**
 * A row of numbers read in place: `keys[i]` is `v[o + i]`, through a getter on the prototype. A large case
 * has millions of rows (joints, stations, shell corners); as plain objects every number would be an object of
 * its own, here a row is one small object over the bytes that came. The values are the same numbers.
 * `toJSON` gives the plain object (`plain`).
 */
interface Row {
  readonly v: Float64Array;
  readonly o: number;
}

function defineValues<K extends string>(proto: object, keys: readonly K[]) {
  keys.forEach((k, i) =>
    Object.defineProperty(proto, k, {
      get(this: Row) {
        return this.v[this.o + i];
      },
      enumerable: true,
    }),
  );
}

/** Rows of `keys` (frame and link stations). */
function rowsOf<K extends string>(keys: readonly K[]): new (v: Float64Array, o: number) => Record<K, number> {
  class Values implements Row {
    constructor(
      readonly v: Float64Array,
      readonly o: number,
    ) {}
    toJSON() {
      const out = {} as Record<K, number>;
      keys.forEach((k, i) => (out[k] = this.v[this.o + i]));
      return out;
    }
  }
  defineValues(Values.prototype, keys);
  return Values as unknown as new (v: Float64Array, o: number) => Record<K, number>;
}

/** Rows of a joint's `keys` (displacements, reactions, shell corners, mode shapes). */
function jointRowsOf<K extends string>(keys: readonly K[]): new (node: Id, v: Float64Array, o: number) => { node: Id } & Record<K, number> {
  class JointValues implements Row {
    constructor(
      readonly node: Id,
      readonly v: Float64Array,
      readonly o: number,
    ) {}
    toJSON() {
      const out = { node: this.node } as { node: Id } & Record<K, number>;
      keys.forEach((k, i) => ((out as Record<K, number>)[k] = this.v[this.o + i]));
      return out;
    }
  }
  defineValues(JointValues.prototype, keys);
  return JointValues as unknown as new (node: Id, v: Float64Array, o: number) => { node: Id } & Record<K, number>;
}

const DispValues = jointRowsOf(DOFS);
const ForceValues = jointRowsOf(FORCES);
const CornerValues = jointRowsOf(SHELL_COMPONENTS);

/** Plain objects and arrays of a value whose rows are read in place (for tests and exports, which look at own fields). */
export function plain<T>(x: T): T {
  if (Array.isArray(x)) return x.map(plain) as T;
  if (x === null || typeof x !== "object") return x;
  if (typeof (x as { toJSON?: unknown }).toJSON === "function") return (x as unknown as { toJSON(): T }).toJSON();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(x)) out[k] = plain(v);
  return out as T;
}

/** A mode's shape from the bytes the desktop app sends: `OWB1` and UX..RZ of each of `joints`, or `OWJ1` and JSON. */
export function decodeShape(buf: ArrayBuffer, joints: Id[] | undefined): ModeShape {
  const magic = new TextDecoder().decode(new Uint8Array(buf, 0, 4));
  if (magic === "OWJ1") return JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 8))) as ModeShape;
  if (magic !== "OWB1" || !joints) throw new Error(`unknown mode data (${magic})`);
  const v = new Float64Array(buf, 8);
  return joints.map((node, i) => new DispValues(node, v, 6 * i));
}

// ---- Binary case values (desktop app) ----

/** A frame or link as every case of a run has it (sent once, in the summary). */
export interface ElementGeometry {
  element: Id;
  nodes: [Id, Id];
  length: number;
  offsets?: [number, number];
  axes: [Vec3, Vec3, Vec3];
}

/** A shell element as every case has it, with the joint of each corner. */
export interface ShellGeometry {
  element: Id;
  area: Id;
  nodes: Id[];
  axes: [Vec3, Vec3, Vec3];
  corners: Id[];
}

/** The objects of a run's cases, in their order (gui/src-tauri/src/case_codec.rs). */
export interface Geometry {
  displacements: Id[];
  reactions: Id[];
  frames: ElementGeometry[];
  links: ElementGeometry[];
  shells: ShellGeometry[];
  joint_drifts?: { story: string; node: Id; below: Id }[];
}

const STATION_KEYS = ["x", "P", "V2", "V3", "T", "M2", "M3", "ux", "uy", "uz"] as const;
const StationValues = rowsOf(STATION_KEYS);

/**
 * A case from the bytes the desktop app sends: `OWB1` and the numbers in the geometry's order, or `OWJ1`
 * and the case as JSON (a case whose objects differ from the geometry). The same objects as from JSON.
 */
export function decodeCase(buf: ArrayBuffer, head: CaseHead, g: Geometry | undefined): CaseResult {
  const magic = new TextDecoder().decode(new Uint8Array(buf, 0, 4));
  if (magic === "OWJ1") return JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 8))) as CaseResult;
  if (magic !== "OWB1" || !g) throw new Error(`unknown case data (${magic})`);
  // Rows read the numbers where they are (above); `k` walks them in the geometry's order.
  const v = new Float64Array(buf, 8);
  let k = 0;
  const take = (n: number) => {
    const o = k;
    k += n;
    return o;
  };
  const displacements = g.displacements.map((node) => new DispValues(node, v, take(DOFS.length)));
  const reactions = g.reactions.map((node) => new ForceValues(node, v, take(FORCES.length)));
  const elements = (list: ElementGeometry[]): ElementResult[] =>
    list.map((e) => {
      const n = v[k++];
      const stations: Station[] = [];
      for (let s = 0; s < n; s++) stations.push(new StationValues(v, take(STATION_KEYS.length)) as unknown as Station);
      return { ...e, stations };
    });
  const frames = elements(g.frames);
  const links = elements(g.links);
  const shells: ShellResult[] = g.shells.map((s) => {
    const { corners, ...rest } = s;
    return { ...rest, corners: corners.map((node) => new CornerValues(node, v, take(SHELL_COMPONENTS.length))) };
  });
  const joint_drifts = (g.joint_drifts ?? []).map((d) => ({ ...d, drift: [v[k++], v[k++]] as [number, number] }));
  const { has_shells: _, ...h } = head;
  return { ...h, displacements, reactions, frames, links, ...(shells.length ? { shells } : {}), ...(joint_drifts.length ? { joint_drifts } : {}) };
}
