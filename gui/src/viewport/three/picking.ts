// Picking in screen space. Every pickable joint, line and triangle is projected once per camera position and
// sorted into a grid of screen cells, so a mouse move measures only the items near it. The rules are those of
// a search over every item: a joint within 10 px wins, then a member or link within 7 px before an area edge
// within 7 px, then the nearest face under the mouse. Items are tried in their scene order with the same
// strict comparisons, so ties go the same way, and the projection repeats `Vector3.project` step by step, so
// the numbers are the same too.

import type { PickRef } from "../../scene/types";

export const NODE_PICK_PX = 10;
export const LINE_PICK_PX = 7;
const CELL = 32;
/** Lines and triangles reaching more cells than this are tried on every query instead. */
const MAX_CELLS = 256;

/**
 * Pickable items in scene order: their points and the object each belongs to (an index into `refs`). A point
 * the items share (a joint's dot, the ends of its lines, the corners of its faces) is kept and projected once.
 */
export interface PickItems {
  refs: PickRef[];
  /** x, y, z of each point (the same coordinates once). */
  xyz: Float64Array;
  /** The point of each joint. */
  points: Int32Array;
  pointRef: Int32Array;
  /** Two points per line. */
  lines: Int32Array;
  lineRef: Int32Array;
  /** Three points per triangle. */
  tris: Int32Array;
  triRef: Int32Array;
}

export const noPickItems = (): PickItems => ({ refs: [], xyz: new Float64Array(), points: new Int32Array(), pointRef: new Int32Array(), lines: new Int32Array(), lineRef: new Int32Array(), tris: new Int32Array(), triRef: new Int32Array() });

/**
 * Makes pick items for at most `np` joints, `nl` lines and `nt` triangles, added in scene order. Points with
 * the same coordinates (bit for bit) become one, so they project to the same numbers as before.
 */
export class PickItemsBuilder {
  readonly refs: PickRef[] = [];
  private index = new Map<string, number>();
  private xyz: Float64Array;
  private bits: Uint32Array;
  private n = 0;
  private table: Int32Array;
  private mask: number;
  private key = new Float64Array(3);
  private keyBits = new Uint32Array(this.key.buffer);
  private points: Int32Array;
  private pointRef: Int32Array;
  private lines: Int32Array;
  private lineRef: Int32Array;
  private tris: Int32Array;
  private triRef: Int32Array;
  private ip = 0;
  private il = 0;
  private it = 0;

  constructor(np: number, nl: number, nt: number) {
    const most = np + 2 * nl + 3 * nt;
    this.xyz = new Float64Array(3 * most);
    this.bits = new Uint32Array(this.xyz.buffer);
    let size = 16;
    while (size < 2 * most) size *= 2;
    this.table = new Int32Array(size).fill(-1);
    this.mask = size - 1;
    this.points = new Int32Array(np);
    this.pointRef = new Int32Array(np);
    this.lines = new Int32Array(2 * nl);
    this.lineRef = new Int32Array(nl);
    this.tris = new Int32Array(3 * nt);
    this.triRef = new Int32Array(nt);
  }

  /** The index of an object in `refs` (added when new). */
  ref(p: PickRef): number {
    const key = `${p.kind}:${p.id}`;
    let i = this.index.get(key);
    if (i === undefined) {
      this.index.set(key, (i = this.refs.length));
      this.refs.push(p);
    }
    return i;
  }

  /** The point at `q[o..o + 3]`. */
  private point(q: ArrayLike<number>, o: number): number {
    const [k, b] = [this.key, this.keyBits];
    k[0] = q[o];
    k[1] = q[o + 1];
    k[2] = q[o + 2];
    let h = 0x811c9dc5;
    for (let w = 0; w < 6; w++) h = Math.imul(h ^ b[w], 0x01000193);
    h = (h ^ (h >>> 15)) & this.mask;
    for (;;) {
      const at = this.table[h];
      if (at < 0) break;
      const x = 6 * at;
      const bits = this.bits;
      if (bits[x] === b[0] && bits[x + 1] === b[1] && bits[x + 2] === b[2] && bits[x + 3] === b[3] && bits[x + 4] === b[4] && bits[x + 5] === b[5]) return at;
      h = (h + 1) & this.mask;
    }
    const i = this.n++;
    this.xyz.set(k, 3 * i);
    this.table[h] = i;
    return i;
  }

  addPoint(q: ArrayLike<number>, ref: number) {
    this.points[this.ip] = this.point(q, 0);
    this.pointRef[this.ip++] = ref;
  }

  addLine(a: ArrayLike<number>, b: ArrayLike<number>, ref: number) {
    this.lines[2 * this.il] = this.point(a, 0);
    this.lines[2 * this.il + 1] = this.point(b, 0);
    this.lineRef[this.il++] = ref;
  }

  /** The triangle of the 9 numbers at `q[o..o + 9]`. */
  addTriangle(q: ArrayLike<number>, o: number, ref: number) {
    for (let c = 0; c < 3; c++) this.tris[3 * this.it + c] = this.point(q, o + 3 * c);
    this.triRef[this.it++] = ref;
  }

  done(): PickItems {
    const cut = (a: Int32Array, n: number) => (a.length === n ? a : a.slice(0, n));
    return {
      refs: this.refs,
      xyz: this.xyz.slice(0, 3 * this.n),
      points: cut(this.points, this.ip),
      pointRef: cut(this.pointRef, this.ip),
      lines: cut(this.lines, 2 * this.il),
      lineRef: cut(this.lineRef, this.il),
      tris: cut(this.tris, 3 * this.it),
      triRef: cut(this.triRef, this.it),
    };
  }
}

/** The pick items of `a`, then those of `b` (an object named in both keeps its place from `a`). */
export function joinPickItems(a: PickItems, b: PickItems): PickItems {
  const refs = [...a.refs];
  const index = new Map(a.refs.map((r, i) => [`${r.kind}:${r.id}`, i]));
  const map = new Int32Array(b.refs.length);
  b.refs.forEach((r, i) => {
    const key = `${r.kind}:${r.id}`;
    let j = index.get(key);
    if (j === undefined) {
      index.set(key, (j = refs.length));
      refs.push(r);
    }
    map[i] = j;
  });
  const shift = a.xyz.length / 3;
  const xyz = new Float64Array(a.xyz.length + b.xyz.length);
  xyz.set(a.xyz);
  xyz.set(b.xyz, a.xyz.length);
  const join = (x: Int32Array, y: Int32Array, f: (v: number) => number) => {
    const out = new Int32Array(x.length + y.length);
    out.set(x);
    for (let i = 0; i < y.length; i++) out[x.length + i] = f(y[i]);
    return out;
  };
  const moved = (v: number) => v + shift;
  const ref = (v: number) => map[v];
  return {
    refs,
    xyz,
    points: join(a.points, b.points, moved),
    pointRef: join(a.pointRef, b.pointRef, ref),
    lines: join(a.lines, b.lines, moved),
    lineRef: join(a.lineRef, b.lineRef, ref),
    tris: join(a.tris, b.tris, moved),
    triRef: join(a.triRef, b.triRef, ref),
  };
}

export interface Camera {
  /** `camera.matrixWorldInverse.elements` and `camera.projectionMatrix.elements`. */
  view: ArrayLike<number>;
  proj: ArrayLike<number>;
  width: number;
  height: number;
}

/** Screen x, y and depth (-1 near .. 1 far) of each point; NaN where outside the view depth. */
export function projectAll(xyz: Float64Array, cam: Camera): Float64Array {
  const out = new Float64Array(xyz.length);
  const v = cam.view;
  const p = cam.proj;
  for (let i = 0; i < xyz.length; i += 3) {
    const x = xyz[i];
    const y = xyz[i + 1];
    const z = xyz[i + 2];
    // Vector3.applyMatrix4(matrixWorldInverse).applyMatrix4(projectionMatrix), as three.js does it.
    let w = 1 / (v[3] * x + v[7] * y + v[11] * z + v[15]);
    const x1 = (v[0] * x + v[4] * y + v[8] * z + v[12]) * w;
    const y1 = (v[1] * x + v[5] * y + v[9] * z + v[13]) * w;
    const z1 = (v[2] * x + v[6] * y + v[10] * z + v[14]) * w;
    w = 1 / (p[3] * x1 + p[7] * y1 + p[11] * z1 + p[15]);
    const nx = (p[0] * x1 + p[4] * y1 + p[8] * z1 + p[12]) * w;
    const ny = (p[1] * x1 + p[5] * y1 + p[9] * z1 + p[13]) * w;
    const nz = (p[2] * x1 + p[6] * y1 + p[10] * z1 + p[14]) * w;
    if (nz < -1 || nz > 1 || Number.isNaN(nz)) {
      out[i] = out[i + 1] = out[i + 2] = NaN;
      continue;
    }
    out[i] = ((nx + 1) / 2) * cam.width;
    out[i + 1] = ((1 - ny) / 2) * cam.height;
    out[i + 2] = nz;
  }
  return out;
}

/** Items of one kind sorted into screen cells (compressed rows: `start[cell]..start[cell + 1]` in `items`). */
interface Grid {
  start: Int32Array;
  items: Int32Array;
  /** Items reaching too many cells: tried on every query. */
  wide: Int32Array;
}

/** A pick search for one camera position. */
export class ScreenPicker {
  /** Screen x, y and depth of each point of the items (`projectAll`). */
  readonly s: Float64Array;
  private cols: number;
  private rows: number;
  private gp: Grid;
  private gl: Grid;
  private gt: Grid;
  private stamp: Int32Array;
  private stampValue = 0;

  constructor(
    readonly items: PickItems,
    readonly cam: Camera,
  ) {
    this.s = projectAll(items.xyz, cam);
    this.cols = Math.max(1, Math.ceil(cam.width / CELL));
    this.rows = Math.max(1, Math.ceil(cam.height / CELL));
    const n = items.pointRef.length;
    this.gp = this.grid(n, (i, add) => {
      const o = this.pointAt3(i);
      const x = this.s[o];
      const y = this.s[o + 1];
      if (!Number.isNaN(x)) this.cellsOfBox(x - NODE_PICK_PX, y - NODE_PICK_PX, x + NODE_PICK_PX, y + NODE_PICK_PX, add);
    });
    this.gl = this.grid(items.lineRef.length, (i, add) => this.cellsOfLine(i, add));
    this.gt = this.grid(items.triRef.length, (i, add) => {
      const s = this.s;
      const [a, b, c] = this.triangle3(i);
      if (Number.isNaN(s[a]) || Number.isNaN(s[b]) || Number.isNaN(s[c])) return;
      this.cellsOfBox(Math.min(s[a], s[b], s[c]), Math.min(s[a + 1], s[b + 1], s[c + 1]), Math.max(s[a], s[b], s[c]), Math.max(s[a + 1], s[b + 1], s[c + 1]), add);
    });
    this.stamp = new Int32Array(Math.max(n, items.lineRef.length, items.triRef.length));
  }

  /** Offsets in `s` and `items.xyz` of joint `i`, of the ends of line `i` and of the corners of triangle `i`. */
  pointAt3(i: number): number {
    return 3 * this.items.points[i];
  }
  line3(i: number): [number, number] {
    const l = this.items.lines;
    return [3 * l[2 * i], 3 * l[2 * i + 1]];
  }
  triangle3(i: number): [number, number, number] {
    const t = this.items.tris;
    return [3 * t[3 * i], 3 * t[3 * i + 1], 3 * t[3 * i + 2]];
  }

  /** The object under the mouse, by the rules above (`nodes`: joints only). */
  pickAt(x: number, y: number, nodesOnly: boolean): PickRef | null {
    const { refs, pointRef, lineRef, triRef } = this.items;
    const p = this.nearestPoint(x, y);
    if (p >= 0) return refs[pointRef[p]];
    if (nodesOnly) return null;
    const [line, edge] = this.nearestLines(x, y);
    if (line >= 0) return refs[lineRef[line]];
    if (edge >= 0) return refs[lineRef[edge]];
    const [t] = this.faceAt(x, y);
    return t >= 0 ? refs[triRef[t]] : null;
  }

  /** The model point under the mouse (a joint, the nearest point of a member or edge, a face's point), or null. */
  pointAt(x: number, y: number): [number, number, number] | null {
    const q = this.items.xyz;
    const p = this.nearestPoint(x, y);
    if (p >= 0) {
      const o = this.pointAt3(p);
      return [q[o], q[o + 1], q[o + 2]];
    }
    // The nearest line of any kind.
    let best = -1;
    let bestD = LINE_PICK_PX;
    let bestT = 0;
    for (const i of this.candidates(this.gl, x, y, LINE_PICK_PX)) {
      const [a, b] = this.line3(i);
      const s = this.s;
      if (Number.isNaN(s[a]) || Number.isNaN(s[b])) continue;
      const [d, t] = segment(x, y, s[a], s[a + 1], s[b], s[b + 1]);
      if (d >= bestD) continue;
      [best, bestD, bestT] = [i, d, t];
    }
    if (best >= 0) {
      const [a, b] = this.line3(best);
      return [0, 1, 2].map((k) => q[a + k] + bestT * (q[b + k] - q[a + k])) as [number, number, number];
    }
    const [t, w] = this.faceAt(x, y);
    if (t < 0 || !w) return null;
    const [a, b, c] = this.triangle3(t);
    return [0, 1, 2].map((k) => w[0] * q[a + k] + w[1] * q[b + k] + w[2] * q[c + k]) as [number, number, number];
  }

  /** Index of the nearest joint within NODE_PICK_PX, or -1. */
  private nearestPoint(x: number, y: number): number {
    let best = -1;
    let bestD = NODE_PICK_PX;
    for (const i of this.candidates(this.gp, x, y, 0)) {
      const o = this.pointAt3(i);
      const sx = this.s[o];
      if (Number.isNaN(sx)) continue;
      const d = Math.hypot(sx - x, this.s[o + 1] - y);
      if (d < bestD) [best, bestD] = [i, d];
    }
    return best;
  }

  /** The nearest member or link line and the nearest area edge within LINE_PICK_PX (-1 when none). */
  private nearestLines(x: number, y: number): [number, number] {
    const { refs, lineRef } = this.items;
    let best = -1;
    let bestD = LINE_PICK_PX;
    let edge = -1;
    let edgeD = LINE_PICK_PX;
    for (const i of this.candidates(this.gl, x, y, LINE_PICK_PX)) {
      const [a, b] = this.line3(i);
      const s = this.s;
      if (Number.isNaN(s[a]) || Number.isNaN(s[b])) continue;
      const [d] = segment(x, y, s[a], s[a + 1], s[b], s[b + 1]);
      if (refs[lineRef[i]].kind === "area") {
        if (d < edgeD) [edge, edgeD] = [i, d];
      } else if (d < bestD) [best, bestD] = [i, d];
    }
    return [best, edge];
  }

  /** The nearest triangle under the mouse and its weights, or -1. */
  private faceAt(x: number, y: number): [number, [number, number, number] | null] {
    let best = -1;
    let bestW: [number, number, number] | null = null;
    let depth = Infinity;
    const s = this.s;
    for (const i of this.candidates(this.gt, x, y, 0)) {
      const [a, b, c] = this.triangle3(i);
      if (Number.isNaN(s[a]) || Number.isNaN(s[b]) || Number.isNaN(s[c])) continue;
      const w = barycentric(x, y, s[a], s[a + 1], s[b], s[b + 1], s[c], s[c + 1]);
      if (!w) continue;
      const z = w[0] * s[a + 2] + w[1] * s[b + 2] + w[2] * s[c + 2];
      if (z < depth) [best, bestW, depth] = [i, w, z];
    }
    return [best, bestW];
  }

  /** Items of a grid in the cells within `r` px of (x, y), each once, in scene order. */
  private candidates(g: Grid, x: number, y: number, r: number): Int32Array | number[] {
    const out: number[] = [];
    const v = ++this.stampValue;
    const take = (i: number) => {
      if (this.stamp[i] === v) return;
      this.stamp[i] = v;
      out.push(i);
    };
    this.cellsOfBox(x - r, y - r, x + r, y + r, (c) => {
      for (let k = g.start[c]; k < g.start[c + 1]; k++) take(g.items[k]);
    });
    for (const i of g.wide) take(i);
    return out.sort((a, b) => a - b);
  }

  /** Calls `add` with every cell a box reaches (clamped to the view). */
  private cellsOfBox(x0: number, y0: number, x1: number, y1: number, add: (cell: number) => void) {
    const c0 = clamp(Math.floor(x0 / CELL), 0, this.cols - 1);
    const c1 = clamp(Math.floor(x1 / CELL), 0, this.cols - 1);
    const r0 = clamp(Math.floor(y0 / CELL), 0, this.rows - 1);
    const r1 = clamp(Math.floor(y1 / CELL), 0, this.rows - 1);
    for (let r = r0; r <= r1; r++) for (let c = c0; c <= c1; c++) add(r * this.cols + c);
  }

  /** Cells within LINE_PICK_PX of a line (cells near it by their centres, a little more than needed). */
  private cellsOfLine(i: number, add: (cell: number) => void) {
    const s = this.s;
    const [a, b] = this.line3(i);
    const [ax, ay, bx, by] = [s[a], s[a + 1], s[b], s[b + 1]];
    if (Number.isNaN(ax) || Number.isNaN(bx)) return;
    const m = LINE_PICK_PX;
    const reach = m + CELL * 0.7072;
    this.cellsOfBox(Math.min(ax, bx) - m, Math.min(ay, by) - m, Math.max(ax, bx) + m, Math.max(ay, by) + m, (c) => {
      const cx = ((c % this.cols) + 0.5) * CELL;
      const cy = (Math.floor(c / this.cols) + 0.5) * CELL;
      if (segment(cx, cy, ax, ay, bx, by)[0] <= reach) add(c);
    });
  }

  /** Sorts `n` items into cells (two passes: count, then fill); items reaching too many cells go to `wide`. */
  private grid(n: number, cells: (i: number, add: (cell: number) => void) => void): Grid {
    const count = new Int32Array(this.cols * this.rows + 1);
    const reach = new Int32Array(n);
    for (let i = 0; i < n; i++) {
      let k = 0;
      cells(i, () => k++);
      reach[i] = k;
      if (k <= MAX_CELLS) cells(i, (c) => count[c + 1]++);
    }
    for (let c = 0; c < this.cols * this.rows; c++) count[c + 1] += count[c];
    const start = count.slice();
    const items = new Int32Array(count[this.cols * this.rows]);
    const fill = count.slice();
    const wide: number[] = [];
    for (let i = 0; i < n; i++) {
      if (reach[i] > MAX_CELLS) wide.push(i);
      else if (reach[i] > 0) cells(i, (c) => (items[fill[c]++] = i));
    }
    return { start, items, wide: Int32Array.from(wide) };
  }
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Distance from (x, y) to the segment a-b and the parameter of the nearest point. */
export function segment(x: number, y: number, ax: number, ay: number, bx: number, by: number): [number, number] {
  const dx = bx - ax;
  const dy = by - ay;
  const l2 = dx * dx + dy * dy;
  const t = l2 === 0 ? 0 : Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / l2));
  return [Math.hypot(x - (ax + t * dx), y - (ay + t * dy)), t];
}

/** Barycentric weights of screen point (x, y) in triangle a-b-c, or null when outside. */
export function barycentric(x: number, y: number, ax: number, ay: number, bx: number, by: number, cx: number, cy: number): [number, number, number] | null {
  const det = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
  if (Math.abs(det) < 1e-9) return null;
  const wa = ((by - cy) * (x - cx) + (cx - bx) * (y - cy)) / det;
  const wb = ((cy - ay) * (x - cx) + (ax - cx) * (y - cy)) / det;
  const wc = 1 - wa - wb;
  return wa >= 0 && wb >= 0 && wc >= 0 ? [wa, wb, wc] : null;
}
