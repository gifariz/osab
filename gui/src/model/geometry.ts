// Small vector helpers and the end-order rule (same as the solver's `draw_order_reversed`).

import type { ModelNode, Vec3 } from "./types";

export const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
export const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
export const scale = (a: Vec3, s: number): Vec3 => [a[0] * s, a[1] * s, a[2] * s];
export const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export const norm = (a: Vec3) => Math.sqrt(dot(a, a));
export const dist = (a: Vec3, b: Vec3) => norm(sub(a, b));
export const pos = (n: ModelNode): Vec3 => [n.x, n.y, n.z];

/**
 * The rule for which end of a newly drawn member is end i: the end with the lower Z; if both
 * ends are at the same height, the lower X; if X is also equal, the lower Y.
 * Returns true when the member drawn a -> b must be stored as b -> a.
 */
export function drawOrderReversed(a: Vec3, b: Vec3): boolean {
  const tol = 1e-9 * Math.max(dist(a, b), 1);
  for (const k of [2, 0, 1]) {
    if (Math.abs(a[k] - b[k]) > tol) return b[k] < a[k];
  }
  return false;
}

/** Distance from point p to segment ab, and the parameter t (0..1) of the closest point. */
export function pointSegment(p: Vec3, a: Vec3, b: Vec3): { d: number; t: number } {
  const ab = sub(b, a);
  const l2 = dot(ab, ab);
  const t = l2 === 0 ? 0 : Math.min(1, Math.max(0, dot(sub(p, a), ab) / l2));
  return { d: dist(p, add(a, scale(ab, t))), t };
}

export const cross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
export const unit = (a: Vec3): Vec3 => scale(a, 1 / norm(a));

/**
 * Local axes 1, 2, 3 of a member from end i to end j (the common default rule, same as the solver):
 * non-vertical members have local 2 in the vertical plane pointing up; vertical members have
 * local 2 = +X; local 3 = 1 x 2; `angle` (degrees) rotates 2 and 3 about 1.
 * Used to draw loads given in local directions before an analysis; results carry the solver's axes.
 */
export function localAxes(i: Vec3, j: Vec3, angleDeg = 0): [Vec3, Vec3, Vec3] {
  const e1 = unit(sub(j, i));
  let e2: Vec3;
  let e3: Vec3;
  if (Math.hypot(e1[0], e1[1]) < 1e-3) {
    e2 = [1, 0, 0];
    e3 = unit(cross(e1, e2));
  } else {
    e3 = unit(cross(e1, [0, 0, 1]));
    e2 = cross(e3, e1);
  }
  const a = (angleDeg * Math.PI) / 180;
  const [s, c] = [Math.sin(a), Math.cos(a)];
  return [e1, add(scale(e2, c), scale(e3, s)), add(scale(e2, -s), scale(e3, c))];
}
