// Groups: named sets of joints, frames, links and areas (an object may be in several). "All" is every
// object and is not stored. Define > Groups makes them; they select, show and hide objects by name.

import { t } from "../i18n";
import { EditError, checkNewId } from "./ops";
import type { Group, Id, Model } from "./types";

/** The kinds of objects a group holds. */
export const GROUP_KEYS = ["nodes", "frames", "links", "areas"] as const;
export type GroupKey = (typeof GROUP_KEYS)[number];
export type Members = Record<GroupKey, Id[]>;

/** The group every model has: every object. */
export const ALL = "All";

/** Colours given to new groups in turn. */
export const GROUP_COLORS = ["#d9534f", "#3f8fd2", "#5cb85c", "#f0ad4e", "#9b59b6", "#1abc9c", "#e67e22", "#7f8c8d"];

const groupsOf = (m: Model) => m.groups ?? [];

/** A group read from a file. */
export function readGroup(raw: Record<string, unknown>): Group {
  const g: Group = { id: String(raw.id ?? "") };
  if (typeof raw.color === "string" && raw.color) g.color = raw.color;
  for (const k of GROUP_KEYS) {
    const list = Array.isArray(raw[k]) ? (raw[k] as unknown[]).map(String) : [];
    if (list.length) g[k] = [...new Set(list)];
  }
  return g;
}

/** The objects of a group ("All": every object). */
export function groupMembers(m: Model, id: Id): Members {
  if (id === ALL) return { nodes: m.nodes.map((n) => n.id), frames: m.frames.map((f) => f.id), links: m.links.map((l) => l.id), areas: m.areas.map((a) => a.id) };
  const g = groupsOf(m).find((x) => x.id === id);
  return { nodes: g?.nodes ?? [], frames: g?.frames ?? [], links: g?.links ?? [], areas: g?.areas ?? [] };
}

/** How many objects a group has. */
export const memberCount = (s: Members) => GROUP_KEYS.reduce((n, k) => n + s[k].length, 0);

/** Adds a group or renames one / changes its colour (its objects stay). */
export function setGroup(m: Model, oldId: Id | null, g: { id: Id; color?: string }): Model {
  const id = g.id.trim();
  if (id.toLowerCase() === ALL.toLowerCase()) throw new EditError(t("err.groupAll"));
  checkNewId(
    groupsOf(m).map((x) => x.id),
    oldId,
    id,
    "group",
  );
  const model = structuredClone(m);
  const list = (model.groups ??= []);
  const i = oldId === null ? -1 : list.findIndex((x) => x.id === oldId);
  const color = g.color || (i >= 0 ? list[i].color : undefined) || GROUP_COLORS[list.length % GROUP_COLORS.length];
  if (i >= 0) list[i] = { ...list[i], id, color };
  else list.push({ id, color });
  return model;
}

/** Deletes a group; its objects stay. */
export function deleteGroup(m: Model, id: Id): Model {
  const model = structuredClone(m);
  model.groups = groupsOf(model).filter((g) => g.id !== id);
  if (!model.groups.length) delete model.groups;
  return model;
}

/** Puts objects into a group, or takes them out (`remove`). Only the objects given: a frame's joints stay out. */
export function assignGroup(m: Model, id: Id, objects: Partial<Members>, remove = false): Model {
  if (id === ALL) throw new EditError(t("err.groupAll"));
  const model = structuredClone(m);
  const g = groupsOf(model).find((x) => x.id === id);
  if (!g) throw new EditError(t("err.unknownGroup", { id }));
  for (const k of GROUP_KEYS) {
    const add = objects[k] ?? [];
    if (!add.length) continue;
    const set = new Set(g[k] ?? []);
    for (const o of add) {
      if (remove) set.delete(o);
      else set.add(o);
    }
    const order = new Map((model[k] as { id: Id }[]).map((o, n) => [o.id, n]));
    const list = [...set].filter((o) => order.has(o)).sort((a, b) => order.get(a)! - order.get(b)!);
    if (list.length) g[k] = list;
    else delete g[k];
  }
  return model;
}

/** The groups without objects that no longer exist (after a deletion); changes `m` in place. */
export function pruneGroups(m: Model) {
  if (!m.groups) return;
  const live: Record<GroupKey, Set<Id>> = {
    nodes: new Set(m.nodes.map((n) => n.id)),
    frames: new Set(m.frames.map((f) => f.id)),
    links: new Set(m.links.map((l) => l.id)),
    areas: new Set(m.areas.map((a) => a.id)),
  };
  for (const g of m.groups) {
    for (const k of GROUP_KEYS) {
      if (!g[k]) continue;
      const list = g[k]!.filter((o) => live[k].has(o));
      if (list.length) g[k] = list;
      else delete g[k];
    }
  }
}
