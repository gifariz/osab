// The viewport contract. It draws plain scene data and reports clicks in terms of ids and points.
// It knows nothing about the model, so a different renderer (e.g. native wgpu) can replace
// ThreeViewport by implementing this interface.

import type { Vec3 } from "../model/types";
import type { Bounds, PickRef, PickStyle, SceneData } from "../scene/types";
import type { ViewSpec } from "../state/store";

export interface ClickEvent {
  /** Object under the mouse (joints win over members, members over areas). */
  pick: PickRef | null;
  /** Snap point under the mouse (grid intersection or joint), when snapping is on. */
  snap: Vec3 | null;
  /** Where a new point goes: the snap point, else the point on the plan/elevation plane rounded to
   * the snap step (null in the 3D view or when snapping is off). */
  point: Vec3 | null;
  /** The point on the plan/elevation plane under the mouse, not rounded (null in the 3D view). */
  planePoint: Vec3 | null;
  shift: boolean;
  ctrl: boolean;
}

export interface BoxEvent {
  picks: PickRef[];
  /** Dragged right-to-left: everything the box touches (crossing selection). */
  crossing: boolean;
  shift: boolean;
  ctrl: boolean;
}

export interface ViewportEvents {
  hover: (pick: PickRef | null) => void;
  click: (e: ClickEvent) => void;
  box: (e: BoxEvent) => void;
  /** Point under the mouse: the snap point, or the point on the view plane (plan/elevation). */
  cursor: (p: Vec3 | null, snapped: boolean) => void;
  /** Right click without dragging (ends a Draw Frame chain). */
  finish: () => void;
}

export interface Interaction {
  /** What hovering and clicking can pick. */
  pick: "all" | "nodes" | "none";
  snap: boolean;
  /** Free points on the plan/elevation plane are rounded to this step (model length units). */
  step: number;
  /** Left-drag draws a selection box. */
  box: boolean;
  /** Points clicked so far while drawing (a member's start, an area's corners): a rubber band runs
   * through them to the mouse. */
  rubber: Vec3[];
  /** Close the rubber band back to its first point (drawing an area). */
  rubberClosed: boolean;
}

export interface Viewport {
  /**
   * The model (`base`) and the results drawn over it. `overlay` gives the results' items; it is called only
   * when it is not the one drawn already (the same function keeps the results layer as it is), so nothing has
   * to keep the items once they are drawn. A new `base` with the same `overlay` builds only the model's layer.
   */
  setScene(base: SceneData, overlay: (() => SceneData) | null): void;
  /** Selected and highlighted objects of the model drawn in these styles (by `pickKey`); the rest as built. */
  setStyles(styles: Map<string, PickStyle>): void;
  /** Mode animation: the results' displacements drawn at this fraction (-1..1) of their full size. */
  setSwing(swing: number): void;
  /** Background colour and the colour of hover highlights, snap marker and rubber band. */
  setColors(background: string, hover: string): void;
  /** The 3D view in parallel projection (no perspective) or not; plan and elevation views always are. */
  setParallel(on: boolean): void;
  setView(view: ViewSpec, bounds: Bounds, fit: boolean): void;
  setSnapPoints(points: Vec3[]): void;
  setInteraction(i: Interaction): void;
  zoomToFit(): void;
  /** Screen position (CSS pixels, relative to the viewport) of a world point, or null if hidden. */
  project(p: Vec3): { x: number; y: number } | null;
  on<K extends keyof ViewportEvents>(event: K, handler: ViewportEvents[K]): () => void;
  dispose(): void;
}
