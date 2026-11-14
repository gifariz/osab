// Areas in the drawing data: faces and edges, area loads, extruded members and areas (View > Extrude),
// the deformed shape of areas and shell force contours.

import { areaAxes, centroid, labelPoint, quadPoint, triangulate } from "../model/area";
import { add, localAxes, pos, scale, sub } from "../model/geometry";
import { frameCentroidOffsets } from "../model/insertion";
import { ShapeUtils, Vector2 } from "three";

import { sectionRects } from "../model/sections";
import { outlineFaces, sectionShape, shapeOutline, type P2 } from "../model/sectionShapes";
import type { CaseResult, Id, Model, ShellCorner, ShellResult, ShellView, Vec3 } from "../model/types";
import { modelUnits } from "../units/display";
import { areaOf, frameOf, nodeOf } from "./lookup";
import type { PickRef, SceneData, SceneMesh } from "./types";

/** Corners of each triangle of a face (a fan for a triangle or quad, ear clipping for a polygon). */
function faceCorners(p: Vec3[]): number[] {
  const out: number[] = [];
  if (p.length <= 4) {
    for (let i = 1; i + 1 < p.length; i++) out.push(0, i, i + 1);
    return out;
  }
  for (const tri of triangulate(p)) out.push(...tri);
  return out;
}

/** Triangles of a face (a fan for a triangle or quad, ear clipping for a polygon), flattened. */
export function faceTriangles(p: Vec3[]): number[] {
  return faceCorners(p).flatMap((k) => p[k]);
}

/** Corner coordinates of an area, or null when a joint is missing. */
export function areaCorners(m: Model, nodes: Id[]): Vec3[] | null {
  const pts: Vec3[] = [];
  for (const id of nodes) {
    const n = nodeOf(m, id);
    if (!n) return null;
    pts.push(pos(n));
  }
  return pts;
}

/**
 * Face and edges of an area; `motion`: each corner's displacement for mode animation; `skipEdge`: edges
 * already drawn (from corner k to the next).
 */
export function addAreaFace(scene: SceneData, p: Vec3[], color: string, opacity: number, edge: string, edgeWidth: number, dashed: boolean, pick: PickRef, motion?: Vec3[], skipEdge?: (k: number) => boolean) {
  const corners = faceCorners(p);
  scene.meshes.push({ positions: corners.flatMap((k) => p[k]), color, opacity, pick, ...(motion ? { motion: corners.flatMap((k) => motion[k]) } : {}) });
  for (let k = 0; k < p.length; k++) {
    if (skipEdge?.(k)) continue;
    const j = (k + 1) % p.length;
    scene.lines.push({ a: p[k], b: p[j], color: edge, width: edgeWidth, dashed, pick, ...(motion ? { motion: [motion[k], motion[j]] as [Vec3, Vec3] } : {}) });
  }
}

/**
 * Uniform area loads: the value with its unit and direction, written on the area (no arrows: arrows are
 * point and frame loads). An area with several loads in the pattern gets one line each.
 */
export function addAreaLoads(scene: SceneData, m: Model, patternId: Id, shown: (id: Id) => Vec3[] | null) {
  const c = m.load_patterns.find((x) => x.id === patternId);
  if (!c) return;
  const texts = new Map<Id, string[]>();
  const U = modelUnits(m);
  for (const l of c.area_loads) {
    const a = areaOf(m, l.area);
    if (!a || !shown(a.id) || l.q === 0) continue;
    texts.set(a.id, [...(texts.get(a.id) ?? []), `${U.fmt(l.q, "forcePerArea")} ${U.label("forcePerArea")} ${l.dir}`]);
  }
  for (const [id, lines] of texts) {
    const p = shown(id)!;
    // Off the area's centre, where its own label goes.
    const at = p.length === 3 ? add(scale(centroid(p), 0.7), scale(p[0], 0.3)) : p.length === 4 ? quadPoint(p, 0.5, 0.3) : labelPoint(p);
    scene.labels.push({ position: at, text: lines.join("\n"), className: "lbl-load" });
  }
}

// ---- Extruded view ----

/** A closed prism from its bottom and top polygons (same corner count, same order). */
function prismTriangles(bottom: Vec3[], top: Vec3[]): number[] {
  const n = bottom.length;
  const out = [...faceTriangles([...bottom].reverse()), ...faceTriangles(top)];
  for (let k = 0; k < n; k++) out.push(...faceTriangles([bottom[k], bottom[(k + 1) % n], top[(k + 1) % n], top[k]]));
  return out;
}

/** A section's faces in local 3 (h) and 2 (v) from its centroid: its shape's outline, else rectangles. */
export function sectionFaces(m: Model, sec: Model["sections"][number]): { outer: P2[]; holes: P2[][] }[] {
  const spec = sectionShape(m, sec);
  if (!spec) {
    return sectionRects(m, sec).map((r) => ({
      outer: [
        [r.z0, r.y0],
        [r.z1, r.y0],
        [r.z1, r.y1],
        [r.z0, r.y1],
      ],
      holes: [],
    }));
  }
  const { steel, fill } = shapeOutline(spec);
  const [c2, c3] = sec.centroid_offset ?? [0, 0];
  const shift = (p: P2[]) => p.map(([h, v]): P2 => [h - c3, v - c2]);
  return [...outlineFaces(steel.map(shift)), ...outlineFaces(fill.map(shift))];
}

/** Triangles of a section face (an outline with holes), as indices into the outline then the holes. */
function capTriangles(face: { outer: P2[]; holes: P2[][] }): number[][] {
  const v = (p: P2[]) => p.map(([h, w]) => new Vector2(h, w));
  return ShapeUtils.triangulateShape(v(face.outer), face.holes.map(v));
}

/** The solid of a member: its section's faces (local 3 across, 2 up) swept from end i to end j. */
export function frameSolid(m: Model, frameId: Id): number[] {
  const e = frameOf(m, frameId);
  const a = e && nodeOf(m, e.nodes[0]);
  const b = e && nodeOf(m, e.nodes[1]);
  const sec = e && m.sections.find((s) => s.id === e.section);
  if (!e || !a || !b || !sec) return [];
  const [, ax2, ax3] = localAxes(pos(a), pos(b), e.angle ?? 0);
  // Drawn at its insertion point: the section's centroid away from the joint line (a beam at its top), the
  // cardinal points on the box around the outline drawn.
  const faces = sectionFaces(m, sec);
  const pts = faces.flatMap((f) => f.outer);
  const span = (k: 0 | 1): [number, number] => [Math.min(...pts.map((q) => q[k])), Math.max(...pts.map((q) => q[k]))];
  const [ci, cj] = frameCentroidOffsets(m, e, pts.length ? { h: span(0), v: span(1) } : undefined);
  const out: number[] = [];
  const [ei, ej] = [add(pos(a), ci), add(pos(b), cj)];
  const at = (end: Vec3, [h, v]: P2) => add(end, add(scale(ax2, v), scale(ax3, h)));
  for (const face of faces) {
    const points = [face.outer, ...face.holes].flat();
    // Caps: the face at end i (facing back) and at end j.
    for (const tri of capTriangles(face)) {
      out.push(...at(ei, points[tri[0]]), ...at(ei, points[tri[2]]), ...at(ei, points[tri[1]]));
      out.push(...at(ej, points[tri[0]]), ...at(ej, points[tri[1]]), ...at(ej, points[tri[2]]));
    }
    // Sides: each ring of the outline swept along the member.
    for (const ring of [face.outer, ...face.holes]) {
      for (let k = 0; k < ring.length; k++) {
        const [p, q] = [ring[k], ring[(k + 1) % ring.length]];
        out.push(...faceTriangles([at(ei, p), at(ei, q), at(ej, q), at(ej, p)]));
      }
    }
  }
  return out;
}

/** The solid of an area: its face moved half the thickness each way along local 3. */
export function areaSolid(p: Vec3[], thickness: number): number[] {
  const n = scale(areaAxes(p)[2], thickness / 2);
  const bottom = p.map((q) => sub(q, n));
  const top = p.map((q) => add(q, n));
  return prismTriangles(bottom, top);
}

// ---- Results ----

/** A shell contour value at a corner: a component, a principal value or the resultant shear. */
export function shellValue(c: ShellCorner, v: ShellView): number {
  const principal = (a: number, b: number, s: number, sign: 1 | -1) => (a + b) / 2 + sign * Math.hypot((a - b) / 2, s);
  switch (v) {
    case "FMAX":
      return principal(c.F11, c.F22, c.F12, 1);
    case "FMIN":
      return principal(c.F11, c.F22, c.F12, -1);
    case "MMAX":
      return principal(c.M11, c.M22, c.M12, 1);
    case "MMIN":
      return principal(c.M11, c.M22, c.M12, -1);
    case "VMAX":
      return Math.hypot(c.V13, c.V23);
    default:
      return c[v];
  }
}

/** The contour colour ramp's stops, low to high: blue, cyan, green, yellow, red. */
export const RAMP_STOPS: readonly (readonly [number, number, number])[] = [
  [0.18, 0.3, 0.95],
  [0.1, 0.8, 0.95],
  [0.2, 0.85, 0.3],
  [0.98, 0.88, 0.2],
  [0.95, 0.25, 0.2],
];

/** Contour colour for t in 0..1: blue (low), cyan, green, yellow, red (high). */
export function rampColor(t: number): [number, number, number] {
  const stops = RAMP_STOPS;
  const x = Math.min(1, Math.max(0, Number.isFinite(t) ? t : 0.5)) * (stops.length - 1);
  const i = Math.min(stops.length - 2, Math.floor(x));
  const f = x - i;
  return [0, 1, 2].map((k) => stops[i][k] + (stops[i + 1][k] - stops[i][k]) * f) as [number, number, number];
}

/** Values at each shell corner (per element, or averaged over the shells meeting at each joint). */
export function shellCornerValues(shells: ShellResult[], v: ShellView, averaged: boolean): Map<ShellResult, number[]> {
  const out = new Map<ShellResult, number[]>();
  for (const s of shells) out.set(s, s.corners.map((c) => shellValue(c, v)));
  if (!averaged) return out;
  const sum = new Map<Id, [number, number]>();
  for (const [s, vals] of out) {
    s.corners.forEach((c, k) => {
      const acc = sum.get(c.node) ?? [0, 0];
      sum.set(c.node, [acc[0] + vals[k], acc[1] + 1]);
    });
  }
  for (const [s] of out) {
    out.set(
      s,
      s.corners.map((c) => {
        const [total, n] = sum.get(c.node)!;
        return total / n;
      }),
    );
  }
  return out;
}

/**
 * A smooth contour over an element from its corner values: bilinear over a quad, linear over a triangle
 * (like the elements' own fields). Two triangles per quad, one per triangle, with the colour-ramp positions
 * of the corners on every vertex (`contour`), so the colour is worked out per pixel; `colors` are the
 * corners' own colours.
 */
export function contourMesh(p: Vec3[], values: number[], min: number, max: number, pick: PickRef): SceneMesh {
  const span = max - min;
  const t = values.map((v) => (span > 0 ? (v - min) / span : 0.5));
  const triangle = p.length === 3;
  const ramp = triangle ? [t[0], t[1], t[2], t[2]] : t.slice(0, 4);
  // Corner order and element coordinates (u, v) of each vertex.
  const corners = triangle ? [0, 1, 2] : [0, 1, 2, 0, 2, 3];
  const uvOf: [number, number][] = triangle ? [[0, 0], [1, 0], [0, 1]] : [[0, 0], [1, 0], [1, 1], [0, 1]];
  return {
    positions: corners.flatMap((k) => p[k]),
    colors: corners.flatMap((k) => rampColor(t[k])),
    color: "#ffffff",
    opacity: 1,
    pick,
    contour: { uv: corners.flatMap((k) => uvOf[k]), ramp: corners.flatMap(() => ramp), triangle },
  };
}

/** Displaced corners of an area (k = display scale), or null when a joint has no result. */
export function deformedCorners(m: Model, c: CaseResult, nodes: Id[], k: number): Vec3[] | null {
  const p = areaCorners(m, nodes);
  if (!p) return null;
  const out: Vec3[] = [];
  for (let i = 0; i < nodes.length; i++) {
    const d = c.displacements.find((x) => x.node === nodes[i]);
    if (!d) return null;
    out.push(add(p[i], scale([d.ux, d.uy, d.uz], k)));
  }
  return out;
}
