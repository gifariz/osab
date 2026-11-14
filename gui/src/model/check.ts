// Quick checks before running the analysis. Errors stop the run; warnings can be ignored; notes only
// inform (shown in Check Model, they do not hold up a run). The area checks follow the mesh step's rules.

import { t, type Key } from "../i18n";
import { areaAxes, poorShape, shapeProblem } from "./area";
import { dist, localAxes, pos } from "./geometry";
import { SAME_POINT, bodyRestraintConflict, noMeshPolygon, oneElement, unmeshedArea } from "./ops";
import { objectTypes } from "./objectTypes";
import { modelTol, openingHosts, overlaps, pointOnArea, solids } from "./overlaps";
import { storedMeshProblem } from "./storedMesh";
import type { Id, Model, ModelNode, Vec3 } from "./types";

/** The joints by id (the checks look joints up by id many times over; a list search each time is slow on large models). */
const jointsById = (m: Model) => new Map(m.nodes.map((n) => [n.id, n]));

export interface Issue {
  level: "error" | "warning" | "note";
  message: string;
  nodes?: Id[];
  frames?: Id[];
  areas?: Id[];
}

export function checkModel(m: Model): Issue[] {
  const issues: Issue[] = [];
  // Joints on springs alone (with masses or loads) are a structure too.
  if (m.frames.length === 0 && m.links.length === 0 && m.areas.length === 0 && m.springs.length === 0) issues.push({ level: "error", message: t("chk.noMembers") });
  if (m.supports.length === 0 && m.springs.length === 0) {
    issues.push({ level: "error", message: t("chk.noSupports") });
  }
  const conflict = bodyRestraintConflict(m);
  if (conflict) {
    issues.push({
      level: "error",
      message: t("chk.bodyConflict", { a: conflict[0], b: conflict[1] }),
      nodes: conflict,
    });
  }
  // A model with only a modal case is fine.
  if (m.load_cases.length === 0 && !m.modal) issues.push({ level: "error", message: t("chk.noCases") });
  // A case that applies no pattern has no loads (a pattern still empty, like Live in a new model, is fine).
  for (const c of m.load_cases) {
    if (!c.loads.some((l) => l.scale !== 0)) issues.push({ level: "warning", message: t("chk.emptyCase", { id: c.id }) });
  }

  const joints = jointsById(m);
  if (m.dimension !== "3D") issues.push(...planeIssues(m, m.dimension, joints));

  const used = new Set([
    ...m.frames.flatMap((e) => e.nodes),
    ...m.links.flatMap((l) => l.nodes),
    ...m.areas.flatMap((a) => a.nodes),
    ...m.springs.map((s) => s.node),
  ]);
  // A joint with only a joint load on a floor or wall passes the load into the element under it
  // (*Include in mesh* = no, the default), so it is not left loose.
  const tol = modelTol(m);
  const loaded = new Set(m.load_patterns.flatMap((c) => c.node_loads.map((l) => l.node)));
  const onElements = (id: Id) => {
    const n = joints.get(id);
    return !!n && m.areas.some((a) => a.section && !a.opening && pointOnArea(m, a, pos(n), tol));
  };
  const free = m.nodes.filter((n) => !used.has(n.id) && !(loaded.has(n.id) && onElements(n.id))).map((n) => n.id);
  if (free.length) {
    issues.push({ level: "warning", message: t("chk.freeJoints", { list: list(free) }), nodes: free });
  }

  const dupes = samePointPairs(m.nodes).flatMap(([i, j]) => [m.nodes[i].id, m.nodes[j].id]);
  if (dupes.length) {
    issues.push({ level: "warning", message: t("chk.samePoint", { list: list(dupes) }), nodes: dupes });
  }

  issues.push(...areaIssues(m, tol, joints));
  return issues;
}

/**
 * Pairs of joints at the same point (within SAME_POINT), in list order (i < j, by i then j). Joints are put in
 * cells of that size and only the 27 cells around each are compared, so it takes time in proportion to the joints.
 */
export function samePointPairs(nodes: ModelNode[]): [number, number][] {
  const cellOf = (n: ModelNode) => pos(n).map((v) => Math.floor(v / SAME_POINT));
  const cells = new Map<string, number[]>();
  nodes.forEach((n, i) => {
    const k = cellOf(n).join(",");
    const c = cells.get(k);
    if (c) c.push(i);
    else cells.set(k, [i]);
  });
  const pairs: [number, number][] = [];
  nodes.forEach((n, i) => {
    const [x, y, z] = cellOf(n);
    for (let dx = -1; dx <= 1; dx++)
      for (let dy = -1; dy <= 1; dy++)
        for (let dz = -1; dz <= 1; dz++)
          for (const j of cells.get(`${x + dx},${y + dy},${z + dz}`) ?? []) if (j > i && dist(pos(n), pos(nodes[j])) <= SAME_POINT) pairs.push([i, j]);
  });
  return pairs.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
}

/**
 * Areas, by the mesh step's rules: shapes (a meshed area may be any flat polygon whose edges do not cross;
 * one analysed as a single element needs 3 or 4 corners, convex), openings (inside a meshed floor or wall,
 * no loads) and overlaps (who gets elements; an unmeshed area may not overlap). Joints on
 * area edges need no message: edge constraints tie them to the edge.
 */
function areaIssues(m: Model, tol: number, joints: Map<Id, ModelNode>): Issue[] {
  const issues: Issue[] = [];
  const poor: Id[] = [];
  for (const a of m.areas) {
    const p = a.nodes.map((n) => joints.get(n)).filter((n) => n !== undefined).map(pos);
    if (p.length !== a.nodes.length || p.length < 3) continue;
    const single = oneElement(m, a);
    const problem = shapeProblem(p, !single, true);
    if (problem) {
      issues.push({ level: "error", message: t("chk.areaShape", { id: a.id, problem }), areas: [a.id], nodes: a.nodes });
      continue;
    }
    if (single && poorShape(p)) poor.push(a.id);
    const stale = storedMeshProblem(m, a, tol);
    if (stale) issues.push({ level: "error", message: t("chk.storedMesh", { id: a.id, problem: stale }), areas: [a.id] });
  }
  if (poor.length) issues.push({ level: "warning", message: t("chk.areaShapeWarn", { list: list(poor) }), areas: poor });
  const polygons = m.areas.filter((a) => !a.opening && !a.user_mesh && a.section && noMeshPolygon(a)).map((a) => a.id);
  if (polygons.length) issues.push({ level: "warning", message: t("chk.noMeshPolygon", { list: list(polygons) }), areas: polygons });

  // Openings.
  const areas = new Map(m.areas.map((a) => [a.id, a]));
  const openings = m.areas.filter((a) => a.opening);
  const solidAreas = openings.length ? solids(m) : [];
  for (const o of openings) {
    const hosts = openingHosts(m, o, tol, solidAreas);
    // Over stored meshes only (inside one, or across two floors), it is a hole in them already.
    const stored = hosts.every((h) => areas.get(h)?.user_mesh);
    const unmeshed = stored ? [] : hosts.filter((h) => { const a = areas.get(h); return !!a && unmeshedArea(a); });
    if (!hosts.length) issues.push({ level: "error", message: t("chk.openingOutside", { id: o.id }), areas: [o.id] });
    else if (unmeshed.length) issues.push({ level: "error", message: t("chk.openingUnmeshed", { id: o.id, list: list(unmeshed) }), areas: [o.id, ...unmeshed] });
  }
  const openingSet = new Set(openings.map((a) => a.id));
  const loadedOpenings = [...new Set(m.load_patterns.flatMap((c) => c.area_loads.filter((l) => openingSet.has(l.area)).map((l) => l.area)))];
  if (loadedOpenings.length) issues.push({ level: "error", message: t("chk.openingLoads", { list: list(loadedOpenings) }), areas: loadedOpenings });

  // Overlaps: one area gets elements where they overlap (two unmeshed areas: both in full).
  const over = overlaps(m, tol);
  const byId = areas;
  const wins: string[] = [];
  const bothFull: string[] = [];
  const bothFullIds = new Set<Id>();
  for (const o of over) {
    const loser = o.winner === o.a ? o.b : o.a;
    const isUnmeshed = (id: Id) => { const a = byId.get(id); return !!a && unmeshedArea(a); };
    // Both not meshed (one element each): both are analysed in full where they overlap.
    if (isUnmeshed(o.a) && isUnmeshed(o.b)) {
      bothFull.push(`${o.a}/${o.b}`);
      bothFullIds.add(o.a).add(o.b);
      continue;
    }
    const unmeshed = [o.a, o.b].find(isUnmeshed);
    if (unmeshed) {
      const other = unmeshed === o.a ? o.b : o.a;
      issues.push({ level: "error", message: t("chk.overlapUnmeshed", { id: unmeshed, other }), areas: [o.a, o.b] });
      continue;
    }
    wins.push(t(`chk.overlapWhy.${o.why}` as Key, { winner: o.winner, loser }));
  }
  // Property type against orientation: allowed (it changes no result) but usually a slip.
  const types = objectTypes(m).areas;
  const secType = new Map(m.area_sections.map((s) => [s.id, s.property_type ?? "slab"]));
  const wallOnFloor = m.areas.filter((a) => types.get(a.id) === "floor" && secType.get(a.section) === "wall").map((a) => a.id);
  const slabOnWall = m.areas.filter((a) => types.get(a.id) === "wall" && secType.get(a.section) !== "wall" && secType.has(a.section)).map((a) => a.id);
  if (wallOnFloor.length) issues.push({ level: "note", message: t("chk.wallSectionOnFloor", { list: list(wallOnFloor) }), areas: wallOnFloor });
  if (slabOnWall.length) issues.push({ level: "note", message: t("chk.slabSectionOnWall", { list: list(slabOnWall) }), areas: slabOnWall });
  if (bothFull.length) issues.push({ level: "note", message: t("chk.overlapBothFull", { list: list(bothFull) }), areas: [...bothFullIds] });
  if (wins.length) {
    issues.push({ level: "note", message: t("chk.overlaps", { list: wins.length > 8 ? t("chk.more", { list: wins.slice(0, 8).join("; "), n: wins.length - 8 }) : wins.join("; ") }) });
  }
  return issues;
}

/**
 * What a plane analysis keeps: the plane frame (X-Z: UX, UZ, RY) or the plane grid (X-Y: UZ, RX, RY).
 * `coord`: the coordinate all joints share; `forces`: joint loads it cannot take; `out`: global force
 * directions it cannot take; `normal`: the axis local 3 of its areas lies along.
 */
const PLANES = {
  "2D": { coord: 1, forces: ["fy", "mx", "mz"], out: [1], normal: 1, off: "chk.planeOff", loads: "chk.planeLoads", areas: "chk.planeAreas" },
  grid: { coord: 2, forces: ["fx", "fy", "mz"], out: [0, 1], normal: 2, off: "chk.gridOff", loads: "chk.gridLoads", areas: "chk.gridAreas" },
} as const;

/** A plane analysis needs every joint in its plane, areas in that plane and no loads out of it. */
function planeIssues(m: Model, dim: keyof typeof PLANES, joints: Map<Id, ModelNode>): Issue[] {
  const p = PLANES[dim];
  const issues: Issue[] = [];
  const fix = t("chk.planeFix");
  const v0 = m.nodes[0] ? pos(m.nodes[0])[p.coord] : 0;
  const off = m.nodes.filter((n) => Math.abs(pos(n)[p.coord] - v0) > SAME_POINT).map((n) => n.id);
  if (off.length) {
    issues.push({ level: "error", message: t(p.off, { y: v0, z: v0, list: list(off), fix }), nodes: off });
  }
  // Areas must lie in the plane (local 3 along its normal).
  const areas = new Map(m.areas.map((a) => [a.id, a]));
  const frames = new Map(m.frames.map((f) => [f.id, f]));
  const axesOf = (id: Id) => {
    const a = areas.get(id);
    const q = a?.nodes.map((n) => joints.get(n)).filter((n) => n !== undefined).map(pos);
    return a && q && q.length === a.nodes.length ? areaAxes(q, a.angle ?? 0) : null;
  };
  const tilted = m.areas.filter((a) => {
    const axes = axesOf(a.id);
    return axes && Math.abs(Math.abs(axes[2][p.normal]) - 1) > 1e-9;
  });
  if (tilted.length) {
    const ids = tilted.map((a) => a.id);
    issues.push({ level: "error", message: t(p.areas, { list: list(ids) }) + " " + fix, areas: ids });
  }
  // A load direction as a global vector (local directions by the member's or area's axes).
  const vec = (dir: string, axes: [Vec3, Vec3, Vec3]): Vec3 =>
    dir === "X" ? [1, 0, 0] : dir === "Y" ? [0, 1, 0] : dir === "Z" ? [0, 0, 1] : axes[Number(dir) - 1];
  const outOfPlane = (v: Vec3) => p.out.some((k) => Math.abs(v[k]) > 1e-9);
  const loadNodes: Id[] = [];
  const loadElems: Id[] = [];
  const loadAreas: Id[] = [];
  for (const c of m.load_patterns) {
    for (const l of c.node_loads) if (p.forces.some((f) => l[f])) loadNodes.push(l.node);
    for (const l of c.frame_loads) {
      const e = frames.get(l.frame);
      const a = e && joints.get(e.nodes[0]);
      const b = e && joints.get(e.nodes[1]);
      if (!e || !a || !b) continue;
      if (outOfPlane(vec(l.dir, localAxes(pos(a), pos(b), e.angle ?? 0)))) loadElems.push(e.id);
    }
    for (const l of c.area_loads) {
      const axes = axesOf(l.area);
      if (axes && outOfPlane(vec(l.dir, axes))) loadAreas.push(l.area);
    }
  }
  if (loadNodes.length || loadElems.length || loadAreas.length) {
    const parts = [
      loadNodes.length ? t("chk.planeLoadsJoints", { list: list(loadNodes) }) : "",
      loadElems.length ? t("chk.planeLoadsMembers", { list: list(loadElems) }) : "",
      loadAreas.length ? t("chk.planeLoadsAreas", { list: list(loadAreas) }) : "",
    ].filter(Boolean);
    issues.push({
      level: "error",
      message: t(p.loads, { parts: parts.join(t("chk.and")), fix }),
      nodes: loadNodes,
      frames: loadElems,
      areas: loadAreas,
    });
  }
  return issues;
}

const list = (ids: Id[]) => {
  const u = [...new Set(ids)];
  return u.length > 8 ? t("chk.more", { list: u.slice(0, 8).join(", "), n: u.length - 8 }) : u.join(", ");
};
