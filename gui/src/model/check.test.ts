// Check Model on large models: the same answers as comparing every pair, found without comparing every pair.

import { describe, expect, it } from "vitest";

import { checkModel, samePointPairs } from "./check";
import { SAME_POINT, newModel } from "./ops";
import { overlaps } from "./overlaps";
import type { Model, ModelNode } from "./types";

/** A floor of n x n squares on each of `levels` levels, with one extra square overlapping the first per level. */
function floors(n: number, levels: number): Model {
  const m = newModel({ xSpacings: [1], ySpacings: [1], storyHeights: [3] });
  const id = (i: number, j: number, k: number) => `${k}_${j}_${i}`;
  for (let k = 1; k <= levels; k++) {
    for (let j = 0; j <= n; j++) for (let i = 0; i <= n; i++) m.nodes.push({ id: id(i, j, k), x: i, y: j, z: 3 * k });
    for (let j = 0; j < n; j++) for (let i = 0; i < n; i++) m.areas.push({ id: `A${k}_${j}_${i}`, nodes: [id(i, j, k), id(i + 1, j, k), id(i + 1, j + 1, k), id(i, j + 1, k)], section: "Slab1" });
    m.nodes.push({ id: `c${k}`, x: 0.5, y: 0.5, z: 3 * k }, { id: `d${k}`, x: 1.5, y: 0.5, z: 3 * k }, { id: `e${k}`, x: 1.5, y: 1.5, z: 3 * k }, { id: `f${k}`, x: 0.5, y: 1.5, z: 3 * k });
    m.areas.push({ id: `X${k}`, nodes: [`c${k}`, `d${k}`, `e${k}`, `f${k}`], section: "Slab1" });
  }
  return m;
}

describe("Check Model on many objects", () => {
  it("joints at the same point: the pairs of comparing every pair, in the same order", () => {
    const nodes: ModelNode[] = [];
    let seed = 7;
    const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647) * 10;
    for (let i = 0; i < 400; i++) nodes.push({ id: `n${i}`, x: Math.round(rnd()), y: Math.round(rnd()), z: Math.round(rnd() / 5) });
    // Close pairs across a cell boundary.
    nodes.push({ id: "a", x: 2.0000004, y: 0, z: 0 }, { id: "b", x: 1.9999998, y: 0, z: 0 });
    const brute: [number, number][] = [];
    const d = (p: ModelNode, q: ModelNode) => Math.hypot(p.x - q.x, p.y - q.y, p.z - q.z);
    for (let i = 0; i < nodes.length; i++) for (let j = i + 1; j < nodes.length; j++) if (d(nodes[i], nodes[j]) <= SAME_POINT) brute.push([i, j]);
    expect(brute.length).toBeGreaterThan(10);
    expect(samePointPairs(nodes)).toEqual(brute);
  });

  it("overlapping areas: found on each level only, and a large model checks quickly", () => {
    const m = floors(30, 8);
    const over = overlaps(m);
    // The extra square overlaps four squares on each level, none across levels.
    expect(over).toHaveLength(4 * 8);
    expect(over.every((o) => o.a.slice(1).split("_")[0] === o.b.slice(1).split("_")[0] || o.b.startsWith("X"))).toBe(true);
    const t0 = performance.now();
    checkModel(m);
    expect(performance.now() - t0).toBeLessThan(3000);
  });
});
