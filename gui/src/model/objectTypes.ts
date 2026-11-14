// Object types: frames are Column / Beam / Brace / Null, areas Floor / Wall / Opening / Null.
// They are worked out, never stored (a type follows at once when joints move, when its
// two angle limits change and when the section changes): from the direction
// and the model's angle limits, the section ("" = Null) and the opening flag. Names are kept when the
// type changes (OSAB's decision). The types of a model are worked out once and kept, so
// selection, views and filters only read them.

import type { Key } from "../i18n";
import { areaPrefix, isFloor } from "./area";
import { pos, sub } from "./geometry";
import type { AngleLimits, Area, Frame, Id, Model, Vec3 } from "./types";

export type FrameType = "column" | "beam" | "brace" | "null";
export type AreaType = "floor" | "wall" | "opening" | "null";
export type ObjectType = FrameType | AreaType;

export const FRAME_TYPES: readonly FrameType[] = ["column", "beam", "brace", "null"];
export const AREA_TYPES: readonly AreaType[] = ["floor", "wall", "opening", "null"];

/** Default angle limits, in degrees. */
export const DEFAULT_ANGLE_LIMITS: AngleLimits = { column: 1, beam_floor: 20 };

export const angleLimitsOf = (m: Model): AngleLimits => m.angle_limits ?? DEFAULT_ANGLE_LIMITS;

/** Angle of a line from horizontal, degrees (0 to 90). */
export function lineSlope(a: Vec3, b: Vec3): number {
  const d = sub(b, a);
  return (Math.atan2(Math.abs(d[2]), Math.hypot(d[0], d[1])) * 180) / Math.PI;
}

/**
 * Column, beam or brace by direction: within `column` degrees of vertical a column, within
 * `beam_floor` of horizontal a beam, else a brace (reference cases: 89.1 degrees is a column, 88.9 a brace, 19.9 a
 * beam, 20.1 a brace with the default 1 and 20).
 */
export function lineType(a: Vec3, b: Vec3, lim: AngleLimits = DEFAULT_ANGLE_LIMITS): "column" | "beam" | "brace" {
  const s = lineSlope(a, b);
  if (90 - s < lim.column - 1e-9) return "column";
  if (s < lim.beam_floor - 1e-9) return "beam";
  return "brace";
}

/** Label letter of a frame: C, B or D by direction (null frames too; L is a rigid link). */
export const FRAME_PREFIX = { column: "C", beam: "B", brace: "D" } as const;

/** Label letter of an area: F or W by slope (openings and null areas too). */
export const areaLabelPrefix = (p: Vec3[], lim: AngleLimits): string => areaPrefix(p, lim.beam_floor);

function points(m: Model, ids: Id[]): Vec3[] | null {
  const out: Vec3[] = [];
  for (const id of ids) {
    const n = m.nodes.find((x) => x.id === id);
    if (!n) return null;
    out.push(pos(n));
  }
  return out;
}

export function frameType(m: Model, f: Frame, lim = angleLimitsOf(m)): FrameType {
  if (!f.section) return "null";
  const p = points(m, f.nodes);
  return p ? lineType(p[0], p[1], lim) : "brace";
}

export function areaType(m: Model, a: Area, lim = angleLimitsOf(m)): AreaType {
  if (a.opening) return "opening";
  if (!a.section) return "null";
  const p = points(m, a.nodes);
  return p && isFloor(p, lim.beam_floor) ? "floor" : "wall";
}

export interface ObjectTypes {
  frames: Map<Id, FrameType>;
  areas: Map<Id, AreaType>;
}

// Models are never changed in place (every edit makes a new one), so the types of a model can be kept.
const cache = new WeakMap<Model, ObjectTypes>();

/** The type of every frame and area of the model (worked out once per model). */
export function objectTypes(m: Model): ObjectTypes {
  let types = cache.get(m);
  if (!types) {
    const lim = angleLimitsOf(m);
    const byId = new Map(m.nodes.map((n) => [n.id, pos(n)]));
    const at = (ids: Id[]) => (ids.every((id) => byId.has(id)) ? ids.map((id) => byId.get(id)!) : null);
    const frames = new Map<Id, FrameType>();
    for (const f of m.frames) {
      const p = at(f.nodes);
      frames.set(f.id, !f.section ? "null" : p ? lineType(p[0], p[1], lim) : "brace");
    }
    const areas = new Map<Id, AreaType>();
    for (const a of m.areas) {
      const p = at(a.nodes);
      areas.set(a.id, a.opening ? "opening" : !a.section ? "null" : p && isFloor(p, lim.beam_floor) ? "floor" : "wall");
    }
    types = { frames, areas };
    cache.set(m, types);
  }
  return types;
}

/** Language-file key of a type's name ("Column", "Null area"…). */
export const typeName = (ty: ObjectType, of: "frame" | "area"): Key =>
  ty === "null" ? (of === "frame" ? "obj.nullFrame" : "obj.nullArea") : (`obj.${ty}` as Key);

/** Frames and areas of the given types (for Select > Object Type and display filters). */
export function idsOfType(m: Model, types: readonly ObjectType[]): { frames: Id[]; areas: Id[] } {
  const want = new Set<string>(types);
  const { frames, areas } = objectTypes(m);
  return {
    frames: [...frames].filter(([, ty]) => want.has(ty)).map(([id]) => id),
    areas: [...areas].filter(([, ty]) => want.has(ty)).map(([id]) => id),
  };
}
