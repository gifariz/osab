// Openings and overlapping areas, by the mesh step's rules (mesh/src/build.rs, probes 2c and
// docs/elements/STIFF_AREA.md): an opening cuts every floor or wall it overlaps in its plane; where areas overlap in
// one plane only one gets elements: the one inside the other, else the higher slab type (Stiff, Drop,
// Slab), else the smaller one. Null areas never get elements and do not take part.

import { areaAxes, areaOf, insidePolygon, segDist, segmentsCross } from "./area";
import { cross, dot, norm, pos, sub } from "./geometry";
import type { Area, Id, Model, ModelNode, Vec3 } from "./types";

type P2 = [number, number];

export interface Flat {
  area: Area;
  origin: Vec3;
  axes: [Vec3, Vec3, Vec3];
  corners: P2[];
  size: number;
  /** Slab type: Stiff 2, Drop 1, Slab 0. */
  rank: number;
  /** Bounding box of the corners (global), for a quick test before the exact one. */
  lo: Vec3;
  hi: Vec3;
}

/** The model's length tolerance, as the mesh step's: 1e-6 of the model's diagonal. */
export function modelTol(m: Model): number {
  if (!m.nodes.length) return 1e-6;
  const lo = [0, 1, 2].map((k) => Math.min(...m.nodes.map((n) => pos(n)[k])));
  const hi = [0, 1, 2].map((k) => Math.max(...m.nodes.map((n) => pos(n)[k])));
  return 1e-6 * Math.max(Math.hypot(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]), 1e-9);
}

/** An area in its plane; `joints` (by id) for many areas at once (a list search per corner is slow on large models). */
function flat(m: Model, a: Area, joints?: Map<Id, ModelNode>): Flat | null {
  const p: Vec3[] = [];
  for (const id of a.nodes) {
    const n = joints ? joints.get(id) : m.nodes.find((x) => x.id === id);
    if (!n) return null;
    p.push(pos(n));
  }
  if (p.length < 3) return null;
  const slab = m.area_sections.find((s) => s.id === a.section)?.slab_type;
  const lo = [0, 1, 2].map((k) => Math.min(...p.map((q) => q[k]))) as Vec3;
  const hi = [0, 1, 2].map((k) => Math.max(...p.map((q) => q[k]))) as Vec3;
  const f: Flat = { area: a, origin: p[0], axes: areaAxes(p, a.angle ?? 0), corners: [], size: areaOf(p), rank: slab === "stiff" ? 2 : slab === "drop" ? 1 : 0, lo, hi };
  f.corners = p.map((q) => to2(f, q));
  return f;
}

const to2 = (f: Flat, q: Vec3): P2 => [dot(sub(q, f.origin), f.axes[0]), dot(sub(q, f.origin), f.axes[1])];
const to3 = (f: Flat, q: P2): Vec3 => [0, 1, 2].map((k) => f.origin[k] + q[0] * f.axes[0][k] + q[1] * f.axes[1][k]) as Vec3;
const offset = (f: Flat, q: Vec3) => Math.abs(dot(sub(q, f.origin), f.axes[2]));

function coplanar(a: Flat, b: Flat, tol: number): boolean {
  return norm(cross(a.axes[2], b.axes[2])) < 1e-6 && offset(a, b.origin) <= tol;
}

/** Two polygons in one plane overlap: a corner strictly inside the other, edges crossing, or the same shape. */
function polygonsOverlap(a: P2[], b: P2[], tol: number): boolean {
  const strictly = (p: P2, poly: P2[]) => insidePolygon(p, poly, 0) && poly.every((q, k) => segDist(p, q, poly[(k + 1) % poly.length]) > tol);
  if (a.some((p) => strictly(p, b)) || b.some((p) => strictly(p, a))) return true;
  for (let i = 0; i < a.length; i++) {
    for (let j = 0; j < b.length; j++) {
      if (segmentsCross(a[i], a[(i + 1) % a.length], b[j], b[(j + 1) % b.length], tol)) return true;
    }
  }
  const mid = (p: P2[]): P2 => [p.reduce((s, q) => s + q[0], 0) / p.length, p.reduce((s, q) => s + q[1], 0) / p.length];
  return strictly(mid(a), b) || strictly(mid(b), a);
}

/** The areas that get elements (not openings, not null areas), in their planes. */
export function solids(m: Model): Flat[] {
  const joints = new Map(m.nodes.map((n) => [n.id, n]));
  return m.areas.flatMap((a) => (a.section && !a.opening ? [flat(m, a, joints)].filter((f): f is Flat => f !== null) : []));
}

/** Whether two boxes touch (within tol). */
const boxesTouch = (a: Flat, b: Flat, tol: number) => [0, 1, 2].every((k) => a.lo[k] <= b.hi[k] + tol && b.lo[k] <= a.hi[k] + tol);

/**
 * The pairs (i < j, by i then j) of areas whose boxes touch: sorted by the box's low X, each compared only with the
 * ones that start before it ends (sweep and prune), so large models do not compare every pair.
 */
function touchingPairs(hosts: Flat[], tol: number): [number, number][] {
  const order = hosts.map((_, i) => i).sort((i, j) => hosts[i].lo[0] - hosts[j].lo[0]);
  const pairs: [number, number][] = [];
  for (let s = 0; s < order.length; s++) {
    const a = hosts[order[s]];
    for (let r = s + 1; r < order.length && hosts[order[r]].lo[0] <= a.hi[0] + tol; r++) {
      if (!boxesTouch(a, hosts[order[r]], tol)) continue;
      const [i, j] = [order[s], order[r]];
      pairs.push(i < j ? [i, j] : [j, i]);
    }
  }
  return pairs.sort((x, y) => x[0] - y[0] || x[1] - y[1]);
}

/** The floors and walls an opening cuts (overlapping it in its plane); none = the opening is misplaced. */
export function openingHosts(m: Model, opening: Area, tol = modelTol(m), hosts: Flat[] = solids(m)): Id[] {
  const p = opening.nodes.map((id) => m.nodes.find((n) => n.id === id)).filter((n) => n !== undefined).map(pos);
  if (p.length !== opening.nodes.length || p.length < 3) return [];
  const lo = [0, 1, 2].map((k) => Math.min(...p.map((q) => q[k])));
  const hi = [0, 1, 2].map((k) => Math.max(...p.map((q) => q[k])));
  return hosts
    .filter((h) => [0, 1, 2].every((k) => lo[k] <= h.hi[k] + tol && h.lo[k] <= hi[k] + tol))
    .filter((h) => p.every((q) => offset(h, q) <= tol) && polygonsOverlap(p.map((q) => to2(h, q)), h.corners, tol))
    .map((h) => h.area.id);
}

export interface Overlap {
  a: Id;
  b: Id;
  /** The one that gets elements where they overlap. */
  winner: Id;
  /** Why: it lies inside the other, its slab type ranks higher, or it is the smaller one. */
  why: "inside" | "type" | "smaller";
}

/** Every pair of areas (with elements) that overlap in one plane, and which one gets elements there. */
export function overlaps(m: Model, tol = modelTol(m)): Overlap[] {
  const hosts = solids(m);
  // `within(a, b)`: a lies inside b (corners and edge midpoints inside b or on its edges, and smaller).
  const within = (a: Flat, b: Flat) =>
    a.size < b.size - tol * tol &&
    a.corners.every((c, k) => {
      const d = a.corners[(k + 1) % a.corners.length];
      const inB = (q: P2) => insidePolygon(to2(b, to3(a, q)), b.corners, tol);
      return inB(c) && inB([(c[0] + d[0]) / 2, (c[1] + d[1]) / 2]);
    });
  const out: Overlap[] = [];
  for (const [i, j] of touchingPairs(hosts, tol)) {
    const [a, b] = [hosts[i], hosts[j]];
    if (!coplanar(a, b, tol) || !polygonsOverlap(a.corners, b.corners.map((q) => to2(a, to3(b, q))), tol)) continue;
    let winner: Flat;
    let why: Overlap["why"];
    if (within(a, b) || within(b, a)) [winner, why] = [within(a, b) ? a : b, "inside"];
    else if (a.rank !== b.rank) [winner, why] = [a.rank > b.rank ? a : b, "type"];
    // Equal sizes: the later one (the mesh step's order).
    else [winner, why] = [a.size < b.size ? a : b.size < a.size ? b : b, "smaller"];
    out.push({ a: a.area.id, b: b.area.id, winner: winner.area.id, why });
  }
  return out;
}

/** Whether a point lies on an area (in its plane, inside its corners). */
export function pointOnArea(m: Model, a: Area, p: Vec3, tol = modelTol(m)): boolean {
  const f = flat(m, a);
  return !!f && offset(f, p) <= tol && insidePolygon(to2(f, p), f.corners, tol);
}
