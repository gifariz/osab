import * as THREE from "three";
import { describe, expect, it } from "vitest";

import type { PickRef } from "../../scene/types";
import { LINE_PICK_PX, NODE_PICK_PX, PickItemsBuilder, ScreenPicker, barycentric, noPickItems, segment, type PickItems } from "./picking";

type V = [number, number, number];

/** A seeded random number generator (the same scenes every run). */
function random(seed: number) {
  let s = seed;
  return () => {
    s = (s * 1664525 + 1013904223) % 4294967296;
    return s / 4294967296;
  };
}

/** A scene of joints, members, area edges and faces, with some items on top of others. */
function scene(seed: number) {
  const r = random(seed);
  const p = (): V => [r() * 40 - 20, r() * 40 - 20, r() * 30];
  const points: { p: V; pick: PickRef }[] = [];
  const lines: { a: V; b: V; pick: PickRef }[] = [];
  const tris: { a: V; b: V; c: V; pick: PickRef }[] = [];
  for (let i = 0; i < 150; i++) points.push({ p: p(), pick: { kind: "node", id: `J${i}` } });
  // A joint twice: the first one wins a tie.
  points.push({ p: points[3].p, pick: { kind: "node", id: "twin" } });
  for (let i = 0; i < 200; i++) lines.push({ a: p(), b: p(), pick: { kind: i % 3 ? "frame" : "area", id: `L${i % 120}` } });
  // A member on an area edge.
  lines.push({ a: lines[0].a, b: lines[0].b, pick: { kind: "frame", id: "on-edge" } });
  for (let i = 0; i < 150; i++) {
    const c = p();
    const q = (): V => [c[0] + r() * 8, c[1] + r() * 8, c[2] + r() * 4];
    tris.push({ a: c, b: q(), c: q(), pick: { kind: "area", id: `F${i % 60}` } });
  }
  // A long line and a big face, reaching many cells.
  lines.push({ a: [-500, -3, 2], b: [500, 4, 20], pick: { kind: "frame", id: "long" } });
  tris.push({ a: [-300, -300, 1], b: [300, -300, 1], c: [0, 300, 1], pick: { kind: "area", id: "big" } });
  return { points, lines, tris };
}

function items(s: ReturnType<typeof scene>): PickItems {
  const it = new PickItemsBuilder(s.points.length, s.lines.length, s.tris.length);
  for (const x of s.points) it.addPoint(x.p, it.ref(x.pick));
  for (const x of s.lines) it.addLine(x.a, x.b, it.ref(x.pick));
  for (const x of s.tris) it.addTriangle([...x.a, ...x.b, ...x.c], 0, it.ref(x.pick));
  return it.done();
}

/** The search over every item that picking replaced (the viewport's earlier `pickAt` and `pivotAt`). */
function reference(s: ReturnType<typeof scene>, camera: THREE.Camera, width: number, height: number) {
  const project = (p: V) => {
    const v = new THREE.Vector3(...p).project(camera);
    if (v.z < -1 || v.z > 1) return null;
    return { x: ((v.x + 1) / 2) * width, y: ((1 - v.y) / 2) * height, z: v.z };
  };
  const pickAt = (x: number, y: number, nodesOnly: boolean): PickRef | null => {
    let best: PickRef | null = null;
    let bestD = NODE_PICK_PX;
    for (const p of s.points) {
      const q = project(p.p);
      if (!q) continue;
      const d = Math.hypot(q.x - x, q.y - y);
      if (d < bestD) [best, bestD] = [p.pick, d];
    }
    if (best || nodesOnly) return best;
    let areaEdge: PickRef | null = null;
    let areaD = LINE_PICK_PX;
    bestD = LINE_PICK_PX;
    for (const l of s.lines) {
      const a = project(l.a);
      const b = project(l.b);
      if (!a || !b) continue;
      const [d] = segment(x, y, a.x, a.y, b.x, b.y);
      if (l.pick.kind === "area") {
        if (d < areaD) [areaEdge, areaD] = [l.pick, d];
      } else if (d < bestD) [best, bestD] = [l.pick, d];
    }
    if (best || areaEdge) return best ?? areaEdge;
    let depth = Infinity;
    for (const t of s.tris) {
      const [a, b, c] = [project(t.a), project(t.b), project(t.c)];
      if (!a || !b || !c) continue;
      const w = barycentric(x, y, a.x, a.y, b.x, b.y, c.x, c.y);
      if (!w) continue;
      const z = w[0] * a.z + w[1] * b.z + w[2] * c.z;
      if (z < depth) [best, depth] = [t.pick, z];
    }
    return best;
  };
  const pointAt = (x: number, y: number): V | null => {
    let best: V | null = null;
    let bestD = NODE_PICK_PX;
    for (const p of s.points) {
      const q = project(p.p);
      const d = q ? Math.hypot(q.x - x, q.y - y) : Infinity;
      if (d < bestD) [best, bestD] = [p.p, d];
    }
    if (!best) {
      bestD = LINE_PICK_PX;
      for (const l of s.lines) {
        const a = project(l.a);
        const b = project(l.b);
        if (!a || !b) continue;
        const [d, t] = segment(x, y, a.x, a.y, b.x, b.y);
        if (d >= bestD) continue;
        [best, bestD] = [[0, 1, 2].map((k) => l.a[k] + t * (l.b[k] - l.a[k])) as V, d];
      }
    }
    if (!best) {
      let depth = Infinity;
      for (const t of s.tris) {
        const [a, b, c] = [project(t.a), project(t.b), project(t.c)];
        if (!a || !b || !c) continue;
        const w = barycentric(x, y, a.x, a.y, b.x, b.y, c.x, c.y);
        if (!w) continue;
        const z = w[0] * a.z + w[1] * b.z + w[2] * c.z;
        if (z < depth) [best, depth] = [[0, 1, 2].map((k) => w[0] * t.a[k] + w[1] * t.b[k] + w[2] * t.c[k]) as V, z];
      }
    }
    return best;
  };
  return { pickAt, pointAt };
}

describe("picking through screen cells", () => {
  const [width, height] = [900, 640];
  const cameras = (): THREE.Camera[] => {
    const persp = new THREE.PerspectiveCamera(40, width / height, 0.5, 400);
    persp.up.set(0, 0, 1);
    persp.position.set(45, -60, 40);
    persp.lookAt(0, 0, 12);
    // Close up: some items behind the camera or beyond the view.
    const near = new THREE.PerspectiveCamera(40, width / height, 0.5, 60);
    near.up.set(0, 0, 1);
    near.position.set(4, -10, 14);
    near.lookAt(0, 0, 10);
    const ortho = new THREE.OrthographicCamera(-30, 30, 22, -22, 0.001, 500);
    ortho.up.set(0, 1, 0);
    ortho.position.set(0, 0, 200);
    ortho.lookAt(0, 0, 0);
    for (const c of [persp, near, ortho]) {
      c.updateMatrixWorld();
      c.updateProjectionMatrix();
    }
    return [persp, near, ortho];
  };

  it("finds what a search over every item finds, ties included", () => {
    for (const seed of [1, 2]) {
      const s = scene(seed);
      const it = items(s);
      for (const camera of cameras()) {
        const ref = reference(s, camera, width, height);
        const picker = new ScreenPicker(it, { view: camera.matrixWorldInverse.elements, proj: camera.projectionMatrix.elements, width, height });
        let found = 0;
        for (let y = 0.5; y < height; y += 11.3) {
          for (let x = 0.25; x < width; x += 9.1) {
            const want = ref.pickAt(x, y, false);
            if (want) found++;
            expect(picker.pickAt(x, y, false)).toEqual(want);
            expect(picker.pickAt(x, y, true)).toEqual(ref.pickAt(x, y, true));
            expect(picker.pointAt(x, y)).toEqual(ref.pointAt(x, y));
          }
        }
        // The scenes give the search something to find.
        expect(found).toBeGreaterThan(100);
      }
    }
  }, 60000);

  it("keeps a point the items share once, and only points equal to the last bit", () => {
    const s = scene(1);
    const it = items(s);
    // Joints, line ends and triangle corners refer to the same point where their coordinates are equal.
    expect(it.xyz.length / 3).toBeLessThan(s.points.length + 2 * s.lines.length + 3 * s.tris.length);
    const at = (k: number) => [...it.xyz.slice(3 * k, 3 * k + 3)];
    s.points.forEach((x, i) => expect(at(it.points[i])).toEqual(x.p));
    s.lines.forEach((x, i) => expect([at(it.lines[2 * i]), at(it.lines[2 * i + 1])]).toEqual([x.a, x.b]));
    s.tris.forEach((x, i) => expect([0, 1, 2].map((c) => at(it.tris[3 * i + c]))).toEqual([x.a, x.b, x.c]));
    expect(it.points[150]).toBe(it.points[3]);
    const near = new PickItemsBuilder(3, 0, 0);
    near.addPoint([1, 2, 3], near.ref({ kind: "node", id: "a" }));
    near.addPoint([1, 2, 3 + 4e-16], near.ref({ kind: "node", id: "b" }));
    near.addPoint([1, 2, 3], near.ref({ kind: "node", id: "c" }));
    const n = near.done();
    expect([...n.points]).toEqual([0, 1, 0]);
    expect(n.refs.map((r) => r.id)).toEqual(["a", "b", "c"]);
  });

  it("finds nothing in an empty scene", () => {
    const empty = noPickItems();
    const [camera] = cameras();
    const picker = new ScreenPicker(empty, { view: camera.matrixWorldInverse.elements, proj: camera.projectionMatrix.elements, width, height });
    expect(picker.pickAt(100, 100, false)).toBeNull();
    expect(picker.pointAt(100, 100)).toBeNull();
  });
});
