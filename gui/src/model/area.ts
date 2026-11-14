// Area geometry, the same rules as the solver (solver/src/area.rs, axes.rs), checked on the reference cases
// (tests/area_axes_cases.json): corner order, local axes, floor/wall, shape checks.

import { t } from "../i18n";
import { add, cross, dot, norm, scale, sub, unit } from "./geometry";
import type { Vec3 } from "./types";

/** Corners within this fraction of the area's size off its plane still count as flat. */
const FLAT_TOL = 1e-6;
/** Local 3 counts as vertical when the sine of its angle to Z is below this (the common rule). */
const VERTICAL_TOL = 1e-3;

/** Largest distance between two corners (a quad's longer diagonal, a triangle's longest side). */
export function areaSize(p: Vec3[]): number {
  let s = 0;
  for (let i = 0; i < p.length; i++) for (let j = i + 1; j < p.length; j++) s = Math.max(s, norm(sub(p[i], p[j])));
  return s;
}

/**
 * Normal (not normalised): for a quad the cross product of the lines joining opposite side midpoints, for
 * a triangle the cross product of two sides, for more corners Newell's normal (as the mesh step).
 */
function normal(p: Vec3[]): Vec3 {
  if (p.length === 4) {
    const a = sub(scale(add(p[1], p[2]), 0.5), scale(add(p[0], p[3]), 0.5));
    const b = sub(scale(add(p[2], p[3]), 0.5), scale(add(p[0], p[1]), 0.5));
    return cross(a, b);
  }
  if (p.length === 3) return cross(sub(p[1], p[0]), sub(p[2], p[0]));
  const n: Vec3 = [0, 0, 0];
  for (let k = 0; k < p.length; k++) {
    const [a, b] = [p[k], p[(k + 1) % p.length]];
    n[0] += (a[1] - b[1]) * (a[2] + b[2]);
    n[1] += (a[2] - b[2]) * (a[0] + b[0]);
    n[2] += (a[0] - b[0]) * (a[1] + b[1]);
  }
  return n;
}

/**
 * Local axes (rows 1, 2, 3), the common rule (docs/SOURCES.md): 3 normal (right-hand rule
 * over the corner order); reference vector +Z, or +Y when 3 is (nearly) vertical; 1 = ref x 3, 2 = 3 x 1;
 * `angle` (degrees) rotates 1 and 2 about 3.
 */
export function areaAxes(p: Vec3[], angle = 0): [Vec3, Vec3, Vec3] {
  const e3 = unit(normal(p));
  let ref: Vec3 = [0, 0, 1];
  if (norm(cross(e3, ref)) < VERTICAL_TOL) ref = [0, 1, 0];
  const e1 = unit(cross(ref, e3));
  const e2 = cross(e3, e1);
  const a = (angle * Math.PI) / 180;
  const [s, c] = [Math.sin(a), Math.cos(a)];
  return [add(scale(e1, c), scale(e2, s)), add(scale(e1, -s), scale(e2, c)), e3];
}

/** Corners are analysed starting at the first corner (in the given order) at the lowest Z. */
export function firstCorner(p: Vec3[]): number {
  const low = Math.min(...p.map((q) => q[2]));
  const tol = 1e-9 * Math.max(areaSize(p), 1);
  return Math.max(0, p.findIndex((q) => q[2] - low <= tol));
}

/** The corner list in analysis order (rotated, direction kept). */
export function analysisOrder<T>(items: T[], p: Vec3[]): T[] {
  const k = firstCorner(p);
  return [...items.slice(k), ...items.slice(0, k)];
}

/** Slope of an area from horizontal, degrees (0 for a floor lying flat, 90 for a vertical wall). */
export function areaSlope(p: Vec3[]): number {
  const e3 = unit(normal(p));
  return (Math.acos(Math.min(1, Math.abs(e3[2]))) * 180) / Math.PI;
}

/**
 * Floor when the area slopes less than the floor/wall limit (20
 * degrees by default, the model's `angle_limits`), else Wall. Same rule as the mesh step.
 */
export const isFloor = (p: Vec3[], limit = 20): boolean => areaSlope(p) < limit - 1e-9;

/** Label prefix: F for floors, W for walls. */
export const areaPrefix = (p: Vec3[], limit = 20) => (isFloor(p, limit) ? "F" : "W");

/** Corner coordinates in the area's 1-2 plane (relative to the centroid). */
export function localXY(p: Vec3[], axes: [Vec3, Vec3, Vec3]): [number, number][] {
  const c = scale(p.reduce(add, [0, 0, 0] as Vec3), 1 / p.length);
  return p.map((q) => [dot(sub(q, c), axes[0]), dot(sub(q, c), axes[1])]);
}

/** Inside angle (degrees) at each corner, in the area's plane. */
function cornerAngles(xy: [number, number][]): number[] {
  return xy.map((cur, k) => {
    const prev = xy[(k + xy.length - 1) % xy.length];
    const next = xy[(k + 1) % xy.length];
    const a = [prev[0] - cur[0], prev[1] - cur[1]];
    const b = [next[0] - cur[0], next[1] - cur[1]];
    return (Math.atan2(b[0] * a[1] - b[1] * a[0], b[0] * a[0] + b[1] * a[1]) * 180) / Math.PI;
  });
}

/**
 * Why an area cannot be analysed, or null. A meshed area may have any number of corners (flat, not in one
 * line, edges not crossing; L-shapes are fine: the mesh step's rules). An area analysed as one element
 * (meshing off, or the area's mesh "none" with 3 or 4 corners; with more it is meshed) needs 3 or 4 corners and must be convex.
 * `warpedOk`: a 4-corner area may have its corners off one plane (the analysis takes it as one warped element
 * on its mean plane); editing keeps areas flat.
 */
export function shapeProblem(p: Vec3[], meshed = false, warpedOk = false): string | null {
  const s = areaSize(p);
  if (!(s > 1e-9)) return t("chk.areaDegenerate");
  if (p.length === 3) return norm(cross(sub(p[1], p[0]), sub(p[2], p[0]))) > 1e-9 * s * s ? null : t("chk.areaCollinear");
  if (!meshed && p.length !== 4) return t("chk.areaCorners", { n: p.length });
  const axes = areaAxes(p);
  const c = centroid(p);
  const off = Math.max(...p.map((q) => Math.abs(dot(sub(q, c), axes[2]))));
  if (off > FLAT_TOL * s && !(warpedOk && p.length === 4)) return t("chk.areaNotFlat");
  const xy = localXY(p, axes);
  if (meshed) {
    if (edgesCross(xy, 1e-6 * s)) return t("chk.areaEdgesCross");
    return Math.abs(signedArea(xy)) > 1e-12 * s * s ? null : t("chk.areaCollinear");
  }
  if (cornerAngles(xy).some((a) => !(a > 1e-6 && a < 180 - 1e-6))) return t("chk.areaNotConvex");
  return null;
}

/** Signed area of a polygon in a plane (positive counterclockwise). */
export function signedArea(xy: [number, number][]): number {
  let a = 0;
  for (let k = 0; k < xy.length; k++) {
    const [p, q] = [xy[k], xy[(k + 1) % xy.length]];
    a += p[0] * q[1] - q[0] * p[1];
  }
  return a / 2;
}

/** Whether two edges that are not neighbours cross or touch, away from their ends by more than `tol`. */
function edgesCross(xy: [number, number][], tol: number): boolean {
  const n = xy.length;
  for (let i = 0; i < n; i++) {
    for (let j = i + 2; j < n; j++) {
      if (i === 0 && j === n - 1) continue;
      if (segmentsCross(xy[i], xy[(i + 1) % n], xy[j], xy[(j + 1) % n], tol)) return true;
    }
  }
  return false;
}

type P2 = [number, number];
const cross2 = (a: P2, b: P2) => a[0] * b[1] - a[1] * b[0];
const sub2 = (a: P2, b: P2): P2 => [a[0] - b[0], a[1] - b[1]];

/** Segments a-b and c-d cross strictly inside both (by more than `tol`); parallel ones do not. */
export function segmentsCross(a: P2, b: P2, c: P2, d: P2, tol: number): boolean {
  const r = sub2(b, a);
  const s = sub2(d, c);
  const den = cross2(r, s);
  if (Math.abs(den) <= 1e-12 * Math.hypot(...r) * Math.hypot(...s)) return false;
  const tt = cross2(sub2(c, a), s) / den;
  const uu = cross2(sub2(c, a), r) / den;
  const [et, eu] = [tol / Math.hypot(...r), tol / Math.hypot(...s)];
  return tt > et && tt < 1 - et && uu > eu && uu < 1 - eu;
}

/** Point in polygon (even-odd rule); points within `tol` of an edge count as inside. */
export function insidePolygon(p: P2, poly: P2[], tol: number): boolean {
  const n = poly.length;
  for (let k = 0; k < n; k++) if (segDist(p, poly[k], poly[(k + 1) % n]) <= tol) return true;
  let c = false;
  for (let k = 0; k < n; k++) {
    const [a, b] = [poly[k], poly[(k + 1) % n]];
    if (a[1] > p[1] !== b[1] > p[1] && p[0] < a[0] + ((p[1] - a[1]) / (b[1] - a[1])) * (b[0] - a[0])) c = !c;
  }
  return c;
}

/** Distance from p to segment a-b (2D). */
export function segDist(p: P2, a: P2, b: P2): number {
  const d = sub2(b, a);
  const l2 = d[0] * d[0] + d[1] * d[1];
  const t = l2 === 0 ? 0 : Math.min(1, Math.max(0, ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2));
  return Math.hypot(p[0] - a[0] - t * d[0], p[1] - a[1] - t * d[1]);
}

/**
 * Triangles covering a flat polygon (ear clipping in its plane; L-shapes and other non-convex polygons
 * too), as corner index triples. Used to draw areas; the analysis mesh comes from the mesh step.
 */
export function triangulate(p: Vec3[]): [number, number, number][] {
  if (p.length < 3) return [];
  if (p.length === 3) return [[0, 1, 2]];
  const xy = localXY(p, areaAxes(p));
  const idx = p.map((_, i) => i);
  if (signedArea(xy) < 0) idx.reverse();
  const out: [number, number, number][] = [];
  const isEar = (k: number) => {
    const [a, b, c] = [idx[(k + idx.length - 1) % idx.length], idx[k], idx[(k + 1) % idx.length]];
    if (cross2(sub2(xy[b], xy[a]), sub2(xy[c], xy[b])) <= 0) return false;
    // No other corner inside the triangle a-b-c.
    return idx.every((q) => q === a || q === b || q === c || !inTriangle(xy[q], xy[a], xy[b], xy[c]));
  };
  let guard = 0;
  while (idx.length > 3 && guard++ < 10 * p.length) {
    const k = idx.findIndex((_, j) => isEar(j));
    if (k < 0) break;
    out.push([idx[(k + idx.length - 1) % idx.length], idx[k], idx[(k + 1) % idx.length]]);
    idx.splice(k, 1);
  }
  // Left over (a degenerate polygon, or the last triangle): a fan.
  for (let k = 1; k + 1 < idx.length; k++) out.push([idx[0], idx[k], idx[k + 1]]);
  return out;
}

/** Inside triangle a-b-c (counterclockwise) or on its sides: a corner on the diagonal stops an ear. */
function inTriangle(p: P2, a: P2, b: P2, c: P2): boolean {
  return cross2(sub2(b, a), sub2(p, a)) >= 0 && cross2(sub2(c, b), sub2(p, b)) >= 0 && cross2(sub2(a, c), sub2(p, c)) >= 0;
}

/** A point inside the area for its label: the area's centroid, or the largest triangle's when outside. */
export function labelPoint(p: Vec3[]): Vec3 {
  if (p.length <= 4) return centroid(p);
  let total = 0;
  let sum: Vec3 = [0, 0, 0];
  let biggest = { w: -1, at: centroid(p) };
  for (const tri of triangulate(p)) {
    const q = tri.map((k) => p[k]);
    const w = areaOf(q);
    total += w;
    sum = add(sum, scale(centroid(q), w));
    if (w > biggest.w) biggest = { w, at: centroid(q) };
  }
  const g = total > 0 ? scale(sum, 1 / total) : centroid(p);
  const axes = areaAxes(p);
  const to2 = (q: Vec3): P2 => [dot(q, axes[0]), dot(q, axes[1])];
  return insidePolygon(to2(g), p.map(to2), 0) ? g : biggest.at;
}

/**
 * True for a poorly shaped area (common shell-element guidelines, docs/SOURCES.md): an aspect ratio above 4 (a triangle's
 * longest side over its shortest), or for quads a corner angle outside 45-135 degrees.
 */
export function poorShape(p: Vec3[]): boolean {
  if (p.length > 4) return false;
  if (p.length === 3) {
    const sides = [0, 1, 2].map((k) => norm(sub(p[k], p[(k + 1) % 3])));
    return Math.max(...sides) / Math.min(...sides) > 4;
  }
  const xy = localXY(p, areaAxes(p));
  const mid = (i: number, j: number) => [(xy[i][0] + xy[j][0]) / 2, (xy[i][1] + xy[j][1]) / 2];
  const d = (a: number[], b: number[]) => Math.hypot(a[0] - b[0], a[1] - b[1]);
  const d1 = d(mid(0, 1), mid(2, 3));
  const d2 = d(mid(1, 2), mid(3, 0));
  const aspect = Math.max(d1, d2) / Math.min(d1, d2);
  return aspect > 4 || cornerAngles(xy).some((a) => a < 45 || a > 135);
}

/** Point at parameters (u, v) in 0..1 of a quad (bilinear). */
export function quadPoint(p: Vec3[], u: number, v: number): Vec3 {
  const w = [(1 - u) * (1 - v), u * (1 - v), u * v, (1 - u) * v];
  return [0, 1, 2].map((k) => w.reduce((s, wi, i) => s + wi * p[i][k], 0)) as Vec3;
}

/** Point with area coordinates (l1, l2, l3) of a triangle. */
export function triPoint(p: Vec3[], l1: number, l2: number, l3: number): Vec3 {
  return [0, 1, 2].map((k) => l1 * p[0][k] + l2 * p[1][k] + l3 * p[2][k]) as Vec3;
}

/**
 * A few points spread inside an area (for load arrows): four in a quad, three in a triangle, the centres
 * of the drawing triangles of a polygon (at most six, the largest).
 */
export function insidePoints(p: Vec3[]): Vec3[] {
  if (p.length === 3) return [triPoint(p, 2 / 3, 1 / 6, 1 / 6), triPoint(p, 1 / 6, 2 / 3, 1 / 6), triPoint(p, 1 / 6, 1 / 6, 2 / 3)];
  if (p.length === 4) return [quadPoint(p, 0.25, 0.25), quadPoint(p, 0.75, 0.25), quadPoint(p, 0.75, 0.75), quadPoint(p, 0.25, 0.75)];
  return triangulate(p)
    .map((tri) => tri.map((k) => p[k]))
    .sort((a, b) => areaOf(b) - areaOf(a))
    .slice(0, 6)
    .map(centroid);
}

/** Area of a flat quad (or triangle). */
export function areaOf(p: Vec3[]): number {
  let n: Vec3 = [0, 0, 0];
  for (let i = 1; i + 1 < p.length; i++) n = add(n, cross(sub(p[i], p[0]), sub(p[i + 1], p[0])));
  return norm(n) / 2;
}

/** Centroid of the corners. */
export const centroid = (p: Vec3[]): Vec3 => scale(p.reduce(add, [0, 0, 0] as Vec3), 1 / p.length);
