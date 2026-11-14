// Pier and spandrel labels: named labels (Define), given to frames and areas (Assign > Pier Label /
// Spandrel Label). The analysis makes a pier of the vertical walls and vertical frames with one pier label
// at a story, a spandrel of the vertical walls and horizontal frames with one spandrel label, and reports
// their forces (docs/phases/PIER_SPANDREL_PLAN.md); other objects keep the label but it does not count.

import { t } from "../i18n";
import { EditError, checkNewId } from "./ops";
import type { Id, Model, PierLabel, SpandrelLabel } from "./types";

export type WallKind = "pier" | "spandrel";
export const WALL_KINDS: WallKind[] = ["pier", "spandrel"];

type Label = PierLabel & SpandrelLabel;

/** Colours given to new labels in turn. */
export const LABEL_COLORS = ["#c0392b", "#2980b9", "#27ae60", "#d35400", "#8e44ad", "#16a085", "#f39c12", "#2c3e50"];

const key = (kind: WallKind) => (kind === "pier" ? "pier_labels" : "spandrel_labels");

/** The labels of one kind. */
export const labelsOf = (m: Model, kind: WallKind): Label[] => (m[key(kind)] ?? []) as Label[];

/** A label read from a file. */
export function readWallLabel(raw: Record<string, unknown>, kind: WallKind): Label {
  const l: Label = { id: String(raw.id ?? "") };
  if (kind === "spandrel" && raw.multi_story === true) l.multi_story = true;
  if (typeof raw.color === "string" && raw.color) l.color = raw.color;
  return l;
}

/** Adds a label, or renames one / changes it (the objects with it follow a new name). */
export function setWallLabel(m: Model, kind: WallKind, oldId: Id | null, l: { id: Id; color?: string; multi_story?: boolean }): Model {
  const id = l.id.trim();
  checkNewId(
    labelsOf(m, kind).map((x) => x.id),
    oldId,
    id,
    kind === "pier" ? "pierLabel" : "spandrelLabel",
  );
  const model = structuredClone(m);
  const list = (model[key(kind)] ??= []) as Label[];
  const i = oldId === null ? -1 : list.findIndex((x) => x.id === oldId);
  const color = l.color || (i >= 0 ? list[i].color : undefined) || LABEL_COLORS[list.length % LABEL_COLORS.length];
  const next: Label = { id, color };
  if (kind === "spandrel" && l.multi_story) next.multi_story = true;
  if (i >= 0) list[i] = next;
  else list.push(next);
  if (oldId !== null && oldId !== id) {
    for (const o of [...model.frames, ...model.areas]) if (o[kind] === oldId) o[kind] = id;
  }
  return model;
}

/** Deletes a label; the objects with it lose it. */
export function deleteWallLabel(m: Model, kind: WallKind, id: Id): Model {
  const model = structuredClone(m);
  const list = labelsOf(model, kind).filter((x) => x.id !== id);
  if (list.length) model[key(kind)] = list;
  else delete model[key(kind)];
  for (const o of [...model.frames, ...model.areas]) if (o[kind] === id) delete o[kind];
  return model;
}

/** Gives frames and areas a label, or takes their label away (`id` null). Openings take none. */
export function assignWallLabel(m: Model, kind: WallKind, id: Id | null, objects: { frames?: Id[]; areas?: Id[] }): Model {
  if (id !== null && !labelsOf(m, kind).some((x) => x.id === id)) throw new EditError(t("err.unknownLabel", { id }));
  const model = structuredClone(m);
  const frames = new Set(objects.frames ?? []);
  const areas = new Set(objects.areas ?? []);
  const targets = [...model.frames.filter((f) => frames.has(f.id)), ...model.areas.filter((a) => areas.has(a.id) && !a.opening)];
  for (const o of targets) {
    if (id === null) delete o[kind];
    else o[kind] = id;
  }
  return model;
}

/** How many frames and areas have a label. */
export const labelUsers = (m: Model, kind: WallKind, id: Id) => [...m.frames, ...m.areas].filter((o) => o[kind] === id).length;
