// Renaming objects: new names for joints, frames, links and areas, carried into everything that refers
// to them (members' and areas' joints, supports, springs, joint masses and loads). Used to give the
// bundled example models the names of objects drawn in the app (1, M, K, A and a number).

import { AREA_NAME, FRAME_NAME, LINK_NAME } from "./ops";
import type { Id, Model } from "./types";

/** Old name -> new name, per kind; names not listed stay. */
export interface Renames {
  nodes: Map<Id, Id>;
  frames: Map<Id, Id>;
  links: Map<Id, Id>;
  areas: Map<Id, Id>;
}

const renamer = (map: Map<Id, Id>) => (id: Id) => map.get(id) ?? id;

function assertUnique(ids: Id[], kind: string) {
  const seen = new Set<Id>();
  for (const id of ids) {
    if (seen.has(id)) throw new Error(`Renaming gives two ${kind} named ${id}`);
    seen.add(id);
  }
}

/** The model with the objects renamed; frames and links share one set of names. */
export function renameObjects(m: Model, r: Renames): Model {
  const node = renamer(r.nodes);
  const frame = renamer(r.frames);
  const link = renamer(r.links);
  const area = renamer(r.areas);
  const out: Model = {
    ...m,
    nodes: m.nodes.map((n) => ({ ...n, id: node(n.id) })),
    frames: m.frames.map((f) => ({ ...f, id: frame(f.id), nodes: [node(f.nodes[0]), node(f.nodes[1])] })),
    links: m.links.map((l) => ({ ...l, id: link(l.id), nodes: [node(l.nodes[0]), node(l.nodes[1])] })),
    areas: m.areas.map((a) => ({ ...a, id: area(a.id), nodes: a.nodes.map(node) })),
    supports: m.supports.map((s) => ({ ...s, node: node(s.node) })),
    springs: m.springs.map((s) => ({ ...s, node: node(s.node) })),
    load_patterns: m.load_patterns.map((p) => ({
      ...p,
      node_loads: p.node_loads.map((l) => ({ ...l, node: node(l.node) })),
      frame_loads: p.frame_loads.map((l) => ({ ...l, frame: frame(l.frame) })),
      area_loads: p.area_loads.map((l) => ({ ...l, area: area(l.area) })),
    })),
  };
  if (m.masses) out.masses = m.masses.map((x) => ({ ...x, node: node(x.node) }));
  assertUnique(out.nodes.map((n) => n.id), "joints");
  assertUnique([...out.frames.map((f) => f.id), ...out.links.map((l) => l.id)], "frames or links");
  assertUnique(out.areas.map((a) => a.id), "areas");
  return out;
}

/** New names for every object in file order, as objects drawn in the app get: joints 1…, frames M1…, links K1…, areas A1…. */
export function namesByKind(m: Model): Renames {
  const named = (ids: Id[], prefix: string) => new Map(ids.map((id, k) => [id, prefix + (k + 1)]));
  return {
    nodes: named(m.nodes.map((n) => n.id), ""),
    frames: named(m.frames.map((f) => f.id), FRAME_NAME),
    links: named(m.links.map((l) => l.id), LINK_NAME),
    areas: named(m.areas.map((a) => a.id), AREA_NAME),
  };
}
