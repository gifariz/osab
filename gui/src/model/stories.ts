// Stories: the levels of a building (base first) and the story every object belongs to, and applying
// an edited story table (heights, names, added and deleted stories), which moves the structure with
// its stories. Rules (docs/SOURCES.md):
// - a joint belongs to the lowest level at or above it; below the base to the base, above the top
//   level to the top story. A frame, link or area belongs to the story of its highest joint (a column
//   to the level at its top, a beam at mid-height to the level above it).
// - when the levels change, every joint keeps its distance to its own story's level: a column's bottom
//   joint (at the story below) follows that story, its top joint its own one, so it stretches with its
//   story; a joint between levels keeps its distance below the level above.
// - frame loads keep their position as a fraction of the member's length (also those given as a
//   distance).
// - deleting a story deletes its structure (the objects of that story, and its joints no other object
//   uses); the story above grows to cover it, so the levels above stay. The joints of the deleted story
//   that other objects still use move down with the deleted level to the level below (a column of the
//   story above then reaches down to it) and join a joint already there.

import { t } from "../i18n";
import { dist, pos } from "./geometry";
import { EditError, SAME_POINT, deleteObjects, keepLoadFractions, memberLength, mergeJoints, nodeById } from "./ops";
import type { Id, Model, StoryLevel } from "./types";

const clone = <T>(v: T): T => structuredClone(v);
const round = (v: number) => Math.round(v * 1e9) / 1e9;

/** The model's story levels, base first: the grid's (models made in the app) or the file's own. */
export function storyLevels(m: Model): StoryLevel[] {
  const g = m.gui?.grid?.stories;
  const levels = g?.length ? g.map((s) => ({ id: s.label, z: s.z })) : (m.stories ?? []).map((s) => ({ ...s }));
  return levels.sort((a, b) => a.z - b.z);
}

/** The model with these story levels (base first), kept where the model keeps them. */
export function setStoryLevels(m: Model, levels: StoryLevel[]): Model {
  const model = clone(m);
  const sorted = [...levels].sort((a, b) => a.z - b.z);
  const grid = model.gui?.grid;
  if (grid) grid.stories = sorted.map((s) => ({ label: s.id, z: s.z }));
  if (!grid || model.stories) model.stories = sorted.map((s) => ({ id: s.id, z: s.z }));
  return model;
}

/** Index of the story a height belongs to: the lowest level at or above it (0 below the base, the top above the top). */
export function storyIndexAt(levels: StoryLevel[], z: number): number {
  for (let i = 0; i < levels.length; i++) if (z <= levels[i].z + SAME_POINT) return i;
  return levels.length - 1;
}

/** The story a height belongs to ("" in a model without stories). */
export const storyAt = (levels: StoryLevel[], z: number): Id => levels[storyIndexAt(levels, z)]?.id ?? "";

/** Story index of every joint, frame, link and area. */
interface StoryIndices {
  nodes: Map<Id, number>;
  frames: Map<Id, number>;
  links: Map<Id, number>;
  areas: Map<Id, number>;
}

function storyIndices(m: Model, levels: StoryLevel[]): StoryIndices {
  const nodes = new Map(m.nodes.map((n) => [n.id, storyIndexAt(levels, n.z)]));
  const highest = (ids: Id[]) => Math.max(...ids.map((id) => nodes.get(id) ?? 0));
  return {
    nodes,
    frames: new Map(m.frames.map((f) => [f.id, highest(f.nodes)])),
    links: new Map(m.links.map((l) => [l.id, highest(l.nodes)])),
    areas: new Map(m.areas.map((a) => [a.id, highest(a.nodes)])),
  };
}

/** The story of every joint, frame, link and area (story names; empty maps in a model without stories). */
export interface ObjectStories {
  nodes: Map<Id, Id>;
  frames: Map<Id, Id>;
  links: Map<Id, Id>;
  areas: Map<Id, Id>;
}

// Models are never changed in place (every edit makes a new one), so the stories of a model can be kept.
const cache = new WeakMap<Model, ObjectStories>();

/** The story of every object of the model (worked out once per model). */
export function objectStories(m: Model): ObjectStories {
  let out = cache.get(m);
  if (!out) {
    const levels = storyLevels(m);
    const named = (map: Map<Id, number>) => new Map(levels.length ? [...map].map(([id, i]) => [id, levels[i].id]) : []);
    const idx = storyIndices(m, levels);
    out = { nodes: named(idx.nodes), frames: named(idx.frames), links: named(idx.links), areas: named(idx.areas) };
    cache.set(m, out);
  }
  return out;
}

/** The objects of a story (what deleting it deletes, joints only when no other object uses them). */
export function storyObjects(m: Model, story: Id): { nodes: Id[]; frames: Id[]; links: Id[]; areas: Id[] } {
  const levels = storyLevels(m);
  const k = levels.findIndex((s) => s.id === story);
  if (k <= 0) return { nodes: [], frames: [], links: [], areas: [] };
  return deletedStructure(m, storyIndices(m, levels), new Set([k]));
}

function deletedStructure(m: Model, idx: StoryIndices, deleted: Set<number>) {
  const of = (map: Map<Id, number>) => [...map].filter(([, i]) => deleted.has(i)).map(([id]) => id);
  const [frames, links, areas] = [of(idx.frames), of(idx.links), of(idx.areas)];
  const gone = new Set([...frames, ...links, ...areas]);
  const used = new Set<Id>();
  for (const o of [...m.frames, ...m.links, ...m.areas]) if (!gone.has(o.id)) for (const n of o.nodes) used.add(n);
  const nodes = of(idx.nodes).filter((id) => !used.has(id));
  return { nodes, frames, links, areas };
}

/** A row of an edited story table: its name, its level and the story it was (absent: a new story). */
export interface StoryRow {
  id: Id;
  z: number;
  from?: Id;
}

/** Story table rows (base first) from the base and the story heights above it, bottom up. */
export function rowsFromHeights(base: StoryRow, stories: { id: Id; height: number; from?: Id }[]): StoryRow[] {
  const rows: StoryRow[] = [{ ...base }];
  for (const s of stories) rows.push({ id: s.id, z: round(rows[rows.length - 1].z + s.height), ...(s.from !== undefined ? { from: s.from } : {}) });
  return rows;
}

/** What applying a story table did besides moving joints. */
export interface StoryEdit {
  model: Model;
  /** Objects deleted with deleted stories. */
  deleted: { nodes: Id[]; frames: Id[]; links: Id[]; areas: Id[] };
  /** Joints of deleted stories that joined a joint at the level below: joint -> the joint it joined. */
  merged: Map<Id, Id>;
  /** Areas whose stored mesh was removed because the area changed shape (they are meshed again). */
  meshesCleared: Id[];
  /** User wind load rows removed with deleted stories. */
  windRowsRemoved: number;
}

function checkRows(old: StoryLevel[], rows: StoryRow[]) {
  if (!rows.length || rows[0].from !== old[0]?.id) throw new EditError(t("err.storyBase"));
  const names = new Set<Id>();
  const from = new Map<Id, number>(old.map((s, i) => [s.id, i]));
  let lastOld = -1;
  for (let r = 0; r < rows.length; r++) {
    const row = rows[r];
    const id = row.id.trim();
    if (!id) throw new EditError(t("err.storyName"));
    if (names.has(id)) throw new EditError(t("err.storyNameTwice", { id }));
    names.add(id);
    if (!Number.isFinite(row.z)) throw new EditError(t("err.storyLevel", { id }));
    if (r > 0 && !(row.z > rows[r - 1].z + SAME_POINT)) throw new EditError(t("err.storyHeight", { id }));
    if (row.from !== undefined) {
      const i = from.get(row.from);
      // Each old story at most once and in its old order (stories cannot swap places).
      if (i === undefined || i <= lastOld) throw new EditError(t("err.storyFrom", { id }));
      lastOld = i;
    }
  }
}

/**
 * Applies an edited story table (rows base first; each row names the story it was, or none for a new
 * story). Joints keep their distance to their story's level, deleted stories take their structure
 * with them, renamed stories carry their names into user wind loads. One edit: the caller keeps it as
 * one undo step.
 */
export function applyStoryTable(m: Model, rows: StoryRow[]): StoryEdit {
  const old = storyLevels(m);
  if (!old.length) throw new EditError(t("err.storyNone"));
  checkRows(old, rows);
  const newIndex = new Map<number, number>();
  rows.forEach((r, k) => {
    if (r.from !== undefined) newIndex.set(old.findIndex((s) => s.id === r.from), k);
  });
  const deletedIdx = new Set(old.map((_, i) => i).filter((i) => !newIndex.has(i)));
  const idx = storyIndices(m, old);

  // 1. The structure of deleted stories.
  const deleted = deletedStructure(m, idx, deletedIdx);
  let model = deleted.frames.length + deleted.links.length + deleted.areas.length + deleted.nodes.length ? deleteObjects(m, deleted.nodes, deleted.frames, deleted.links, deleted.areas) : clone(m);

  // 2. New heights: each joint keeps its distance to its story's level; a joint of a deleted story keeps
  // its distance to the deleted level, measured from the nearest kept level below. Every joint must
  // stay above the level below it (a story cannot get lower than the joints between its levels).
  const before = new Map(m.nodes.map((n) => [n.id, n.z]));
  const lengths = new Map(model.frames.map((f) => [f.id, memberLength(model, f.id)]));
  const target = (i: number): number => {
    for (let k = i; k >= 0; k--) if (newIndex.has(k)) return newIndex.get(k)!;
    return 0;
  };
  const levels = rows.map((r) => ({ id: r.id.trim(), z: r.z }));
  const survivors: Id[] = [];
  for (const n of model.nodes) {
    const i = idx.nodes.get(n.id)!;
    const k = target(i);
    const z = round(levels[k].z + (n.z - old[i].z));
    if (k > 0 && !(z > levels[k - 1].z + SAME_POINT)) throw new EditError(t("err.storyJoint", { joint: n.id, story: levels[k].id }));
    if (deletedIdx.has(i)) survivors.push(n.id);
    n.z = z;
  }

  // 3. Frame loads keep their fraction of the length.
  keepLoadFractions(model, lengths);

  // 4. Stored meshes move with their area when it moves as a whole; an area that changes shape loses
  // its stored mesh (it is meshed again).
  const meshesCleared: Id[] = [];
  for (const a of model.areas) {
    if (!a.user_mesh) continue;
    const dz = a.nodes.map((id) => nodeById(model, id)!.z - before.get(id)!);
    if (dz.every((d) => Math.abs(d - dz[0]) <= SAME_POINT)) {
      if (Math.abs(dz[0]) > SAME_POINT) for (const p of a.user_mesh.joints) p[2] = round(p[2] + dz[0]);
    } else {
      delete a.user_mesh;
      meshesCleared.push(a.id);
    }
  }

  // 5. Joints of deleted stories that land on a joint already there join it.
  const merged = new Map<Id, Id>();
  for (const id of survivors) {
    const n = nodeById(model, id)!;
    const other = model.nodes.find((x) => x.id !== id && !merged.has(x.id) && dist(pos(x), pos(n)) <= SAME_POINT);
    if (other) merged.set(id, other.id);
  }
  if (merged.size) model = mergeJoints(model, merged);

  // 6. Story names in user wind loads; rows on deleted stories go.
  const rename = new Map<Id, Id>();
  rows.forEach((r) => r.from !== undefined && rename.set(r.from, r.id.trim()));
  let windRowsRemoved = 0;
  for (const p of model.load_patterns) {
    if (!p.wind_loads) continue;
    const kept = p.wind_loads.filter((w) => rename.has(w.story));
    windRowsRemoved += p.wind_loads.length - kept.length;
    p.wind_loads = kept.map((w) => ({ ...w, story: rename.get(w.story)! }));
  }

  return { model: setStoryLevels(model, levels), deleted, merged, meshesCleared, windRowsRemoved };
}


