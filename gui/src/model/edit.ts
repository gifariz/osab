// Edit > Move and Edit > Replicate. Rules (docs/SOURCES.md):
// - Move: selected joints move and stretch the members attached to them. A selected frame, link or area
//   takes along the joints that only moving objects use; a joint it shares with an object that stays is
//   left there, and the moved object gets a joint at the new place (one already there, else a new one).
//   A moved joint that lands on a joint is merged into it (the joint already there stays). Frame loads on
//   stretched members keep their fraction of the length.
// - Replicate: copies of the selected objects (linear, radial about a vertical axis, mirrored about a
//   vertical plane, or to other stories). Copies use joints already at their corners; a copy that would
//   be the same as an object already there is skipped. Assignments of joints (supports, springs, masses,
//   joint loads) are copied for selected joints only. "Delete original objects" deletes the selection
//   afterwards, with the joints it leaves alone (not special ones).

import { t } from "../i18n";
import { shapeProblem } from "./area";
import { storedOrder } from "./areaOps";
import { add, drawOrderReversed, norm, pos, sub } from "./geometry";
import { angleLimitsOf, lineType } from "./objectTypes";
import {
  AREA_NAME,
  EditError,
  FRAME_NAME,
  LINK_NAME,
  SAME_POINT,
  addNode,
  deleteObjects,
  keepLoadFractions,
  memberLength,
  mergeJoints,
  nextNumber,
  nodeById,
  oneElement,
} from "./ops";
import { storyIndexAt, storyLevels } from "./stories";
import type { Area, Frame, FrameLoad, Id, Link, Model, ModelNode, Vec3 } from "./types";

const clone = <T>(v: T): T => structuredClone(v);
/** Rounded to 1e-9 (and no -0). */
const round = (v: number) => Math.round(v * 1e9) / 1e9 + 0;

/** Objects by kind (the shape of the app's selection). */
export interface ObjectSet {
  nodes: Id[];
  frames: Id[];
  links: Id[];
  areas: Id[];
}

export interface MoveResult {
  model: Model;
  /** Moved joints merged into joints already at their new place. */
  merged: number;
  /** Areas that lost their stored mesh (stretched). */
  meshesCleared: Id[];
}

/** Members, links and areas must keep a shape that can be analysed after an edit of these joints. */
function checkShapes(m: Model, changed: Set<Id>) {
  for (const o of [...m.frames, ...m.links]) {
    if (!o.nodes.some((n) => changed.has(n))) continue;
    const [a, b] = o.nodes.map((n) => nodeById(m, n)!);
    if (o.nodes[0] === o.nodes[1] || norm(sub(pos(a), pos(b))) <= SAME_POINT) throw new EditError(t("err.zeroLength", { id: o.id }));
  }
  for (const a of m.areas) {
    if (!a.nodes.some((n) => changed.has(n))) continue;
    const problem = new Set(a.nodes).size < a.nodes.length ? t("err.areaCollapsed") : shapeProblem(a.nodes.map((x) => pos(nodeById(m, x)!)), !oneElement(m, a));
    if (problem) throw new EditError(t("err.areaShape", { problem }));
  }
}

/** Edit > Move: moves the selection by `d` (rules at the top of this file). One edit. */
export function moveObjects(m: Model, sel: ObjectSet, d: Vec3): MoveResult {
  if (norm(d) <= SAME_POINT) throw new EditError(t("err.moveZero"));
  const [frames, links, areas] = [new Set(sel.frames), new Set(sel.links), new Set(sel.areas)];
  const usedMoving = new Set<Id>();
  const usedStaying = new Set<Id>();
  const each = (o: { id: Id; nodes: Id[] }, moving: boolean) => o.nodes.forEach((n) => (moving ? usedMoving : usedStaying).add(n));
  m.frames.forEach((o) => each(o, frames.has(o.id)));
  m.links.forEach((o) => each(o, links.has(o.id)));
  m.areas.forEach((o) => each(o, areas.has(o.id)));
  const moving = new Set(sel.nodes);
  for (const n of usedMoving) if (!usedStaying.has(n)) moving.add(n);
  const shared = [...usedMoving].filter((n) => !moving.has(n));
  if (!moving.size && !shared.length) throw new EditError(t("err.moveNothing"));

  let model = clone(m);
  const lengths = new Map(model.frames.map((f) => [f.id, memberLength(model, f.id)]));
  const at = (n: ModelNode) => add(pos(n), d).map(round) as Vec3;
  for (const n of model.nodes) if (moving.has(n.id)) [n.x, n.y, n.z] = at(n);

  // A shared joint stays; the moved objects get a joint at its new place.
  const remap = new Map<Id, Id>();
  for (const id of shared) {
    const r = addNode(model, at(nodeById(m, id)!));
    model = r.model;
    remap.set(id, r.id);
  }
  const to = (id: Id) => remap.get(id) ?? id;
  for (const f of model.frames) if (frames.has(f.id)) f.nodes = [to(f.nodes[0]), to(f.nodes[1])];
  for (const l of model.links) if (links.has(l.id)) l.nodes = [to(l.nodes[0]), to(l.nodes[1])];
  for (const a of model.areas) if (areas.has(a.id)) a.nodes = a.nodes.map(to);

  // Moved joints that land on a joint are merged into it.
  const into = new Map<Id, Id>();
  for (const id of moving) {
    const n = nodeById(model, id)!;
    const other = model.nodes.find((x) => x.id !== id && !moving.has(x.id) && norm(sub(pos(x), pos(n))) <= SAME_POINT);
    if (!other) continue;
    into.set(id, other.id);
    if (n.special) other.special = true;
  }
  if (into.size) model = mergeJoints(model, into);
  const changed = new Set([...moving].map((id) => into.get(id) ?? id).concat([...remap.values()]));
  checkShapes(model, changed);
  keepLoadFractions(model, lengths);

  // Stored meshes move with an area that moves; a stretched area loses its stored mesh.
  const meshesCleared: Id[] = [];
  for (const a of model.areas) {
    if (!a.user_mesh) continue;
    if (areas.has(a.id)) a.user_mesh.joints = a.user_mesh.joints.map((p) => add(p, d).map(round) as Vec3);
    else if (a.nodes.some((n) => changed.has(n))) {
      delete a.user_mesh;
      meshesCleared.push(a.id);
    }
  }
  return { model, merged: into.size, meshesCleared };
}

// ---- Replicate ----

export type ReplicateSpec =
  | { kind: "linear"; offset: Vec3; copies: number }
  /** About the vertical axis through `center` (x, y); degrees, counterclockwise seen from above. */
  | { kind: "radial"; center: [number, number]; angle: number; copies: number; rotateAxes: boolean }
  /** About the vertical plane through two points (x, y). */
  | { kind: "mirror"; a: [number, number]; b: [number, number] }
  /** To these stories: each joint keeps its distance to its story's level, stories shifted alike. */
  | { kind: "story"; stories: Id[] };

export interface ReplicateOptions {
  /** Supports, springs and masses of selected joints. */
  supports: boolean;
  /** Joint loads of selected joints, frame and area loads, in every pattern. */
  loads: boolean;
  /** Diaphragms of selected joints and of areas. */
  diaphragms: boolean;
  /** Delete the selection afterwards. */
  deleteOriginals: boolean;
}

export const DEFAULT_REPLICATE_OPTIONS: ReplicateOptions = { supports: true, loads: true, diaphragms: true, deleteOriginals: false };

export interface ReplicateResult {
  model: Model;
  /** The copies (joints copied as selected joints, and new frames, links and areas). */
  created: ObjectSet;
  /** Copies left out: the same object was already there, or a story copy would go above the top. */
  skipped: number;
}

/** One copy: where a joint goes (null: outside the stories), the turn of local axes, and the mesh points. */
interface Placement {
  place(p: Vec3, story: number): Vec3 | null;
  /** Rigid copies (not Story): where a stored mesh point goes. */
  rigid?: (p: Vec3) => Vec3;
  /** Degrees added to the angle of columns and floors (Radial with Rotate local axes). */
  turn: number;
  /** A mirror image (areas' corner order and mesh elements reversed). */
  mirrored: boolean;
}

function placements(m: Model, spec: ReplicateSpec): Placement[] {
  const many = (n: number) => {
    if (!Number.isInteger(n) || n < 1 || n > 1000) throw new EditError(t("err.copies"));
    return Array.from({ length: n }, (_, i) => i + 1);
  };
  if (spec.kind === "linear") {
    if (norm(spec.offset) <= SAME_POINT) throw new EditError(t("err.moveZero"));
    return many(spec.copies).map((k) => {
      const f = (p: Vec3) => add(p, spec.offset.map((v) => v * k) as Vec3);
      return { place: f, rigid: f, turn: 0, mirrored: false };
    });
  }
  if (spec.kind === "radial") {
    if (!(Math.abs(spec.angle) > 1e-9)) throw new EditError(t("err.radialAngle"));
    const [cx, cy] = spec.center;
    return many(spec.copies).map((k) => {
      const a = (spec.angle * k * Math.PI) / 180;
      const [c, s] = [Math.cos(a), Math.sin(a)];
      const f = (p: Vec3): Vec3 => [cx + (p[0] - cx) * c - (p[1] - cy) * s, cy + (p[0] - cx) * s + (p[1] - cy) * c, p[2]];
      return { place: f, rigid: f, turn: spec.rotateAxes ? spec.angle * k : 0, mirrored: false };
    });
  }
  if (spec.kind === "mirror") {
    const [ax, ay] = spec.a;
    const [dx, dy] = [spec.b[0] - ax, spec.b[1] - ay];
    const len2 = dx * dx + dy * dy;
    if (len2 <= SAME_POINT * SAME_POINT) throw new EditError(t("err.mirrorLine"));
    const f = (p: Vec3): Vec3 => {
      const s = ((p[0] - ax) * dx + (p[1] - ay) * dy) / len2;
      const [fx, fy] = [ax + s * dx, ay + s * dy];
      return [2 * fx - p[0], 2 * fy - p[1], p[2]];
    };
    return [{ place: f, rigid: f, turn: 0, mirrored: true }];
  }
  const levels = storyLevels(m);
  if (!spec.stories.length) throw new EditError(t("err.storyPick"));
  return spec.stories.map((id) => {
    const target = levels.findIndex((s) => s.id === id);
    if (target < 0) throw new EditError(t("err.storyUnknown", { story: id }));
    return {
      // `story`: the index of the object's own story; each joint goes `target - story` stories up.
      place: (p: Vec3, story: number) => {
        const i = storyIndexAt(levels, p[2]);
        const k = i + target - story;
        if (k < 0 || k >= levels.length) return null;
        return [p[0], p[1], levels[k].z + (p[2] - levels[i].z)];
      },
      turn: 0,
      mirrored: false,
    };
  });
}

/** Turns a member end for end: nodes, releases, end offsets and the positions of its loads. */
function reverseFrame(f: Frame, loads: FrameLoad[], length: number) {
  f.nodes = [f.nodes[1], f.nodes[0]];
  if (f.releases) f.releases = { i: f.releases.j, j: f.releases.i };
  if (f.end_offsets && f.end_offsets !== "auto") f.end_offsets = { i: f.end_offsets.j, j: f.end_offsets.i };
  const flip = (v: number) => round(length - v);
  for (const l of loads) {
    if (l.type === "point") l.at = flip(l.at);
    else {
      const [from, to] = [l.from, l.to];
      l.from = to === undefined ? undefined : flip(to);
      l.to = from === undefined ? undefined : flip(from);
      if (l.from === undefined) delete l.from;
      if (l.to === undefined) delete l.to;
      if (l.type === "trapezoidal") [l.w1, l.w2] = [l.w2, l.w1];
    }
  }
}

const sameSet = (a: Id[], b: Id[]) => a.length === b.length && a.every((x) => b.includes(x));

/** Edit > Replicate: copies of the selection (rules at the top of this file). One edit. */
export function replicateObjects(m: Model, sel: ObjectSet, spec: ReplicateSpec, opts: ReplicateOptions = DEFAULT_REPLICATE_OPTIONS): ReplicateResult {
  if (!sel.nodes.length && !sel.frames.length && !sel.links.length && !sel.areas.length) throw new EditError(t("err.moveNothing"));
  const copies = placements(m, spec);
  const levels = storyLevels(m);
  const storyOf = (ids: Id[]) => (levels.length ? Math.max(...ids.map((id) => storyIndexAt(levels, nodeById(m, id)!.z))) : 0);
  const lim = angleLimitsOf(m);
  let model = clone(m);
  const created: ObjectSet = { nodes: [], frames: [], links: [], areas: [] };
  let skipped = 0;
  const selectedJoints = new Set(sel.nodes);

  for (const c of copies) {
    // Joints of this copy, by original joint (null: this copy of the joint is left out).
    const jointCopy = (id: Id, story: number): Id | null => {
      const n = nodeById(m, id)!;
      const p = c.place(pos(n), story);
      if (!p) return null;
      const r = addNode(model, p.map(round) as Vec3);
      model = r.model;
      if (r.created && selectedJoints.has(id)) {
        const copy = nodeById(model, r.id)!;
        if (n.special) copy.special = true;
        if (n.in_mesh) copy.in_mesh = true;
        if (opts.diaphragms && n.diaphragm) copy.diaphragm = n.diaphragm;
        else if (!opts.diaphragms) delete copy.diaphragm;
        if (opts.supports) {
          for (const key of ["supports", "springs", "masses"] as const) {
            const list = model[key] as { node: Id }[] | undefined;
            const own = list?.find((x) => x.node === id);
            if (list && own) list.push({ ...clone(own), node: r.id });
          }
        }
        if (opts.loads) for (const pat of model.load_patterns) for (const l of pat.node_loads.filter((x) => x.node === id)) pat.node_loads.push({ ...l, node: r.id });
        created.nodes.push(r.id);
      }
      return r.id;
    };
    const objectJoints = (nodes: Id[]): Id[] | null => {
      const story = storyOf(nodes);
      const out: Id[] = [];
      for (const n of nodes) {
        const id = jointCopy(n, story);
        if (id === null) return null;
        out.push(id);
      }
      return out;
    };

    for (const id of sel.nodes) if (jointCopy(id, storyOf([id])) === null) skipped++;

    for (const fid of sel.frames) {
      const f = m.frames.find((x) => x.id === fid)!;
      const nodes = objectJoints(f.nodes);
      if (!nodes || nodes[0] === nodes[1] || model.frames.some((x) => sameSet(x.nodes, nodes))) {
        skipped++;
        continue;
      }
      const copy: Frame = { ...clone(f), id: FRAME_NAME + nextNumber(model.frames.map((x) => x.id), FRAME_NAME), nodes: [nodes[0], nodes[1]] };
      delete copy.label;
      const [a, b] = nodes.map((n) => pos(nodeById(model, n)!));
      if (c.turn && lineType(a, b, lim) === "column") copy.angle = round(((copy.angle ?? 0) + c.turn) % 360) || undefined;
      if (copy.angle === undefined) delete copy.angle;
      const loads: FrameLoad[] = opts.loads ? model.load_patterns.flatMap((p) => p.frame_loads.filter((l) => l.frame === fid)) : [];
      const copied = loads.map((l) => ({ ...l, frame: copy.id }));
      if (drawOrderReversed(a, b)) reverseFrame(copy, copied, norm(sub(a, b)));
      model.frames.push(copy);
      if (opts.loads) {
        for (const p of model.load_patterns) {
          const own = p.frame_loads.filter((l) => l.frame === fid).length;
          if (own) p.frame_loads.push(...copied.splice(0, own));
        }
      }
      created.frames.push(copy.id);
    }

    for (const lid of sel.links) {
      const l = m.links.find((x) => x.id === lid)!;
      const nodes = objectJoints(l.nodes);
      if (!nodes || nodes[0] === nodes[1] || model.links.some((x) => sameSet(x.nodes, nodes))) {
        skipped++;
        continue;
      }
      const [a, b] = nodes.map((n) => pos(nodeById(model, n)!));
      const copy: Link = { ...clone(l), id: LINK_NAME + nextNumber([...model.links.map((x) => x.id), ...model.frames.map((x) => x.id)], LINK_NAME), nodes: drawOrderReversed(a, b) ? [nodes[1], nodes[0]] : [nodes[0], nodes[1]] };
      delete copy.label;
      model.links.push(copy);
      created.links.push(copy.id);
    }

    for (const aid of sel.areas) {
      const ar = m.areas.find((x) => x.id === aid)!;
      const nodes = objectJoints(ar.nodes);
      if (!nodes || new Set(nodes).size < nodes.length || model.areas.some((x) => sameSet(x.nodes, nodes))) {
        skipped++;
        continue;
      }
      const copy: Area = { ...clone(ar), id: AREA_NAME + nextNumber(model.areas.map((x) => x.id), AREA_NAME), nodes: storedOrder(model, nodes) };
      delete copy.label;
      if (!opts.diaphragms) delete copy.diaphragm;
      if (c.turn && !ar.opening && copy.nodes.every((n) => Math.abs(nodeById(model, n)!.z - nodeById(model, copy.nodes[0])!.z) <= SAME_POINT)) {
        copy.angle = round(((copy.angle ?? 0) + c.turn) % 360) || undefined;
        if (copy.angle === undefined) delete copy.angle;
      }
      if (copy.user_mesh) {
        if (c.rigid) {
          copy.user_mesh.joints = copy.user_mesh.joints.map((p) => c.rigid!(p).map(round) as Vec3);
          if (c.mirrored) copy.user_mesh.elements = copy.user_mesh.elements.map((e) => [...e].reverse());
        } else {
          // Story copies: the mesh moves when every corner moved by the same height.
          const dz = ar.nodes.map((n, k) => nodeById(model, nodes[k])!.z - nodeById(m, n)!.z);
          if (dz.every((v) => Math.abs(v - dz[0]) <= SAME_POINT)) copy.user_mesh.joints = copy.user_mesh.joints.map((p) => [p[0], p[1], round(p[2] + dz[0])] as Vec3);
          else delete copy.user_mesh;
        }
      }
      model.areas.push(copy);
      if (opts.loads) for (const p of model.load_patterns) for (const l of p.area_loads.filter((x) => x.area === aid)) p.area_loads.push({ ...l, area: copy.id });
      created.areas.push(copy.id);
    }
  }

  if (opts.deleteOriginals) {
    const kept = new Set<Id>([...created.frames, ...created.links, ...created.areas]);
    const usedByCopies = new Set<Id>();
    for (const o of [...model.frames, ...model.links, ...model.areas]) if (kept.has(o.id)) o.nodes.forEach((n) => usedByCopies.add(n));
    const nodes = sel.nodes.filter((n) => !usedByCopies.has(n) && !created.nodes.includes(n));
    model = deleteObjects(model, nodes, sel.frames, sel.links, sel.areas);
  }
  return { model, created, skipped };
}
