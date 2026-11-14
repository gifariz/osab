// Labels: names by plan position, the same on every story. An object's label is a letter by its type
// (J joints, C / B / D frames by direction, null frames too, L links, F / W areas by slope, openings and
// null areas too) and a number; objects with the same key on several stories share it, so label and
// story name one object. Rules (docs/SOURCES.md):
// - the key is the letter and the object's joints in any order, each as its X and Y, how many stories
//   it lies below the object's story, and its distance to its own story's level. So a beam A-B is B-A,
//   the two diagonals of an X brace differ, a column spanning two stories differs from a one-story one,
//   a joint between levels differs from the joint at the level, floors match in any corner order.
// - labels are stored with the objects and kept: a label belongs to the key most of its objects have;
//   an object drawn under one on another story takes its label, an object moved onto another's key
//   takes that label, one alone on its key keeps its label; new keys get the next number of their
//   letter. Label and story stay unique (a second object with the same key on the same story gets its
//   own number).

import { pos } from "./geometry";
import { FRAME_PREFIX, angleLimitsOf, areaLabelPrefix, lineType } from "./objectTypes";
import { storyIndexAt, storyLevels } from "./stories";
import type { Id, Model, Vec3 } from "./types";

/** Label letters of joints and links (frames and areas: by their type). */
export const JOINT_LABEL = "J";
export const LINK_LABEL = "L";

const LABEL = /^([A-Z]+)(\d+)$/;
const grid = (v: number) => Math.round(v * 1e6);

type Kind = "nodes" | "frames" | "links" | "areas";
interface Item {
  kind: Kind;
  index: number;
  prefix: string;
  key: string;
  story: number;
  label?: string;
}

/** The label every object should have: index by kind, in model order. */
function labelPlan(m: Model): Item[] {
  const levels = storyLevels(m);
  const at = new Map(m.nodes.map((n) => [n.id, n]));
  const storyOf = (z: number) => (levels.length ? storyIndexAt(levels, z) : 0);
  const levelOf = (k: number) => levels[k]?.z ?? 0;
  const lim = angleLimitsOf(m);
  const items: Item[] = [];
  /** Key and story of an object on these joints; null when a joint is missing. */
  const keyOf = (prefix: string, ids: Id[]): { key: string; story: number } | null => {
    const ns = ids.map((id) => at.get(id));
    if (ns.some((n) => !n)) return null;
    const stories = ns.map((n) => storyOf(n!.z));
    const story = Math.max(...stories);
    const parts = ns.map((n, k) => [grid(n!.x), grid(n!.y), story - stories[k], grid(n!.z - levelOf(stories[k]))].join(","));
    return { key: prefix + ":" + parts.sort().join(";"), story };
  };
  const push = (kind: Kind, index: number, prefix: string, ids: Id[], label: string | undefined) => {
    const k = keyOf(prefix, ids);
    if (k) items.push({ kind, index, prefix, ...k, label });
  };
  const points = (ids: Id[]): Vec3[] | null => (ids.every((id) => at.has(id)) ? ids.map((id) => pos(at.get(id)!)) : null);
  m.nodes.forEach((n, i) => push("nodes", i, JOINT_LABEL, [n.id], n.label));
  m.frames.forEach((f, i) => {
    const p = points(f.nodes);
    if (p) push("frames", i, FRAME_PREFIX[lineType(p[0], p[1], lim)], f.nodes, f.label);
  });
  m.links.forEach((l, i) => push("links", i, LINK_LABEL, l.nodes, l.label));
  m.areas.forEach((a, i) => {
    const p = points(a.nodes);
    if (p) push("areas", i, areaLabelPrefix(p, lim), a.nodes, a.label);
  });

  // 1. Each label in use belongs to the key most of its holders have (the first holder's on a tie).
  const holders = new Map<string, Item[]>();
  const next = new Map<string, number>(); // letter -> next number
  for (const it of items) {
    const p = it.label ? LABEL.exec(it.label) : null;
    if (!p || p[1] !== it.prefix) continue;
    holders.set(it.label!, [...(holders.get(it.label!) ?? []), it]);
    next.set(p[1], Math.max(next.get(p[1]) ?? 1, parseInt(p[2], 10) + 1));
  }
  const owned = new Map<string, { label: string; count: number }[]>(); // key -> its labels
  for (const [label, hs] of holders) {
    const count = new Map<string, number>();
    for (const h of hs) count.set(h.key, (count.get(h.key) ?? 0) + 1);
    let owner = hs[0].key;
    for (const [k, c] of count) if (c > count.get(owner)!) owner = k;
    owned.set(owner, [...(owned.get(owner) ?? []), { label, count: count.get(owner)! }]);
  }
  // A key with several labels (an object moved onto another's key, or two objects with one key on a
  // story) uses the one most objects have first (the lowest number on a tie).
  const num = (l: string) => parseInt(LABEL.exec(l)![2], 10);
  for (const list of owned.values()) list.sort((a, b) => b.count - a.count || num(a.label) - num(b.label));
  // 2. In model order, every object takes its key's first label free on its story, or the next number
  // of its letter.
  const taken = new Set<string>(); // label@story
  for (const it of items) {
    const list = owned.get(it.key) ?? [];
    let label = list.find((l) => !taken.has(l.label + "@" + it.story))?.label;
    if (!label) {
      const n = next.get(it.prefix) ?? 1;
      next.set(it.prefix, n + 1);
      label = it.prefix + n;
      owned.set(it.key, [...list, { label, count: 0 }]);
    }
    taken.add(label + "@" + it.story);
    it.label = label;
  }
  return items;
}

/**
 * The model with every object's label up to date (the same model when nothing changes). Run after
 * every edit and when a file is opened; old files and examples get labels in file order.
 */
export function withLabels(m: Model): Model {
  const items = labelPlan(m);
  const want: Record<Kind, Map<number, string>> = { nodes: new Map(), frames: new Map(), links: new Map(), areas: new Map() };
  for (const it of items) want[it.kind].set(it.index, it.label!);
  const update = <T extends { label?: string }>(list: T[], kind: Kind): T[] | null => {
    let changed = false;
    const out = list.map((o, i) => {
      const label = want[kind].get(i);
      if (label === o.label) return o;
      changed = true;
      const copy = { ...o };
      if (label) copy.label = label;
      else delete copy.label;
      return copy;
    });
    return changed ? out : null;
  };
  const nodes = update(m.nodes, "nodes");
  const frames = update(m.frames, "frames");
  const links = update(m.links, "links");
  const areas = update(m.areas, "areas");
  if (!nodes && !frames && !links && !areas) return m;
  return { ...m, nodes: nodes ?? m.nodes, frames: frames ?? m.frames, links: links ?? m.links, areas: areas ?? m.areas };
}

/** Objects with the same label as these, on every story (One story / All stories selection). */
export function sameLabel(m: Model, sel: { nodes: Id[]; frames: Id[]; links: Id[]; areas: Id[] }): { nodes: Id[]; frames: Id[]; links: Id[]; areas: Id[] } {
  const pick = <T extends { id: Id; label?: string }>(list: T[], ids: Id[]): Id[] => {
    const set = new Set(ids);
    const labels = new Set(list.filter((o) => set.has(o.id) && o.label).map((o) => o.label));
    return list.filter((o) => set.has(o.id) || (o.label && labels.has(o.label))).map((o) => o.id);
  };
  return { nodes: pick(m.nodes, sel.nodes), frames: pick(m.frames, sel.frames), links: pick(m.links, sel.links), areas: pick(m.areas, sel.areas) };
}
