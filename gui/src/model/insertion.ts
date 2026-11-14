// Insertion points (Assign > Frame / Shell > Insertion Point): where a frame's section or a shell sits
// relative to its joints. The defaults of drawn objects: columns at 5 (middle centre), beams at 8 (top
// centre), braces at 10 (centroid), floors at their top, walls at their middle, all without the stiffness
// transform, so they move the drawing only (docs/phases/INSERTION_POINT_PLAN.md).

import { t, type Key } from "../i18n";
import { areaAxes, isFloor } from "./area";
import { add, localAxes, pos, scale } from "./geometry";
import { angleLimitsOf, lineType } from "./objectTypes";
import type { Area, AreaInsertion, Frame, FrameInsertion, Id, Model, Vec3 } from "./types";

const clone = <T>(v: T): T => structuredClone(v);

/** The centroid, nothing else: the same as no insertion point. */
export const isDefaultFrameInsertion = (i: FrameInsertion | undefined) =>
  !i || (i.point === 10 && !i.mirror2 && !i.mirror3 && !i.transform && !(i.offset_i ?? []).some((v) => v) && !(i.offset_j ?? []).some((v) => v));

export const isDefaultAreaInsertion = (i: AreaInsertion | undefined) => !i || (i.point === "middle" && !i.transform);

/** A frame insertion point without its default parts (null when it is the default). */
function cleanFrameInsertion(i: FrameInsertion): FrameInsertion | null {
  if (isDefaultFrameInsertion(i)) return null;
  const out: FrameInsertion = { point: i.point };
  if (i.mirror2) out.mirror2 = true;
  if (i.mirror3) out.mirror3 = true;
  if (i.offset_i?.some((v) => v)) out.offset_i = [...i.offset_i] as Vec3;
  if (i.offset_j?.some((v) => v)) out.offset_j = [...i.offset_j] as Vec3;
  if (i.global && (out.offset_i || out.offset_j)) out.global = true;
  if (i.transform) out.transform = true;
  return out;
}

/** Assign > Frame > Insertion Point: the same insertion point on these frames (null: the centroid). */
export function setFrameInsertion(m: Model, frames: Id[], ins: FrameInsertion | null): Model {
  const set = new Set(frames);
  const value = ins && cleanFrameInsertion(ins);
  const model = clone(m);
  for (const f of model.frames) {
    if (!set.has(f.id)) continue;
    if (value) f.insertion = clone(value);
    else delete f.insertion;
  }
  return model;
}

/** Assign > Shell > Insertion Point: the same insertion point on these areas (null: the middle). */
export function setAreaInsertion(m: Model, areas: Id[], ins: AreaInsertion | null): Model {
  const set = new Set(areas);
  const value = ins && !isDefaultAreaInsertion(ins) ? { point: ins.point, ...(ins.transform ? { transform: true } : {}) } : null;
  const model = clone(m);
  for (const a of model.areas) {
    if (!set.has(a.id) || a.opening) continue;
    if (value) a.insertion = { ...value };
    else delete a.insertion;
  }
  return model;
}

/** The insertion point a frame drawn from `a` to `b` gets: 5 for a column, 8 for a beam, none for a brace. */
export function drawnFrameInsertion(m: Model, a: Vec3, b: Vec3): FrameInsertion | undefined {
  const type = lineType(a, b, angleLimitsOf(m));
  return type === "column" ? { point: 5 } : type === "beam" ? { point: 8 } : undefined;
}

/** The insertion point an area drawn at these corners gets: the top for a floor, none for a wall. */
export function drawnAreaInsertion(m: Model, corners: Vec3[]): AreaInsertion | undefined {
  return isFloor(corners, angleLimitsOf(m).beam_floor) ? { point: "top" } : undefined;
}

/**
 * Where a frame's section centroid lies from its joints, at end i and end j (global): the cardinal
 * point on the section's box, mirrors and joint offsets (as drawn; the analysis uses it only with the
 * transform on).
 */
export function frameCentroidOffsets(m: Model, f: Frame, drawn?: { v: [number, number]; h: [number, number] }): [Vec3, Vec3] {
  const ins = f.insertion;
  const a = m.nodes.find((n) => n.id === f.nodes[0]);
  const b = m.nodes.find((n) => n.id === f.nodes[1]);
  if (!ins || !a || !b) return [[0, 0, 0], [0, 0, 0]];
  const sec = m.sections.find((s) => s.id === f.section);
  // A section given only by its properties is drawn as the rectangle with the same A and I33
  // (`sectionRects`): its cardinal points are on that rectangle.
  const eqDepth = sec && sec.A > 0 && sec.I33 > 0 ? Math.sqrt((12 * sec.I33) / sec.A) : 0;
  const [d, w] = [sec?.depth ?? eqDepth, sec?.width ?? (eqDepth > 0 ? sec!.A / eqDepth : 0)];
  const [c2, c3] = sec?.centroid_offset ?? [0, 0];
  // Row 0 = bottom (-2 face), column 0 = left (-3 side): the box's middle lies towards +2, +3, and the
  // centroid `centroid_offset` from it. Point 10 (and 11: no shear centre kept) is the centroid.
  const p = ins.point - 1;
  const box = ins.point >= 1 && ins.point <= 9;
  let d2 = box ? ((1 - Math.floor(p / 3)) * d) / 2 + c2 : 0;
  let d3 = box ? ((1 - (p % 3)) * w) / 2 + c3 : 0;
  // The outline as drawn (`drawn`: its extent along local 2 and 3 from the centroid) wins: the cardinal points
  // are on the box around what is drawn, whatever the section stores.
  if (box && drawn) {
    const pick = ([lo, hi]: [number, number], k: number) => (k === 0 ? lo : k === 1 ? (lo + hi) / 2 : hi);
    d2 = -pick(drawn.v, Math.floor(p / 3));
    d3 = -pick(drawn.h, p % 3);
  }
  if (ins.mirror3) d2 = -d2;
  if (ins.mirror2) d3 = -d3;
  const ax = localAxes(pos(a), pos(b), f.angle ?? 0);
  const card = add(scale(ax[1], d2), scale(ax[2], d3));
  const joint = (o: Vec3 | undefined): Vec3 =>
    !o ? [0, 0, 0] : ins.global ? o : add(add(scale(ax[0], o[0]), scale(ax[1], o[1])), scale(ax[2], o[2]));
  return [add(card, joint(ins.offset_i)), add(card, joint(ins.offset_j))];
}

/** Where an area's mid-plane lies from its joints (global; zero at the middle). */
export function areaMidPlaneOffset(m: Model, a: Area, corners: Vec3[]): Vec3 {
  const p = a.insertion?.point;
  if (!p || p === "middle") return [0, 0, 0];
  const t = m.area_sections.find((s) => s.id === a.section)?.thickness ?? 0;
  return scale(areaAxes(corners, a.angle ?? 0)[2], (p === "top" ? -0.5 : 0.5) * t);
}

/**
 * Short text of an insertion point for the view and tables: the cardinal point with M2 / M3 for the
 * mirrors, + for joint offsets and T when the stiffness is transformed ("8", "8 M2 T", "Top T"…).
 */
export function insertionText(i: FrameInsertion | AreaInsertion | undefined, area: boolean): string {
  if (!i) return area ? t("ins.middle") : "10";
  if (typeof i.point === "string") return t(`ins.${i.point}` as Key) + (i.transform ? " T" : "");
  const f = i as FrameInsertion;
  const offsets = (f.offset_i ?? []).some((v) => v) || (f.offset_j ?? []).some((v) => v);
  return [String(f.point), f.mirror2 ? "M2" : "", f.mirror3 ? "M3" : "", offsets ? "+" : "", f.transform ? "T" : ""].filter(Boolean).join(" ");
}
