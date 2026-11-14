// One scene on the graphics card. Its faces, lines and joint dots are merged into a few buffers (faces of
// a kind in one, lines by width, all dots in one, symbols by kind, arrow heads as copies of one cone), each
// with its colour per vertex or per line, so a scene of many objects is a few draw calls. A selection changes
// colours, dashes and sizes in those buffers (`setStyles`); a mode animation moves the vertices through a
// uniform. See-through faces are drawn in one pass, sorted back to front object by object as the renderer
// sorts separate objects, so they blend in the same order. Lines keep the order they had in groups of one
// colour, width and dash (in the order the groups first come; neighbouring groups of one width share a
// buffer), after the faces and arrow heads, which decides what shows where two lines lie on each other; joint
// dots keep their groups of one colour and size. Lines, dots and symbols are drawn before the see-through
// faces and at their depth, so solid objects in front hide them and see-through faces in front dim them.

import * as THREE from "three";
import { LineSegments2 } from "three/examples/jsm/lines/LineSegments2.js";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import { CSS2DObject } from "three/examples/jsm/renderers/CSS2DRenderer.js";

import type { Vec3 } from "../../model/types";
import { pickKey, type PickRef, type PickStyle, type SceneData } from "../../scene/types";
import type { MaterialPool } from "./materials";

/** Support symbols' size in CSS pixels. */
export const GLYPH_PX = 26;
/** Batches of see-through faces with more objects than this are sorted only when the view is at rest. */
const SORT_WHILE_MOVING = 50000;

const linear = new Map<string, [number, number, number]>();
/** A CSS colour in the renderer's working (linear) colour space, as a material's colour would be. */
export function rgb(color: string): [number, number, number] {
  let c = linear.get(color);
  if (!c) {
    const k = new THREE.Color(color);
    linear.set(color, (c = [k.r, k.g, k.b]));
  }
  return c;
}

interface LineItem {
  a: Vec3;
  b: Vec3;
  color: string;
  width: number;
  dashed: boolean;
  opacity: number;
  pick?: PickRef;
  motion?: [Vec3, Vec3];
}

interface LineBatch {
  object: LineSegments2;
  color: Float32Array;
  dash: Float32Array;
  baseColor: Float32Array;
  baseDash: Float32Array;
}

interface FaceBatch {
  object: THREE.Mesh;
  color: Float32Array;
  baseColor: Float32Array;
  transparent: boolean;
  /** Per object: first vertex, vertex count and middle (sorting). */
  start: number[];
  count: number[];
  middle: number[];
  index: THREE.BufferAttribute | null;
  /** The matrix of the last sort (projection x view), and whether every object has one colour. */
  sortedFor: Float64Array | null;
  oneColour: boolean;
}

/** Items of one object, as flat pairs: lines (batch, position), faces (batch, object in it) and dots (group, position). */
interface ObjectItems {
  lines: number[];
  faces: number[];
  dots: number[];
}

interface DotGroup {
  object: THREE.Points;
  color: Float32Array;
  size: Float32Array;
  baseColor: Float32Array;
  baseSize: Float32Array;
}

export class SceneLayer {
  readonly group = new THREE.Group();
  private lines: LineBatch[] = [];
  private faces: FaceBatch[] = [];
  private dots: DotGroup[] = [];
  private objects = new Map<string, ObjectItems>();
  private styled = new Map<string, PickStyle>();
  private disposables: { dispose(): void }[] = [];
  private opaque = 0;

  /**
   * `order`: the render order of the layer's first opaque part (the next ones follow it closely); `styled`:
   * selections are drawn on it (it keeps a copy of the built looks to go back to).
   */
  constructor(
    d: SceneData,
    private pool: MaterialPool,
    private order: number,
    private styled_ = true,
  ) {
    const motion = d.lines.some((l) => l.motion) || d.points.some((p) => p.motion) || d.meshes.some((m) => m.motion);
    this.buildFaces(d, motion);
    this.buildLines(d, motion);
    this.buildDots(d, motion);
    this.buildGlyphs(d);
    this.buildLabels(d);
    if (motion) this.group.traverse((o) => (o.frustumCulled = false));
  }

  /** A copy of a built look to go back to (none on a layer without selections). */
  private copy(a: Float32Array): Float32Array {
    return this.styled_ ? a.slice() : a;
  }

  /** Puts an opaque part after the layer's earlier ones. */
  private inOrder(o: THREE.Object3D) {
    o.renderOrder = this.order + 1e-6 * this.opaque++;
  }

  private items(p: PickRef | undefined): ObjectItems | null {
    if (!p) return null;
    const key = pickKey(p);
    let it = this.objects.get(key);
    if (!it) this.objects.set(key, (it = { lines: [], faces: [], dots: [] }));
    return it;
  }

  // ---- building ----

  private buildFaces(d: SceneData, motion: boolean) {
    // Batches by kind in the order they first come: opaque and see-through, lit, contour.
    const kinds = new Map<string, number[]>();
    d.meshes.forEach((m, i) => {
      if (m.positions.length < 9) return;
      const key = `${!!m.shaded}|${m.opacity < 1}|${!!m.contour}`;
      const list = kinds.get(key);
      if (list) list.push(i);
      else kinds.set(key, [i]);
    });
    for (const [key, list] of kinds) {
      const [shaded, transparent, contour] = key.split("|").map((x) => x === "true");
      let n = 0;
      for (const i of list) n += d.meshes[i].positions.length / 3;
      const position = new Float32Array(3 * n);
      const color = new Float32Array(4 * n);
      const moved = motion ? new Float32Array(3 * n) : null;
      const cuv = contour ? new Float32Array(2 * n) : null;
      const cramp = contour ? new Float32Array(4 * n) : null;
      const ctri = contour ? new Float32Array(n) : null;
      const start: number[] = [];
      const count: number[] = [];
      const middle: number[] = [];
      const b = this.faces.length;
      let v = 0;
      for (const i of list) {
        const m = d.meshes[i];
        const k = m.positions.length / 3;
        position.set(m.positions, 3 * v);
        const [r, g, bl] = rgb(m.colors ? "#ffffff" : m.color);
        let lo = [Infinity, Infinity, Infinity];
        let hi = [-Infinity, -Infinity, -Infinity];
        for (let j = 0; j < k; j++) {
          const c = m.colors;
          color.set(c ? [c[3 * j], c[3 * j + 1], c[3 * j + 2], m.opacity] : [r, g, bl, m.opacity], 4 * (v + j));
          for (let a = 0; a < 3; a++) {
            const x = m.positions[3 * j + a];
            lo[a] = Math.min(lo[a], x);
            hi[a] = Math.max(hi[a], x);
          }
        }
        if (moved && m.motion) moved.set(m.motion, 3 * v);
        if (m.contour && cuv && cramp && ctri) {
          cuv.set(m.contour.uv, 2 * v);
          cramp.set(m.contour.ramp, 4 * v);
          ctri.fill(m.contour.triangle ? 1 : 0, v, v + k);
        }
        // The middle of its box, where the renderer measures an object's depth.
        middle.push((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2);
        start.push(v);
        count.push(k);
        this.items(m.pick)?.faces.push(b, start.length - 1);
        v += k;
      }
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(position, 3));
      g.setAttribute("color", new THREE.BufferAttribute(color, 4));
      if (moved) g.setAttribute("motion", new THREE.BufferAttribute(moved, 3));
      if (cuv && cramp && ctri) {
        g.setAttribute("cuv", new THREE.BufferAttribute(cuv, 2));
        g.setAttribute("cramp", new THREE.BufferAttribute(cramp, 4));
        g.setAttribute("ctri", new THREE.BufferAttribute(ctri, 1));
      }
      if (shaded) g.computeVertexNormals();
      let index: THREE.BufferAttribute | null = null;
      if (transparent) {
        // Drawn through an index, so the objects can be put in depth order.
        const ix = new Uint32Array(n);
        for (let j = 0; j < n; j++) ix[j] = j;
        index = new THREE.BufferAttribute(ix, 1);
        g.setIndex(index);
      }
      const object = new THREE.Mesh(g, this.pool.faces({ shaded, transparent, contour, motion }));
      if (transparent) object.renderOrder = -1;
      else this.inOrder(object);
      this.group.add(object);
      this.disposables.push(g);
      const batch: FaceBatch = { object, color, baseColor: this.copy(color), transparent, start, count, middle, index, sortedFor: null, oneColour: true };
      batch.oneColour = oneColour(batch);
      this.faces.push(batch);
    }
  }

  private buildLines(d: SceneData, motion: boolean) {
    const items: LineItem[] = d.lines.map((l) => ({ ...l, dashed: !!l.dashed, opacity: l.opacity ?? 1 }));
    for (const ar of d.arrows) {
      if (ar.tail.every((v, k) => v === ar.head[k])) continue;
      items.push({ a: ar.tail, b: ar.head, color: ar.color, width: 2, dashed: false, opacity: 1 });
    }
    // Arrow heads: copies of one cone (radius 0.07 and height 0.22 of the arrow, its tip at the head).
    const heads: { at: THREE.Vector3; dir: THREE.Vector3; len: number; color: string }[] = [];
    for (const ar of d.arrows) {
      const tail = new THREE.Vector3(...ar.tail);
      const head = new THREE.Vector3(...ar.head);
      const dir = head.clone().sub(tail);
      const len = dir.length();
      if (len === 0) continue;
      dir.normalize();
      heads.push({ at: head, dir, len, color: ar.color });
      if (ar.double) heads.push({ at: head.clone().addScaledVector(dir, -len * 0.22), dir, len, color: ar.color });
    }
    if (heads.length) {
      const cone = new THREE.ConeGeometry(1, 1, 12);
      const mesh = new THREE.InstancedMesh(cone, this.pool.cones(), heads.length);
      const q = new THREE.Quaternion();
      const up = new THREE.Vector3(0, 1, 0);
      const m = new THREE.Matrix4();
      heads.forEach((h, i) => {
        q.setFromUnitVectors(up, h.dir);
        const p = h.at.clone().addScaledVector(h.dir, -h.len * 0.11);
        m.compose(p, q, new THREE.Vector3(h.len * 0.07, h.len * 0.22, h.len * 0.07));
        mesh.setMatrixAt(i, m);
        mesh.setColorAt(i, new THREE.Color(h.color));
      });
      mesh.computeBoundingSphere();
      this.inOrder(mesh);
      this.group.add(mesh);
      this.disposables.push(cone, mesh);
    }
    // Groups of one colour, width, dash and opacity in the order they first come (each was drawn on its own);
    // neighbouring groups of one width and opacity share a buffer, group after group.
    const groups = new Map<string, number[]>();
    items.forEach((l, i) => {
      const key = `${l.color}|${l.width}|${l.dashed}|${l.opacity}`;
      const list = groups.get(key);
      if (list) list.push(i);
      else groups.set(key, [i]);
    });
    const batches: number[][][] = [];
    let last = "";
    for (const list of groups.values()) {
      const l = items[list[0]];
      const kind = `${l.width}|${l.opacity}`;
      if (kind === last) batches[batches.length - 1].push(list);
      else batches.push([list]);
      last = kind;
    }
    for (const parts of batches) {
      const n = parts.reduce((s, x) => s + x.length, 0);
      const position = new Float32Array(6 * n);
      const color = new Float32Array(6 * n);
      const dist = new Float32Array(2 * n);
      const dash = new Float32Array(n);
      const moved = motion ? new Float32Array(6 * n) : null;
      const b = this.lines.length;
      let k = 0;
      for (const list of parts) {
        const g0 = k;
        for (const i of list) {
          const l = items[i];
          position.set(l.a, 6 * k);
          position.set(l.b, 6 * k + 3);
          const c = rgb(l.color);
          color.set(c, 6 * k);
          color.set(c, 6 * k + 3);
          dash[k] = l.dashed ? 1 : 0;
          if (moved && l.motion) {
            moved.set(l.motion[0], 6 * k);
            moved.set(l.motion[1], 6 * k + 3);
          }
          this.items(l.pick)?.lines.push(b, k);
          k++;
        }
        // Dashes run on along the group's lines, as `LineSegments2.computeLineDistances` measures them.
        for (let j = g0; j < k; j++) {
          const o = 6 * j;
          const len = Math.sqrt((position[o + 3] - position[o]) ** 2 + (position[o + 4] - position[o + 1]) ** 2 + (position[o + 5] - position[o + 2]) ** 2);
          dist[2 * j] = j === g0 ? 0 : dist[2 * j - 1];
          dist[2 * j + 1] = dist[2 * j] + len;
        }
      }
      const g = new LineSegmentsGeometry();
      g.setPositions(position);
      g.setColors(color);
      const db = new THREE.InstancedInterleavedBuffer(dist, 2, 1);
      g.setAttribute("instanceDistanceStart", new THREE.InterleavedBufferAttribute(db, 1, 0));
      g.setAttribute("instanceDistanceEnd", new THREE.InterleavedBufferAttribute(db, 1, 1));
      g.setAttribute("instanceDash", new THREE.InstancedBufferAttribute(dash, 1));
      if (moved) {
        const mb = new THREE.InstancedInterleavedBuffer(moved, 6, 1);
        g.setAttribute("instanceMotionStart", new THREE.InterleavedBufferAttribute(mb, 3, 0));
        g.setAttribute("instanceMotionEnd", new THREE.InterleavedBufferAttribute(mb, 3, 3));
      }
      const first = items[parts[0][0]];
      const object = new LineSegments2(g, this.pool.line(first.width, first.opacity, motion));
      if (first.opacity < 1) object.renderOrder = 0;
      else this.inOrder(object);
      this.group.add(object);
      this.disposables.push(g);
      this.lines.push({ object, color, dash, baseColor: this.copy(color), baseDash: this.copy(dash) });
    }
  }

  private buildDots(d: SceneData, motion: boolean) {
    // Groups of one colour and size, each drawn on its own as before (the renderer orders them by depth).
    const groups = new Map<string, number[]>();
    d.points.forEach((p, i) => {
      const key = `${p.color}|${p.size}`;
      const list = groups.get(key);
      if (list) list.push(i);
      else groups.set(key, [i]);
    });
    for (const list of groups.values()) {
      const n = list.length;
      const position = new Float32Array(3 * n);
      const color = new Float32Array(3 * n);
      const size = new Float32Array(n);
      const moved = motion ? new Float32Array(3 * n) : null;
      const gi = this.dots.length;
      list.forEach((i, k) => {
        const p = d.points[i];
        position.set(p.position, 3 * k);
        color.set(rgb(p.color), 3 * k);
        size[k] = p.size;
        if (moved && p.motion) moved.set(p.motion, 3 * k);
        this.items(p.pick)?.dots.push(gi, k);
      });
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(position, 3));
      g.setAttribute("color", new THREE.BufferAttribute(color, 3));
      g.setAttribute("psize", new THREE.BufferAttribute(size, 1));
      if (moved) g.setAttribute("motion", new THREE.BufferAttribute(moved, 3));
      const object = new THREE.Points(g, this.pool.dots(motion));
      // Before the see-through faces, which then dim the dots behind them.
      object.renderOrder = -2;
      this.group.add(object);
      this.disposables.push(g);
      this.dots.push({ object, color, size, baseColor: this.copy(color), baseSize: this.copy(size) });
    }
  }

  private buildGlyphs(d: SceneData) {
    const kinds = new Map<string, { kind: (typeof d.glyphs)[number]["kind"]; color: string; at: number[] }>();
    for (const gl of d.glyphs) {
      const key = `${gl.kind}|${gl.color}`;
      let k = kinds.get(key);
      if (!k) kinds.set(key, (k = { kind: gl.kind, color: gl.color, at: [] }));
      k.at.push(...gl.position);
    }
    for (const k of kinds.values()) {
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.Float32BufferAttribute(k.at, 3));
      const object = new THREE.Points(g, this.pool.glyph(k.kind, k.color, GLYPH_PX));
      // Before the see-through faces, which then dim the symbols behind them.
      object.renderOrder = -3;
      this.group.add(object);
      this.disposables.push(g);
    }
  }

  /** How many texts the layer has. */
  get labelCount() {
    return this.texts;
  }
  private texts = 0;

  private buildLabels(d: SceneData) {
    this.texts = d.labels.length;
    for (const lb of d.labels) {
      const div = document.createElement("div");
      div.className = `lbl ${lb.className}`;
      div.textContent = lb.text;
      const obj = new CSS2DObject(div);
      obj.position.set(...lb.position);
      if (lb.className === "lbl-node") obj.center.set(-0.25, 1.15);
      else if (lb.className === "lbl-elem") obj.center.set(0.5, 1.2);
      // A diaphragm's name: below and to the right of its centre dot, clear of member texts above it.
      else if (lb.className === "lbl-dia") obj.center.set(-0.2, -0.3);
      this.group.add(obj);
    }
  }

  // ---- selection ----

  /** Draws these objects in their style and every other object as built. */
  setStyles(styles: Map<string, PickStyle>) {
    if (!this.styled_) return;
    const keys = new Set([...this.styled.keys(), ...styles.keys()]);
    const touched = { lines: new Set<number>(), faces: new Set<number>(), dots: new Set<number>() };
    for (const key of keys) {
      const st = styles.get(key);
      if (sameStyle(st, this.styled.get(key))) continue;
      const it = this.objects.get(key);
      if (!it) continue;
      const c = st && rgb(st.color);
      for (const [b, k] of pairs(it.lines)) {
        const L = this.lines[b];
        for (const o of [6 * k, 6 * k + 3]) for (let a = 0; a < 3; a++) L.color[o + a] = c ? c[a] : L.baseColor[o + a];
        L.dash[k] = st?.dashed === undefined ? L.baseDash[k] : st.dashed ? 1 : 0;
        touched.lines.add(b);
      }
      for (const [b, k] of pairs(it.faces)) {
        const F = this.faces[b];
        for (let v = F.start[k]; v < F.start[k] + F.count[k]; v++) {
          for (let a = 0; a < 3; a++) F.color[4 * v + a] = c ? c[a] : F.baseColor[4 * v + a];
          F.color[4 * v + 3] = st?.opacity ?? F.baseColor[4 * v + 3];
        }
        touched.faces.add(b);
      }
      for (const [gi, i] of pairs(it.dots)) {
        const D = this.dots[gi];
        for (let a = 0; a < 3; a++) D.color[3 * i + a] = c ? c[a] : D.baseColor[3 * i + a];
        D.size[i] = st?.size ?? D.baseSize[i];
        touched.dots.add(gi);
      }
    }
    for (const b of touched.lines) {
      const g = this.lines[b].object.geometry;
      (g.getAttribute("instanceColorStart") as THREE.InterleavedBufferAttribute).data.needsUpdate = true;
      g.getAttribute("instanceDash").needsUpdate = true;
    }
    for (const b of touched.faces) {
      const F = this.faces[b];
      F.object.geometry.getAttribute("color").needsUpdate = true;
      F.oneColour = oneColour(F);
      F.sortedFor = null;
    }
    for (const gi of touched.dots) {
      this.dots[gi].object.geometry.getAttribute("color").needsUpdate = true;
      this.dots[gi].object.geometry.getAttribute("psize").needsUpdate = true;
    }
    this.styled = new Map(styles);
  }

  // ---- see-through faces in depth order ----

  /**
   * Puts the see-through faces of each batch in back-to-front order of their objects' middles for the view
   * matrix `projView` (projection x camera inverse), ties in scene order, as the renderer orders separate
   * objects. Batches with more than SORT_WHILE_MOVING objects only when `atRest`. Returns whether a batch
   * still waits for the view to rest.
   */
  sortFaces(projView: THREE.Matrix4, atRest: boolean): boolean {
    let waiting = false;
    const e = projView.elements;
    for (const F of this.faces) {
      if (!F.transparent || !F.index || F.oneColour) continue;
      if (F.sortedFor && F.sortedFor.every((v, i) => v === e[i])) continue;
      const n = F.start.length;
      if (n > SORT_WHILE_MOVING && !atRest) {
        waiting = true;
        continue;
      }
      const z = new Float64Array(n);
      for (let k = 0; k < n; k++) z[k] = e[2] * F.middle[3 * k] + e[6] * F.middle[3 * k + 1] + e[10] * F.middle[3 * k + 2] + e[14];
      const order = new Uint32Array(n);
      for (let k = 0; k < n; k++) order[k] = k;
      order.sort((a, b) => (z[a] !== z[b] ? z[b] - z[a] : a - b));
      const ix = F.index.array as Uint32Array;
      let w = 0;
      for (const k of order) for (let v = F.start[k]; v < F.start[k] + F.count[k]; v++) ix[w++] = v;
      F.index.needsUpdate = true;
      F.sortedFor = Float64Array.from(e);
    }
    return waiting;
  }

  // ---- what the hover highlight draws ----

  /** Line ends of an object (6 per line) with their displacements, if any. */
  objectLines(key: string): { position: number[]; motion: number[] | null } {
    const position: number[] = [];
    const motion: number[] = [];
    let moving = false;
    for (const [b, k] of pairs(this.objects.get(key)?.lines ?? [])) {
      const g = this.lines[b].object.geometry;
      const p = (g.getAttribute("instanceStart") as THREE.InterleavedBufferAttribute).data.array;
      for (let a = 0; a < 6; a++) position.push(p[6 * k + a]);
      const m = g.getAttribute("instanceMotionStart") as THREE.InterleavedBufferAttribute | undefined;
      if (m) moving = true;
      for (let a = 0; a < 6; a++) motion.push(m ? m.data.array[6 * k + a] : 0);
    }
    return { position, motion: moving ? motion : null };
  }

  /** Face corners of an object (9 per triangle) with their displacements, if any. */
  objectTriangles(key: string): { position: number[]; motion: number[] | null } {
    const position: number[] = [];
    const motion: number[] = [];
    let moving = false;
    for (const [b, k] of pairs(this.objects.get(key)?.faces ?? [])) {
      const F = this.faces[b];
      const p = F.object.geometry.getAttribute("position").array;
      const m = F.object.geometry.getAttribute("motion");
      if (m) moving = true;
      for (let v = F.start[k]; v < F.start[k] + F.count[k]; v++) {
        for (let a = 0; a < 3; a++) {
          position.push(p[3 * v + a]);
          motion.push(m ? m.array[3 * v + a] : 0);
        }
      }
    }
    return { position, motion: moving ? motion : null };
  }

  /** The first dot of an object and its displacement, if any. */
  objectDot(key: string): { position: Vec3; motion: Vec3 } | null {
    const at = this.objects.get(key)?.dots;
    if (!at?.length) return null;
    const [gi, i] = [at[0], at[1]];
    const g = this.dots[gi].object.geometry;
    const p = g.getAttribute("position").array;
    const m = g.getAttribute("motion")?.array;
    return { position: [p[3 * i], p[3 * i + 1], p[3 * i + 2]], motion: m ? [m[3 * i], m[3 * i + 1], m[3 * i + 2]] : [0, 0, 0] };
  }

  /**
   * Makes each drawn object again over the same buffers and materials (texts stay), so they come after
   * objects made since: the renderer breaks ties in its draw order by when objects were made, and the
   * results layer kept while the model's layer is built again must stay after it.
   */
  renew() {
    const again = new Map<THREE.Object3D, THREE.Object3D>();
    for (const o of [...this.group.children]) {
      let n: THREE.Object3D;
      if (o instanceof CSS2DObject) continue;
      else if (o instanceof THREE.InstancedMesh) {
        const m = new THREE.InstancedMesh(o.geometry, o.material, o.count);
        m.instanceMatrix = o.instanceMatrix;
        m.instanceColor = o.instanceColor;
        m.boundingSphere = o.boundingSphere;
        this.disposables[this.disposables.indexOf(o)] = m;
        n = m;
      } else if (o instanceof LineSegments2) n = new LineSegments2(o.geometry, o.material);
      else if (o instanceof THREE.Mesh) n = new THREE.Mesh(o.geometry, o.material);
      else if (o instanceof THREE.Points) n = new THREE.Points(o.geometry, o.material);
      else continue;
      n.renderOrder = o.renderOrder;
      n.frustumCulled = o.frustumCulled;
      again.set(o, n);
    }
    // In the group's order, texts where they were.
    const children = this.group.children.map((o) => again.get(o) ?? o);
    for (const o of [...this.group.children]) this.group.remove(o);
    this.group.add(...children);
    for (const L of this.lines) L.object = again.get(L.object) as LineSegments2;
    for (const F of this.faces) F.object = again.get(F.object) as THREE.Mesh;
    for (const D of this.dots) D.object = again.get(D.object) as THREE.Points;
  }

  dispose() {
    this.group.removeFromParent();
    // Labels are DOM elements: removing their objects removes them from the page on the next render.
    for (const o of [...this.group.children]) this.group.remove(o);
    for (const x of this.disposables) x.dispose();
    this.disposables = [];
  }
}

/** The pairs of a flat list (a, b, a, b...). */
function* pairs(list: number[]): Generator<[number, number]> {
  for (let j = 0; j + 1 < list.length; j += 2) yield [list[j], list[j + 1]];
}

function sameStyle(a: PickStyle | undefined, b: PickStyle | undefined): boolean {
  if (!a || !b) return a === b;
  return a.color === b.color && a.dashed === b.dashed && a.opacity === b.opacity && a.size === b.size;
}

/** Whether every object of a face batch has the same colour (then their order does not change the blend). */
function oneColour(F: FaceBatch): boolean {
  const c = F.color;
  for (let i = 4; i < c.length; i++) if (c[i] !== c[i % 4]) return false;
  return true;
}
