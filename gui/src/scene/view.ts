// Which objects a view shows, and the snap points it offers.

import { t } from "../i18n";
import { gridPoints } from "../model/grid";
import { gridOf } from "../model/ops";
import type { Model, Vec3 } from "../model/types";
import type { ViewSpec } from "../state/store";
import type { Bounds } from "./types";
import { modelUnits } from "../units/display";

const IN_PLANE = 1e-6;

/** Plan and elevation views show only what lies in their plane; the 3D view shows everything. */
export function inView(p: Vec3, view: ViewSpec): boolean {
  switch (view.kind) {
    case "3d":
      return true;
    case "plan":
      return Math.abs(p[2] - view.at) <= IN_PLANE;
    case "elev-xz":
      return Math.abs(p[1] - view.at) <= IN_PLANE;
    case "elev-yz":
      return Math.abs(p[0] - view.at) <= IN_PLANE;
  }
}

/** Grid intersections in the view (all of them in 3D). Joints are added by the viewport. */
export function snapPoints(m: Model, view: ViewSpec): Vec3[] {
  return gridPoints(gridOf(m)).filter((p) => inView(p, view));
}

/** Box around the joints and grid. */
export function modelBounds(m: Model): Bounds {
  const pts: Vec3[] = [...gridPoints(gridOf(m)), ...m.nodes.map((n) => [n.x, n.y, n.z] as Vec3)];
  if (pts.length === 0) pts.push([0, 0, 0]);
  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  for (const p of pts) {
    for (let k = 0; k < 3; k++) {
      min[k] = Math.min(min[k], p[k]);
      max[k] = Math.max(max[k], p[k]);
    }
  }
  return { min, max };
}

/** A typical length for sizing arrows and diagrams: the bounds diagonal (at least 1). */
export function boundsSize(b: Bounds): number {
  return Math.max(1, Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]));
}

export function viewName(m: Model, view: ViewSpec): string {
  const g = gridOf(m);
  const U = modelUnits(m);
  const near = <T>(list: T[], value: (t: T) => number) => list.find((t) => Math.abs(value(t) - view.at) <= IN_PLANE);
  switch (view.kind) {
    case "3d":
      return t("view.3d");
    case "plan": {
      const s = near(g.stories, (s) => s.z);
      return t("view.plan", { at: s ? s.label : `Z = ${U.fmt(view.at, "distance")}` });
    }
    case "elev-xz": {
      const l = near(g.y, (l) => l.coord);
      return t("view.elev", { at: l ? l.label : `Y = ${U.fmt(view.at, "distance")}` });
    }
    case "elev-yz": {
      const l = near(g.x, (l) => l.coord);
      return t("view.elev", { at: l ? l.label : `X = ${U.fmt(view.at, "distance")}` });
    }
  }
}
