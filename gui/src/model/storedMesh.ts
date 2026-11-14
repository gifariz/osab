// Stored meshes (user mesh, Edit > Store Mesh): an area keeps the mesh the mesh step gave it, so
// it stays fixed whatever the mesh settings; the mesh step uses it as it is (mesh/src/stored.rs) and
// lines its neighbours up with it. The checks here are the mesh step's.

import { t } from "../i18n";
import { areaAxes, insidePolygon, isFloor } from "./area";
import { dist, dot, pos, sub } from "./geometry";
import { angleLimitsOf } from "./objectTypes";
import { modelTol } from "./overlaps";
import type { Area, Id, MeshInfo, Model, UserMesh, Vec3 } from "./types";

const clone = <T>(v: T): T => structuredClone(v);

function corners(m: Model, a: Area): Vec3[] | null {
  const p: Vec3[] = [];
  for (const id of a.nodes) {
    const n = m.nodes.find((x) => x.id === id);
    if (!n) return null;
    p.push(pos(n));
  }
  return p;
}

/** Whether an area can keep a mesh: it has elements (not an opening or null area), not a Membrane floor. */
export function canStoreMesh(m: Model, a: Area): boolean {
  if (!a.section || a.opening) return false;
  const p = corners(m, a);
  const membrane = m.area_sections.find((s) => s.id === a.section)?.type === "membrane";
  return !!p && !(membrane && isFloor(p, angleLimitsOf(m).beam_floor));
}

/**
 * The mesh the mesh step gave an area (its elements in `mesh`, or the area itself when it is one
 * element), as a mesh to store; null when it has none.
 */
export function userMeshOf(m: Model, mesh: MeshInfo, areaId: Id): UserMesh | null {
  const a = m.areas.find((x) => x.id === areaId);
  if (!a || !canStoreMesh(m, a)) return null;
  const at = new Map<Id, Vec3>(m.nodes.map((n) => [n.id, pos(n)]));
  for (const j of mesh.joints) at.set(j.id, [j.x, j.y, j.z]);
  let elements = mesh.shells.filter((el) => el.parent === areaId).map((el) => el.nodes);
  if (!elements.length) {
    if (a.nodes.length > 4) return null;
    elements = [a.nodes];
  }
  const ids: Id[] = [];
  const out: UserMesh = { joints: [], elements: [] };
  for (const e of elements) {
    out.elements.push(
      e.map((id) => {
        let k = ids.indexOf(id);
        if (k < 0) {
          const p = at.get(id);
          if (!p) throw new Error(`unknown mesh joint ${id}`);
          ids.push(id);
          out.joints.push(p);
          k = ids.length - 1;
        }
        return k;
      }),
    );
  }
  return out;
}

/** Stores these meshes on their areas (Edit > Store Mesh). */
export function storeMeshes(m: Model, meshes: Map<Id, UserMesh>): Model {
  const model = clone(m);
  for (const a of model.areas) {
    const um = meshes.get(a.id);
    if (um) a.user_mesh = clone(um);
  }
  return model;
}

/** Removes the stored meshes of these areas (Edit > Remove Stored Mesh): they are auto-meshed again. */
export function removeStoredMeshes(m: Model, ids: Id[]): Model {
  const set = new Set(ids);
  const model = clone(m);
  for (const a of model.areas) if (set.has(a.id)) delete a.user_mesh;
  return model;
}

/** How an area is meshed, in words: stored (n elements), no mesh, its own size or the model's settings. */
export function areaMeshText(a: Area): string {
  if (a.user_mesh) return t("amesh.storedShort", { n: a.user_mesh.elements.length });
  if (a.mesh === "none") return t("amesh.noneShort");
  return a.mesh ? t("amesh.sizeShort", { size: a.mesh.size }) : t("amesh.defaultShort");
}

/** Why an area's stored mesh cannot be used (the mesh step's checks), or null. */
export function storedMeshProblem(m: Model, a: Area, tol = modelTol(m)): string | null {
  const um = a.user_mesh;
  if (!um) return null;
  const p = corners(m, a);
  if (!p) return null;
  if (!canStoreMesh(m, a)) return t("stored.noElements");
  const axes = areaAxes(p, a.angle ?? 0);
  const to2 = (q: Vec3): [number, number] => [dot(sub(q, p[0]), axes[0]), dot(sub(q, p[0]), axes[1])];
  const poly = p.map(to2);
  if (um.joints.some((q) => Math.abs(dot(sub(q, p[0]), axes[2])) > tol || !insidePolygon(to2(q), poly, tol))) return t("stored.outside");
  // A joint at every corner that no opening covers (where one does, the mesh goes round the opening).
  const inPlane = (h: Vec3[] | null): h is Vec3[] => !!h && h.every((q) => Math.abs(dot(sub(q, p[0]), axes[2])) <= tol);
  const holes = m.areas.filter((o) => o.opening).map((o) => corners(m, o)).filter(inPlane).map((h) => h.map(to2));
  if (p.some((c) => !um.joints.some((q) => dist(q, c) <= tol) && !holes.some((h) => insidePolygon(to2(c), h, tol)))) return t("stored.cornerMoved");
  if (um.elements.some((e) => e.length < 3 || e.length > 4 || new Set(e).size !== e.length || e.some((k) => !(k >= 0 && k < um.joints.length)))) {
    return t("stored.badElement");
  }
  return null;
}
