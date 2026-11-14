// The analysis mesh in the drawing data: positions of model and mesh joints (`~1`…), the elements' edges
// (View > Show Analysis Mesh), and the object-type display filter (View > Show Object Types).

import { objectTypes } from "../model/objectTypes";
import { pos } from "../model/geometry";
import type { Id, MeshInfo, Model, Vec3 } from "../model/types";

/** Coordinates of every joint: the model's and those the mesh step added. */
export function jointPositions(m: Model, mesh?: MeshInfo | null): Map<Id, Vec3> {
  const out = new Map<Id, Vec3>(m.nodes.map((n) => [n.id, pos(n)]));
  for (const j of mesh?.joints ?? []) out.set(j.id, [j.x, j.y, j.z]);
  return out;
}

/** Corner coordinates of a mesh element (or an area), or null when a joint is unknown. */
export function cornersOf(at: Map<Id, Vec3>, nodes: Id[]): Vec3[] | null {
  const p: Vec3[] = [];
  for (const n of nodes) {
    const q = at.get(n);
    if (!q) return null;
    p.push(q);
  }
  return p;
}

/** Edges of the mesh's shell elements whose parent area passes `shown`, each edge once. */
export function meshEdges(m: Model, mesh: MeshInfo, shown: (area: Id) => boolean): [Vec3, Vec3][] {
  const at = jointPositions(m, mesh);
  const seen = new Set<string>();
  const out: [Vec3, Vec3][] = [];
  for (const el of mesh.shells) {
    if (!shown(el.parent)) continue;
    const n = el.nodes;
    for (let k = 0; k < n.length; k++) {
      const [a, b] = [n[k], n[(k + 1) % n.length]];
      const key = a < b ? `${a}|${b}` : `${b}|${a}`;
      const [pa, pb] = [at.get(a), at.get(b)];
      if (seen.has(key) || !pa || !pb) continue;
      seen.add(key);
      out.push([pa, pb]);
    }
  }
  return out;
}

/** Edges of the stored meshes (Edit > Store Mesh) of the areas that pass `shown`, each edge once. */
export function storedEdges(m: Model, shown: (area: Id) => boolean): [Vec3, Vec3][] {
  const out: [Vec3, Vec3][] = [];
  for (const a of m.areas) {
    const um = a.user_mesh;
    if (!um || !shown(a.id)) continue;
    const seen = new Set<string>();
    for (const e of um.elements) {
      for (let k = 0; k < e.length; k++) {
        const [i, j] = [e[k], e[(k + 1) % e.length]];
        const key = i < j ? `${i}|${j}` : `${j}|${i}`;
        if (seen.has(key) || !um.joints[i] || !um.joints[j]) continue;
        seen.add(key);
        out.push([um.joints[i], um.joints[j]]);
      }
    }
  }
  return out;
}

/** Frames and areas left out by the display filter (types like "frame:column", "area:opening"). */
export function filteredOut(m: Model, hiddenTypes: string[] | undefined): { frames: Set<Id>; areas: Set<Id> } {
  const frames = new Set<Id>();
  const areas = new Set<Id>();
  if (!hiddenTypes?.length) return { frames, areas };
  const hide = new Set(hiddenTypes);
  const types = objectTypes(m);
  for (const [id, ty] of types.frames) if (hide.has(`frame:${ty}`)) frames.add(id);
  for (const [id, ty] of types.areas) if (hide.has(`area:${ty}`)) areas.add(id);
  return { frames, areas };
}
