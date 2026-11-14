// Plain drawing data in world coordinates. The viewport draws it without knowing about the model,
// so a different renderer (e.g. a native wgpu viewport) can take the same data.

import type { Id, Vec3 } from "../model/types";

/** What a drawn item stands for, so clicks and hovers can name the object. */
export interface PickRef {
  kind: "node" | "frame" | "link" | "area";
  id: Id;
}

/** The key of an object in maps of objects. */
export const pickKey = (p: PickRef) => `${p.kind}:${p.id}`;

/**
 * How a selected or highlighted object is drawn instead of its own look: the colour of all its items, and
 * whether its lines are dashed, the opacity of its faces and the size of its joint dot (missing: as drawn).
 */
export interface PickStyle {
  color: string;
  dashed?: boolean;
  opacity?: number;
  size?: number;
}

export interface ScenePoint {
  position: Vec3;
  color: string;
  /** Pixels. */
  size: number;
  pick?: PickRef;
  /** Mode animation: the displacement drawn at full swing (see `Viewport.setSwing`). */
  motion?: Vec3;
}

export interface SceneLine {
  a: Vec3;
  b: Vec3;
  color: string;
  /** Pixels. */
  width: number;
  dashed?: boolean;
  opacity?: number;
  pick?: PickRef;
  /** Mode animation: the displacement of each end drawn at full swing. */
  motion?: [Vec3, Vec3];
}

export interface SceneLabel {
  position: Vec3;
  text: string;
  /** CSS class: "lbl-node", "lbl-elem", "lbl-dia", "lbl-grid", "lbl-load", "lbl-mass", "lbl-value", "lbl-value neg". */
  className: string;
}

export interface SceneArrow {
  tail: Vec3;
  head: Vec3;
  color: string;
  /** Moments are drawn as double-headed arrows (right-hand rule). */
  double?: boolean;
}

/** Support symbols, drawn at a fixed screen size just below the joint; springs to its left. */
export interface SceneGlyph {
  position: Vec3;
  kind: "fixed" | "pinned" | "roller" | "partial" | "spring";
  color: string;
}

/** Filled triangles (x, y, z per vertex, 3 vertices per triangle). */
export interface SceneMesh {
  positions: number[];
  color: string;
  opacity: number;
  /** Per-vertex colours (r, g, b in 0..1, 3 per vertex) used instead of `color`, e.g. contours. */
  colors?: number[];
  /** Draw it lit and shaded (extruded solids) instead of flat. */
  shaded?: boolean;
  pick?: PickRef;
  /** Mode animation: the displacement of each vertex drawn at full swing (3 per vertex). */
  motion?: number[];
  /**
   * Contours coloured per pixel (a renderer may use `colors` instead): per vertex the element's own
   * coordinates (2 per vertex: u, v over a quad; the area coordinates of corners 2 and 3 over a triangle) and
   * the colour-ramp positions (0..1) of the element's corners (4 per vertex; a triangle repeats its third).
   */
  contour?: { uv: number[]; ramp: number[]; triangle: boolean };
}

export interface Bounds {
  min: Vec3;
  max: Vec3;
}

export interface SceneData {
  points: ScenePoint[];
  lines: SceneLine[];
  labels: SceneLabel[];
  arrows: SceneArrow[];
  glyphs: SceneGlyph[];
  meshes: SceneMesh[];
}

export const emptyScene = (): SceneData => ({ points: [], lines: [], labels: [], arrows: [], glyphs: [], meshes: [] });
