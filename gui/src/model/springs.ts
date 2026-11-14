// Line and area springs (Define > Spring Properties, Assign > Frame > Line Springs, Assign > Shell > Area
// Springs): named properties of stiffness per unit length or area, assigned to frames and areas. The
// solver lumps them at each analysis element's joints (docs/phases/LINE_AREA_SPRINGS_PLAN.md).

import { t } from "../i18n";
import { EditError, checkNewId } from "./ops";
import type { AreaSpringProp, Id, LineSpringProp, Model } from "./types";

const clone = <T>(v: T): T => structuredClone(v);

export type SpringKind = "line" | "area";

/** The stiffness keys of each kind's properties. */
export const SPRING_KEYS = { line: ["u1", "u2", "u3", "r1"], area: ["u1", "u2", "u3"] } as const;

type Prop<K extends SpringKind> = K extends "line" ? LineSpringProp : AreaSpringProp;

const listOf = (m: Model, kind: SpringKind): (LineSpringProp | AreaSpringProp)[] =>
  (kind === "line" ? m.line_spring_props : m.area_spring_props) ?? [];

/** The objects using a spring property: frames for line springs, areas for area springs. */
export function springUsers(m: Model, kind: SpringKind, id: Id): Id[] {
  return kind === "line" ? m.frames.filter((f) => f.line_spring === id).map((f) => f.id) : m.areas.filter((a) => a.area_spring === id).map((a) => a.id);
}

/**
 * Adds a spring property, or replaces `oldId` (renaming it on the objects that use it). Values must be
 * numbers >= 0; zero ones are left out.
 */
export function upsertSpringProp<K extends SpringKind>(m: Model, kind: K, oldId: Id | null, prop: Prop<K>): Model {
  const id = prop.id.trim();
  const list = listOf(m, kind);
  checkNewId(
    list.map((p) => p.id).filter((x) => x !== oldId),
    null,
    id,
    kind === "line" ? "lineSpring" : "areaSpring",
  );
  const clean: Record<string, unknown> = { id };
  for (const k of SPRING_KEYS[kind]) {
    const v = (prop as unknown as Record<string, number | undefined>)[k] ?? 0;
    if (!(Number.isFinite(v) && v >= 0)) throw new EditError(t("spr.errValues"));
    if (v > 0) clean[k] = v;
  }
  const model = clone(m);
  const next = [...listOf(model, kind)];
  const at = oldId === null ? -1 : next.findIndex((p) => p.id === oldId);
  if (at >= 0) next[at] = clean as unknown as Prop<K>;
  else next.push(clean as unknown as Prop<K>);
  if (kind === "line") model.line_spring_props = next as LineSpringProp[];
  else model.area_spring_props = next as AreaSpringProp[];
  if (oldId !== null && oldId !== id) {
    for (const f of model.frames) if (kind === "line" && f.line_spring === oldId) f.line_spring = id;
    for (const a of model.areas) if (kind === "area" && a.area_spring === oldId) a.area_spring = id;
  }
  return model;
}

/** Deletes a spring property; the objects that used it lose their spring. */
export function deleteSpringProp(m: Model, kind: SpringKind, id: Id): Model {
  const model = clone(m);
  if (kind === "line") {
    model.line_spring_props = (model.line_spring_props ?? []).filter((p) => p.id !== id);
    if (!model.line_spring_props.length) delete model.line_spring_props;
    for (const f of model.frames) if (f.line_spring === id) delete f.line_spring;
  } else {
    model.area_spring_props = (model.area_spring_props ?? []).filter((p) => p.id !== id);
    if (!model.area_spring_props.length) delete model.area_spring_props;
    for (const a of model.areas) if (a.area_spring === id) delete a.area_spring;
  }
  return model;
}

/** Assign > Frame > Line Springs: the property on these frames (null: none). Null frames get none. */
export function assignLineSpring(m: Model, frames: Id[], id: Id | null): Model {
  if (id && !(m.line_spring_props ?? []).some((p) => p.id === id)) throw new EditError(t("spr.errUnknown", { id }));
  const set = new Set(frames);
  const model = clone(m);
  for (const f of model.frames) {
    if (!set.has(f.id)) continue;
    if (id && f.section) f.line_spring = id;
    else delete f.line_spring;
  }
  return model;
}

/** Assign > Shell > Area Springs: the property on these areas (null: none). Openings get none. */
export function assignAreaSpring(m: Model, areas: Id[], id: Id | null): Model {
  if (id && !(m.area_spring_props ?? []).some((p) => p.id === id)) throw new EditError(t("spr.errUnknown", { id }));
  const set = new Set(areas);
  const model = clone(m);
  for (const a of model.areas) {
    if (!set.has(a.id)) continue;
    if (id && !a.opening) a.area_spring = id;
    else delete a.area_spring;
  }
  return model;
}
