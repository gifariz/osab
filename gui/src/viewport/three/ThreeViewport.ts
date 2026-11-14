// three.js implementation of the Viewport. Each scene is drawn as a few merged buffers (`layer.ts`), so a
// large model is a few draw calls; a selection changes their colours, a mode animation a uniform. Picking
// works in screen space (distance in pixels to projected joints and member segments, then the nearest picked
// triangle under the mouse), so thick lines and every view behave the same; the projections are made once
// per camera position and searched through a grid of screen cells (`picking.ts`).

import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { LineMaterial } from "three/examples/jsm/lines/LineMaterial.js";
import { LineSegments2 } from "three/examples/jsm/lines/LineSegments2.js";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import { CSS2DRenderer } from "three/examples/jsm/renderers/CSS2DRenderer.js";

import type { Vec3 } from "../../model/types";
import { pickKey, type Bounds, type PickRef, type PickStyle, type SceneData } from "../../scene/types";
import type { ViewSpec } from "../../state/store";
import type { Interaction, Viewport, ViewportEvents } from "../Viewport";
import { t } from "../../i18n";
import { dotTexture } from "./icons";
import { NavCube } from "./navCube";
import { SceneLayer, rgb } from "./layer";
import { MaterialPool, type Shared } from "./materials";
import { PickItemsBuilder, ScreenPicker, barycentric, joinPickItems, noPickItems, projectAll, type PickItems } from "./picking";

const SNAP_PX = 14;
const CLICK_PX = 5;
/** After the view stops moving: sort large batches of see-through faces, show many texts again (ms). */
const REST_MS = 150;
/** With more texts than this, they are hidden while the view moves (each frame would place every one). */
const TEXTS_WHILE_MOVING = 500;
/** 3D orbit: a drag across the whole view turns it half round; the view stops 5 degrees short of straight down or up. */
const ORBIT_TURN = Math.PI;
const ORBIT_MAX_ELEVATION = THREE.MathUtils.degToRad(85);
const Z_UP = new THREE.Vector3(0, 0, 1);
/** How long a turn of the 3D view from the navigation cube takes (ms). */
const TURN_MS = 300;
/** Top and Bottom from the navigation cube stop this short of vertical (rad), so Z stays a usable up. */
const TURN_MAX_ELEVATION = THREE.MathUtils.degToRad(89.9);

type Listeners = { [K in keyof ViewportEvents]: Set<ViewportEvents[K]> };

/** The pickable joints, lines and triangles of a scene, in scene order. */
function pickItems(d: SceneData): PickItems {
  let [np, nl, nt] = [0, 0, 0];
  for (const p of d.points) if (p.pick) np++;
  for (const l of d.lines) if (l.pick) nl++;
  for (const m of d.meshes) if (m.pick) nt += Math.floor(m.positions.length / 9);
  const it = new PickItemsBuilder(np, nl, nt);
  for (const p of d.points) if (p.pick) it.addPoint(p.position, it.ref(p.pick));
  for (const l of d.lines) if (l.pick) it.addLine(l.a, l.b, it.ref(l.pick));
  for (const m of d.meshes) {
    if (!m.pick) continue;
    const r = it.ref(m.pick);
    const q = m.positions;
    for (let i = 0; i + 8 < q.length; i += 9) it.addTriangle(q, i, r);
  }
  return it.done();
}

/** Camera direction (towards the scene) and up vector of each 2D view. */
function viewAxes(kind: ViewSpec["kind"]): { forward: THREE.Vector3; up: THREE.Vector3 } {
  switch (kind) {
    case "plan":
      return { forward: new THREE.Vector3(0, 0, -1), up: new THREE.Vector3(0, 1, 0) };
    case "elev-xz":
      return { forward: new THREE.Vector3(0, 1, 0), up: new THREE.Vector3(0, 0, 1) };
    case "elev-yz":
      return { forward: new THREE.Vector3(-1, 0, 0), up: new THREE.Vector3(0, 0, 1) };
    default:
      return { forward: new THREE.Vector3(0.6, 0.85, -0.5).normalize(), up: new THREE.Vector3(0, 0, 1) };
  }
}

export class ThreeViewport implements Viewport {
  private renderer: THREE.WebGLRenderer;
  private labels: CSS2DRenderer;
  private scene = new THREE.Scene();
  private persp = new THREE.PerspectiveCamera(40, 1, 0.01, 1000);
  private ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.001, 1000);
  private camera: THREE.Camera = this.persp;
  /** The navigation cube of the 3D view (its colours follow the theme). */
  private navCube: NavCube | null = null;
  private navCubeDark = true;
  /** The 3D view in parallel projection (the orthographic camera) instead of perspective. */
  private parallel = false;
  /** A turn of the 3D view in progress (the navigation cube): its frame request. */
  private turnFrame = 0;
  private controls: OrbitControls;
  private hoverGroup = new THREE.Group();
  private helpers = new THREE.Group();
  /** Lines of the viewport's own (rubber band, hover highlight), sized on resize. */
  private lineMaterials = new Set<LineMaterial>();
  private shared: Shared = {
    view: { value: new THREE.Vector2(1, 1) },
    swing: { value: 1 },
    dashSize: { value: 1 },
    gapSize: { value: 1 },
  };
  private pool = new MaterialPool(this.shared);
  private baseLayer: SceneLayer | null = null;
  private overlayLayer: SceneLayer | null = null;
  /** What the layers were built from (the model's items, the function that gave the results' items). */
  private baseScene: SceneData | null = null;
  private overlaySource: (() => SceneData) | null = null;
  /** Pick items of each layer, and of both (results first). */
  private baseItems: PickItems = noPickItems();
  private overlayItems: PickItems | null = null;
  private styles = new Map<string, PickStyle>();
  private items: PickItems = noPickItems();
  /** The pick search for the current camera (made when first needed after the camera moves). */
  private picker: ScreenPicker | null = null;
  private pickerFor: number[] = [];
  private restTimer = 0;
  private lastMove = 0;
  /** The camera and scene the texts were last placed for. */
  private textsFor: number[] = [];
  private headlight = new THREE.DirectionalLight(0xffffff, 1.6);
  private snapPts: Vec3[] = [];
  private snapScreen: { pts: Float64Array; screen: Float64Array; for: number[] } | null = null;
  private interaction: Interaction = { pick: "all", snap: false, step: 1e-4, box: true, rubber: [], rubberClosed: false };
  private view: ViewSpec = { kind: "3d", at: 0 };
  private bounds: Bounds = { min: [0, 0, 0], max: [1, 1, 1] };
  private hover: PickRef | null = null;
  private snap: Vec3 | null = null;
  private down: { x: number; y: number; button: number } | null = null;
  /** A 3D orbit in progress (right-drag): the last mouse position and the point it turns about. */
  private orbit: { x: number; y: number; pivot: THREE.Vector3 } | null = null;
  private boxDiv: HTMLDivElement;
  private rubber: LineSegments2;
  private snapMarker: THREE.Points;
  private pivotMarker: THREE.Points;
  private listeners: Listeners = { hover: new Set(), click: new Set(), box: new Set(), cursor: new Set(), finish: new Set() };
  private resizeObserver: ResizeObserver;
  private renderPending = false;
  private width = 1;
  private height = 1;
  private cleanup: (() => void)[] = [];
  private hoverColor = "#5ce1e6";

  constructor(private container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setClearColor("#0d1117");
    this.renderer.domElement.style.display = "block";
    container.appendChild(this.renderer.domElement);
    this.labels = new CSS2DRenderer();
    // zIndex makes the label layer its own stacking context, so the labels' own z-order stays below dialogs.
    Object.assign(this.labels.domElement.style, { position: "absolute", left: "0", top: "0", pointerEvents: "none", zIndex: "1" });
    container.appendChild(this.labels.domElement);
    this.boxDiv = document.createElement("div");
    this.boxDiv.className = "select-box";
    this.boxDiv.style.display = "none";
    container.appendChild(this.boxDiv);

    this.scene.add(this.hoverGroup, this.helpers);
    // Shaded meshes (extruded members and areas) get soft ambient light and a light from the camera.
    this.scene.add(new THREE.AmbientLight(0xffffff, 1.1), this.headlight, this.headlight.target);
    this.rubber = this.makeLines([], this.hoverColor, 1.5, true);
    this.rubber.visible = false;
    this.helpers.add(this.rubber);
    const markerGeom = new THREE.BufferGeometry();
    markerGeom.setAttribute("position", new THREE.Float32BufferAttribute([0, 0, 0], 3));
    this.snapMarker = new THREE.Points(
      markerGeom,
      new THREE.PointsMaterial({ color: this.hoverColor, size: 14, sizeAttenuation: false, map: dotTexture(), transparent: true, opacity: 0.65, depthTest: false }),
    );
    this.snapMarker.renderOrder = 10;
    this.snapMarker.visible = false;
    this.helpers.add(this.snapMarker);
    const pivotGeom = new THREE.BufferGeometry();
    pivotGeom.setAttribute("position", new THREE.Float32BufferAttribute([0, 0, 0], 3));
    this.pivotMarker = new THREE.Points(
      pivotGeom,
      new THREE.PointsMaterial({ color: this.hoverColor, size: 11, sizeAttenuation: false, map: dotTexture(), transparent: true, opacity: 0.9, depthTest: false }),
    );
    this.pivotMarker.renderOrder = 11;
    this.pivotMarker.visible = false;
    this.helpers.add(this.pivotMarker);

    this.controls = this.makeControls();
    const canvas = this.renderer.domElement;
    const on = <K extends keyof HTMLElementEventMap>(t: K, f: (e: HTMLElementEventMap[K]) => void) => {
      canvas.addEventListener(t, f as EventListener);
      this.cleanup.push(() => canvas.removeEventListener(t, f as EventListener));
    };
    on("pointermove", (e) => this.onMove(e));
    on("pointerdown", (e) => this.onDown(e));
    on("pointerup", (e) => this.onUp(e));
    on("pointerleave", () => this.onLeave());
    on("contextmenu", (e) => e.preventDefault());
    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(container);
    this.makeNavCube(true);
    this.resize();
  }

  private makeNavCube(dark: boolean) {
    this.navCube?.dispose();
    this.navCubeDark = dark;
    const labels = {
      top: t("cube.top"),
      bottom: t("cube.bottom"),
      front: t("cube.front"),
      back: t("cube.back"),
      left: t("cube.left"),
      right: t("cube.right"),
    };
    this.navCube = new NavCube(
      this.container,
      {
        turnTo: (dir) => this.turnTo(dir),
        orbit: (dx, dy) => {
          if (this.view.kind !== "3d") return;
          window.cancelAnimationFrame(this.turnFrame);
          this.orbitBy(this.controls.target.clone(), -dx * 0.02, dy * 0.02);
        },
        home: () => {
          window.cancelAnimationFrame(this.turnFrame);
          this.zoomToFit();
        },
      },
      labels,
      t("cube.home"),
      dark,
    );
  }

  /**
   * Turns the 3D view so the camera looks from `dir` (model to camera) at the same point and distance, in a
   * short animation by compass angle and height angle (the shorter way round), Z up all the way, so nothing
   * flips. Straight from above or below the camera stops just short of vertical, seen from the front (-Y) side,
   * so the plan reads with Y up.
   */
  private turnTo(dir: THREE.Vector3) {
    if (this.view.kind !== "3d") return;
    window.cancelAnimationFrame(this.turnFrame);
    const cam = this.camera;
    const target = this.controls.target.clone();
    const dist = cam.position.distanceTo(target);
    const angles = (d: THREE.Vector3) => ({ az: Math.atan2(d.y, d.x), el: Math.asin(THREE.MathUtils.clamp(d.z, -1, 1)) });
    const from = angles(cam.position.clone().sub(target).normalize());
    const to = dir.clone().normalize();
    const vertical = Math.abs(to.z) > 0.999;
    const end = vertical ? { az: -Math.PI / 2, el: Math.sign(to.z) * TURN_MAX_ELEVATION } : angles(to);
    let dAz = end.az - from.az;
    dAz = Math.atan2(Math.sin(dAz), Math.cos(dAz));
    const start = performance.now();
    const step = () => {
      const k = Math.min(1, (performance.now() - start) / TURN_MS);
      const e = k * k * (3 - 2 * k);
      const az = from.az + dAz * e;
      const el = from.el + (end.el - from.el) * e;
      const d = new THREE.Vector3(Math.cos(el) * Math.cos(az), Math.cos(el) * Math.sin(az), Math.sin(el));
      cam.position.copy(target).addScaledVector(d, dist);
      cam.up.copy(Z_UP);
      cam.lookAt(target);
      this.controls.update();
      this.cameraMoved();
      if (k < 1) this.turnFrame = window.requestAnimationFrame(step);
    };
    step();
  }

  // ---- Viewport API ----

  setScene(base: SceneData, overlay: (() => SceneData) | null): void {
    const newBase = base !== this.baseScene || !this.baseLayer;
    const newOverlay = overlay !== this.overlaySource;
    if (!newBase && !newOverlay) return;
    // Dashes in proportion to the model (as they were per line material).
    const size = Math.hypot(...this.bounds.max.map((v, i) => v - this.bounds.min[i]));
    this.shared.dashSize.value = Math.max(size, 1) / 90;
    this.shared.gapSize.value = Math.max(size, 1) / 140;
    // Opaque parts in the order they had as separate objects: the rubber band (0), the model, the results
    // over it, then the hover highlight (1).
    if (newBase) {
      this.baseLayer?.dispose();
      this.baseLayer = new SceneLayer(base, this.pool, 0.1);
      this.baseItems = pickItems(base);
      this.baseScene = base;
    }
    if (newOverlay) {
      this.overlayLayer?.dispose();
      const d = overlay?.() ?? null;
      this.overlayLayer = d ? new SceneLayer(d, this.pool, 0.5, false) : null;
      this.overlayItems = d ? pickItems(d) : null;
      this.overlaySource = overlay;
    } else {
      // The results' objects made again after the model's, so ties in the renderer's order go as before.
      this.overlayLayer?.renew();
    }
    const baseLayer = this.baseLayer!;
    this.scene.add(baseLayer.group);
    if (this.overlayLayer) this.scene.add(this.overlayLayer.group);
    this.scene.add(this.hoverGroup, this.helpers);
    if (newBase) baseLayer.setStyles(this.styles);
    this.items = this.overlayItems ? joinPickItems(this.overlayItems, this.baseItems) : this.baseItems;
    this.picker = null;
    this.textsFor = [];
    this.updateHoverHighlight();
    this.requestRender();
  }

  setStyles(styles: Map<string, PickStyle>): void {
    this.styles = styles;
    this.baseLayer?.setStyles(styles);
    this.requestRender();
  }

  setSwing(swing: number): void {
    if (this.shared.swing.value === swing) return;
    this.shared.swing.value = swing;
    this.requestRender();
  }

  setParallel(on: boolean): void {
    if (on === this.parallel) return;
    this.parallel = on;
    if (this.view.kind !== "3d") return;
    // Same direction, target and framing in the other projection: the parallel view shows what the perspective
    // view showed at its target's distance, and back.
    const target = this.controls.target.clone();
    const tanHalf = Math.tan(THREE.MathUtils.degToRad(this.persp.fov / 2));
    const aspect = this.width / this.height;
    if (on) {
      const forward = target.clone().sub(this.persp.position);
      const dist = forward.length();
      forward.normalize();
      const half = dist * tanHalf;
      const radius = this.boundsRadius();
      this.ortho.up.copy(this.persp.up);
      this.ortho.position.copy(target).addScaledVector(forward, -radius * 20);
      this.ortho.lookAt(target);
      Object.assign(this.ortho, { top: half, bottom: -half, left: -half * aspect, right: half * aspect, zoom: 1, near: 0.001, far: radius * 60 });
      this.ortho.updateProjectionMatrix();
      this.camera = this.ortho;
    } else {
      const forward = target.clone().sub(this.ortho.position).normalize();
      const half = (this.ortho.top - this.ortho.bottom) / 2 / this.ortho.zoom;
      const dist = half / tanHalf;
      this.persp.up.copy(this.ortho.up);
      this.persp.position.copy(target).addScaledVector(forward, -dist);
      this.persp.near = dist / 1000;
      this.persp.far = dist * 100;
      this.persp.lookAt(target);
      this.persp.updateProjectionMatrix();
      this.camera = this.persp;
    }
    this.controls.dispose();
    this.controls = this.makeControls();
    this.controls.target.copy(target);
    this.controls.update();
    this.cameraMoved();
  }

  /** Half the diagonal of the model's box (at least 0.5). */
  private boundsRadius(): number {
    const { min, max } = this.bounds;
    return Math.max(0.5, new THREE.Vector3(...max).distanceTo(new THREE.Vector3(...min)) / 2);
  }

  setColors(background: string, hover: string): void {
    this.renderer.setClearColor(background);
    const bg = new THREE.Color(background);
    const dark = bg.r * 0.3 + bg.g * 0.59 + bg.b * 0.11 < 0.5;
    if (dark !== this.navCubeDark) this.makeNavCube(dark);
    this.hoverColor = hover;
    (this.rubber.material as LineMaterial).color.set(hover);
    (this.snapMarker.material as THREE.PointsMaterial).color.set(hover);
    (this.pivotMarker.material as THREE.PointsMaterial).color.set(hover);
    this.updateHoverHighlight();
    this.requestRender();
  }

  setView(view: ViewSpec, bounds: Bounds, fit: boolean): void {
    const kindChanged = view.kind !== this.view.kind || view.at !== this.view.at;
    this.view = view;
    this.bounds = bounds;
    const camera = view.kind === "3d" && !this.parallel ? this.persp : this.ortho;
    if (camera !== this.camera) {
      this.camera = camera;
      this.controls.dispose();
      this.controls = this.makeControls();
    } else {
      this.configureControls();
    }
    if (fit || kindChanged) this.zoomToFit();
    this.requestRender();
  }

  setSnapPoints(points: Vec3[]): void {
    this.snapPts = points;
    this.snapScreen = null;
  }

  setInteraction(i: Interaction): void {
    this.interaction = i;
    if (!i.rubber.length) this.rubber.visible = false;
    if (!i.snap) this.snapMarker.visible = false;
    if (i.pick === "none" && this.hover) this.setHover(null);
    this.requestRender();
  }

  zoomToFit(): void {
    const { min, max } = this.bounds;
    const center = new THREE.Vector3((min[0] + max[0]) / 2, (min[1] + max[1]) / 2, (min[2] + max[2]) / 2);
    const radius = Math.max(0.5, new THREE.Vector3(...max).distanceTo(new THREE.Vector3(...min)) / 2);
    const { forward, up } = viewAxes(this.view.kind);
    if (this.view.kind === "3d" && !this.parallel) {
      // The narrower of the two view angles: a tall, narrow view (a wide side panel) fits by its width.
      const vHalf = THREE.MathUtils.degToRad(this.persp.fov / 2);
      const hHalf = Math.atan(Math.tan(vHalf) * (this.width / this.height || 1));
      const dist = (radius / Math.sin(Math.min(vHalf, hHalf))) * 0.8;
      this.persp.up.copy(up);
      this.persp.position.copy(center).addScaledVector(forward, -dist);
      this.persp.near = dist / 1000;
      this.persp.far = dist * 100;
      this.persp.lookAt(center);
      this.persp.updateProjectionMatrix();
    } else {
      // Keep the camera target on the view plane (a parallel 3D view has none).
      if (this.view.kind !== "3d") {
        const k = this.view.kind === "plan" ? 2 : this.view.kind === "elev-xz" ? 1 : 0;
        center.setComponent(k, this.view.at);
      }
      const right = new THREE.Vector3().crossVectors(forward, up);
      let w = 0;
      let h = 0;
      for (const x of [min[0], max[0]]) {
        for (const y of [min[1], max[1]]) {
          for (const z of [min[2], max[2]]) {
            const d = new THREE.Vector3(x, y, z).sub(center);
            w = Math.max(w, Math.abs(d.dot(right)));
            h = Math.max(h, Math.abs(d.dot(up)));
          }
        }
      }
      const aspect = this.width / this.height;
      const half = Math.max(h, w / aspect, 0.5) * 1.05;
      this.ortho.up.copy(up);
      this.ortho.position.copy(center).addScaledVector(forward, -radius * 20);
      this.ortho.lookAt(center);
      this.ortho.top = half;
      this.ortho.bottom = -half;
      this.ortho.left = -half * aspect;
      this.ortho.right = half * aspect;
      this.ortho.zoom = 1;
      this.ortho.near = 0.001;
      this.ortho.far = radius * 60;
      this.ortho.updateProjectionMatrix();
    }
    this.controls.target.copy(center);
    this.controls.update();
    this.requestRender();
  }

  project(p: Vec3): { x: number; y: number } | null {
    const v = new THREE.Vector3(...p).project(this.camera);
    if (v.z < -1 || v.z > 1) return null;
    return { x: ((v.x + 1) / 2) * this.width, y: ((1 - v.y) / 2) * this.height };
  }

  /** The camera as the pick search needs it, and a signature to see when it changed. */
  private cameraNow() {
    const view = this.camera.matrixWorldInverse.elements;
    const proj = this.camera.projectionMatrix.elements;
    return { cam: { view, proj, width: this.width, height: this.height }, sig: [...view, ...proj, this.width, this.height] };
  }

  /** The pick search for the current camera. */
  private pickerNow(): ScreenPicker {
    const { cam, sig } = this.cameraNow();
    if (!this.picker || sig.some((v, i) => v !== this.pickerFor[i])) {
      this.picker = new ScreenPicker(this.items, { ...cam, view: [...cam.view], proj: [...cam.proj] });
      this.pickerFor = sig;
    }
    return this.picker;
  }

  on<K extends keyof ViewportEvents>(event: K, handler: ViewportEvents[K]): () => void {
    (this.listeners[event] as Set<ViewportEvents[K]>).add(handler);
    return () => (this.listeners[event] as Set<ViewportEvents[K]>).delete(handler);
  }

  dispose(): void {
    window.cancelAnimationFrame(this.turnFrame);
    this.navCube?.dispose();
    this.cleanup.forEach((f) => f());
    this.resizeObserver.disconnect();
    window.clearTimeout(this.restTimer);
    this.controls.dispose();
    this.baseLayer?.dispose();
    this.overlayLayer?.dispose();
    for (const child of this.hoverGroup.children) (child as THREE.Mesh).geometry?.dispose();
    this.pool.dispose();
    this.renderer.dispose();
    this.renderer.domElement.remove();
    this.labels.domElement.remove();
    this.boxDiv.remove();
  }

  // ---- drawing ----

  private makeControls(): OrbitControls {
    const c = new OrbitControls(this.camera, this.renderer.domElement);
    c.addEventListener("change", () => this.cameraMoved());
    this.controls = c;
    this.configureControls();
    return c;
  }

  private configureControls() {
    const c = this.controls;
    const is3d = this.view.kind === "3d";
    // Left button selects and draws, middle pans, wheel zooms; right pans in plan and elevation views and
    // rotates the 3D view (our own orbit about the point under the mouse, see `orbitBy`).
    c.mouseButtons = { LEFT: null as unknown as THREE.MOUSE, MIDDLE: THREE.MOUSE.PAN, RIGHT: is3d ? (null as unknown as THREE.MOUSE) : THREE.MOUSE.PAN };
    c.enableRotate = false;
    c.screenSpacePanning = true;
    c.zoomToCursor = true;
    c.enableDamping = false;
  }

  private resize() {
    const r = this.container.getBoundingClientRect();
    this.width = Math.max(1, r.width);
    this.height = Math.max(1, r.height);
    this.renderer.setSize(this.width, this.height);
    this.labels.setSize(this.width, this.height);
    this.textsFor = [];
    this.persp.aspect = this.width / this.height;
    this.persp.updateProjectionMatrix();
    // Keep what was visible: when the view gets narrower (e.g. a side panel opens), zoom out.
    const oldHalfH = (this.ortho.top - this.ortho.bottom) / 2;
    const oldHalfW = (this.ortho.right - this.ortho.left) / 2;
    const half = Math.max(oldHalfH, oldHalfW / this.persp.aspect);
    this.ortho.top = half;
    this.ortho.bottom = -half;
    this.ortho.left = -half * this.persp.aspect;
    this.ortho.right = half * this.persp.aspect;
    this.ortho.updateProjectionMatrix();
    for (const m of this.lineMaterials) m.resolution.set(this.width, this.height);
    this.shared.view.value.set(this.width, this.height);
    this.requestRender();
  }

  /** The view turned, panned or zoomed: drawn again, and large see-through batches sorted once it rests. */
  private cameraMoved() {
    this.lastMove = performance.now();
    window.clearTimeout(this.restTimer);
    this.restTimer = window.setTimeout(() => this.requestRender(), REST_MS);
    this.requestRender();
  }

  private requestRender() {
    if (this.renderPending) return;
    this.renderPending = true;
    requestAnimationFrame(() => {
      this.renderPending = false;
      this.render();
    });
  }

  private render() {
    this.headlight.position.copy(this.camera.position);
    this.headlight.target.position.copy(this.controls.target);
    // See-through faces back to front for this camera (the matrices as the renderer will have them).
    this.camera.updateMatrixWorld();
    const projView = new THREE.Matrix4().multiplyMatrices(this.camera.projectionMatrix, this.camera.matrixWorldInverse);
    const atRest = performance.now() - this.lastMove >= REST_MS;
    for (const layer of [this.baseLayer, this.overlayLayer]) layer?.sortFaces(projView, atRest);
    this.renderer.render(this.scene, this.camera);
    // The navigation cube follows the 3D view's camera (plan and elevation views have none).
    if (this.navCube) {
      const show = this.view.kind === "3d";
      this.navCube.setVisible(show);
      if (show) {
        // Before the first fit the camera may sit on its target: no direction yet, so the cube keeps its own.
        const forward = this.controls.target.clone().sub(this.camera.position);
        if (forward.lengthSq() > 1e-18) this.navCube.update(forward.normalize(), this.camera.up);
      }
    }
    // Texts are placed again only when the camera or the scene changed (not for a hover or a selection);
    // many of them wait until the view rests.
    const texts = (this.baseLayer?.labelCount ?? 0) + (this.overlayLayer?.labelCount ?? 0);
    const hide = !atRest && texts > TEXTS_WHILE_MOVING;
    this.labels.domElement.style.visibility = hide ? "hidden" : "";
    const { sig } = this.cameraNow();
    if (!hide && sig.some((v, i) => v !== this.textsFor[i])) {
      this.labels.render(this.scene, this.camera);
      this.textsFor = sig;
    }
  }

  private makeLines(positions: number[], color: string, width: number, dashed = false, opacity = 1): LineSegments2 {
    const g = new LineSegmentsGeometry();
    if (positions.length) g.setPositions(positions);
    const size = Math.hypot(...this.bounds.max.map((v, i) => v - this.bounds.min[i]));
    const m = new LineMaterial({
      color,
      linewidth: width,
      dashed,
      dashSize: Math.max(size, 1) / 90,
      gapSize: Math.max(size, 1) / 140,
      transparent: opacity < 1,
      opacity,
    });
    m.resolution.set(this.width, this.height);
    this.lineMaterials.add(m);
    const obj = new LineSegments2(g, m);
    if (dashed && positions.length) obj.computeLineDistances();
    return obj;
  }

  // ---- picking and mouse ----

  private emit<K extends keyof ViewportEvents>(event: K, ...args: Parameters<ViewportEvents[K]>) {
    for (const f of this.listeners[event] as Set<(...a: Parameters<ViewportEvents[K]>) => void>) f(...args);
  }

  private local(e: PointerEvent) {
    const r = this.renderer.domElement.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  private pickAt(x: number, y: number): PickRef | null {
    if (this.interaction.pick === "none") return null;
    return this.pickerNow().pickAt(x, y, this.interaction.pick === "nodes");
  }

  private snapAt(x: number, y: number): Vec3 | null {
    // Snap points projected once per camera position.
    const { cam, sig } = this.cameraNow();
    if (!this.snapScreen || sig.some((v, i) => v !== this.snapScreen!.for[i])) {
      const pts = Float64Array.from(this.snapPts.flat());
      this.snapScreen = { pts, screen: projectAll(pts, cam), for: sig };
    }
    const sc = this.snapScreen.screen;
    let best: Vec3 | null = null;
    let bestD = SNAP_PX;
    for (let i = 0; i < this.snapPts.length; i++) {
      if (Number.isNaN(sc[3 * i])) continue;
      const d = Math.hypot(sc[3 * i] - x, sc[3 * i + 1] - y);
      if (d < bestD) [best, bestD] = [this.snapPts[i], d];
    }
    return best;
  }

  /** Where a new point would go: the snap point, else the plane point rounded to the step. */
  private drawPoint(x: number, y: number): { point: Vec3 | null; snapped: boolean } {
    const snap = this.interaction.snap ? this.snapAt(x, y) : null;
    if (snap) return { point: snap, snapped: true };
    const p = this.planePoint(x, y);
    if (!p || !this.interaction.snap) return { point: p, snapped: false };
    // Round the in-plane coordinates only; the plane coordinate stays exactly on the view plane.
    const k = this.view.kind === "plan" ? 2 : this.view.kind === "elev-xz" ? 1 : 0;
    const step = this.interaction.step;
    const r = p.map((v, i) => (i === k ? this.view.at : Math.round(Math.round(v / step) * step * 1e9) / 1e9)) as Vec3;
    return { point: r, snapped: false };
  }

  /** The point on the plan/elevation plane under the mouse (null in 3D). */
  private planePoint(x: number, y: number): Vec3 | null {
    if (this.view.kind === "3d") return null;
    const ndc = new THREE.Vector2((x / this.width) * 2 - 1, 1 - (y / this.height) * 2);
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, this.camera);
    const normal = viewAxes(this.view.kind).forward.clone().negate();
    const k = this.view.kind === "plan" ? 2 : this.view.kind === "elev-xz" ? 1 : 0;
    const point = new THREE.Vector3();
    point.setComponent(k, this.view.at);
    const plane = new THREE.Plane().setFromNormalAndCoplanarPoint(normal, point);
    const hit = ray.ray.intersectPlane(plane, new THREE.Vector3());
    return hit ? [hit.x, hit.y, hit.z] : null;
  }

  private setHover(p: PickRef | null) {
    if (p?.kind === this.hover?.kind && p?.id === this.hover?.id) return;
    this.hover = p;
    this.updateHoverHighlight();
    this.emit("hover", p);
    this.requestRender();
  }

  private updateHoverHighlight() {
    // Only geometry is made per hover; the materials are the viewport's own (no shader program made again).
    for (const child of [...this.hoverGroup.children]) {
      this.hoverGroup.remove(child);
      (child as THREE.Mesh).geometry?.dispose();
    }
    const h = this.hover;
    if (!h) return;
    const key = pickKey(h);
    const layers = [this.overlayLayer, this.baseLayer].filter((l): l is SceneLayer => !!l);
    const c = rgb(this.hoverColor);
    if (h.kind === "node") {
      const dot = layers.map((l) => l.objectDot(key)).find((x) => x);
      if (!dot) return;
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.Float32BufferAttribute(dot.position, 3));
      g.setAttribute("color", new THREE.Float32BufferAttribute(c, 3));
      g.setAttribute("psize", new THREE.Float32BufferAttribute([13], 1));
      g.setAttribute("motion", new THREE.Float32BufferAttribute(dot.motion, 3));
      const pts = new THREE.Points(g, this.pool.dots(true, true));
      pts.renderOrder = 6;
      pts.frustumCulled = false;
      this.hoverGroup.add(pts);
      return;
    }
    // Lines of the object thick in the hover colour (moving with a mode animation).
    const lines = layers.map((l) => l.objectLines(key));
    const n = lines.reduce((s, l) => s + l.position.length / 6, 0);
    if (n) {
      const position = new Float32Array(6 * n);
      const color = new Float32Array(6 * n);
      const moved = new Float32Array(6 * n);
      let o = 0;
      for (const l of lines) {
        position.set(l.position, o);
        if (l.motion) moved.set(l.motion, o);
        o += l.position.length;
      }
      for (let i = 0; i < 2 * n; i++) color.set(c, 3 * i);
      const g = new LineSegmentsGeometry();
      g.setPositions(position);
      g.setColors(color);
      const zeros = new THREE.InstancedInterleavedBuffer(new Float32Array(2 * n), 2, 1);
      g.setAttribute("instanceDistanceStart", new THREE.InterleavedBufferAttribute(zeros, 1, 0));
      g.setAttribute("instanceDistanceEnd", new THREE.InterleavedBufferAttribute(zeros, 1, 1));
      g.setAttribute("instanceDash", new THREE.InstancedBufferAttribute(new Float32Array(n), 1));
      const mb = new THREE.InstancedInterleavedBuffer(moved, 6, 1);
      g.setAttribute("instanceMotionStart", new THREE.InterleavedBufferAttribute(mb, 3, 0));
      g.setAttribute("instanceMotionEnd", new THREE.InterleavedBufferAttribute(mb, 3, 3));
      const obj = new LineSegments2(g, this.pool.line(h.kind === "area" ? 3 : 5, 1, true));
      obj.frustumCulled = false;
      obj.renderOrder = 1;
      this.hoverGroup.add(obj);
    }
    // Its faces, see-through, over everything.
    const tris = layers.map((l) => l.objectTriangles(key));
    const v = tris.reduce((s, t) => s + t.position.length, 0);
    if (v) {
      const position = new Float32Array(v);
      const moved = new Float32Array(v);
      const color = new Float32Array((4 * v) / 3);
      let o = 0;
      for (const t of tris) {
        position.set(t.position, o);
        if (t.motion) moved.set(t.motion, o);
        o += t.position.length;
      }
      for (let i = 0; i < v / 3; i++) color.set([c[0], c[1], c[2], 1], 4 * i);
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(position, 3));
      g.setAttribute("color", new THREE.BufferAttribute(color, 4));
      g.setAttribute("motion", new THREE.BufferAttribute(moved, 3));
      const face = new THREE.Mesh(g, this.pool.hoverFaces());
      face.renderOrder = 3;
      face.frustumCulled = false;
      this.hoverGroup.add(face);
    }
  }

  /**
   * The point a 3D orbit turns about: the joint, member or face under the mouse (where the drag starts), or
   * the middle of the model when there is nothing there.
   */
  private pivotAt(x: number, y: number): THREE.Vector3 {
    const best = this.pickerNow().pointAt(x, y);
    if (best) return new THREE.Vector3(...best);
    const { min, max } = this.bounds;
    return new THREE.Vector3((min[0] + max[0]) / 2, (min[1] + max[1]) / 2, (min[2] + max[2]) / 2);
  }

  /**
   * Turns the 3D view about `pivot` (turntable, Z stays up): `yaw` about the vertical, `pitch` about the
   * view's horizontal axis (positive lifts the camera); the view stops short of straight down or up.
   */
  private orbitBy(pivot: THREE.Vector3, yaw: number, pitch: number) {
    const cam = this.camera;
    const target = this.controls.target;
    const forward = target.clone().sub(cam.position).normalize();
    const right = forward.clone().cross(Z_UP);
    if (right.lengthSq() < 1e-12) right.set(1, 0, 0);
    right.normalize();
    // Looking down means a negative elevation; lifting the camera lowers it.
    const elevation = Math.asin(THREE.MathUtils.clamp(forward.z, -1, 1));
    // A view nearer vertical (a Top or Bottom from the navigation cube) may stay there; it never goes past.
    const limit = Math.max(ORBIT_MAX_ELEVATION, Math.abs(elevation));
    const next = THREE.MathUtils.clamp(elevation - pitch, -limit, limit);
    const turn = new THREE.Quaternion()
      .setFromAxisAngle(Z_UP, yaw)
      .multiply(new THREE.Quaternion().setFromAxisAngle(right, next - elevation));
    cam.position.sub(pivot).applyQuaternion(turn).add(pivot);
    target.sub(pivot).applyQuaternion(turn).add(pivot);
    cam.up.copy(Z_UP);
    cam.lookAt(target);
    this.controls.update();
    this.cameraMoved();
  }

  private onMove(e: PointerEvent) {
    const { x, y } = this.local(e);
    if (this.orbit) {
      // Right-drag in 3D: drag right turns the model right, drag down shows it more from above.
      const dx = x - this.orbit.x;
      const dy = y - this.orbit.y;
      this.orbit.x = x;
      this.orbit.y = y;
      this.orbitBy(this.orbit.pivot, (-dx / this.width) * ORBIT_TURN, (dy / this.height) * ORBIT_TURN);
      return;
    }
    if (this.down && this.down.button === 0 && this.interaction.box) {
      const dx = x - this.down.x;
      const dy = y - this.down.y;
      if (Math.hypot(dx, dy) > CLICK_PX) {
        Object.assign(this.boxDiv.style, {
          display: "block",
          left: `${Math.min(x, this.down.x)}px`,
          top: `${Math.min(y, this.down.y)}px`,
          width: `${Math.abs(dx)}px`,
          height: `${Math.abs(dy)}px`,
        });
        this.boxDiv.classList.toggle("crossing", dx < 0);
        return;
      }
    }
    if (this.down && this.down.button !== 0) return; // panning or rotating
    this.setHover(this.pickAt(x, y));
    const { point, snapped } = this.drawPoint(x, y);
    this.snap = snapped ? point : null;
    if (point && this.interaction.snap) {
      // Bright on a grid point or joint, faint on a free point.
      (this.snapMarker.geometry.getAttribute("position") as THREE.BufferAttribute).setXYZ(0, ...point);
      this.snapMarker.geometry.getAttribute("position").needsUpdate = true;
      (this.snapMarker.material as THREE.PointsMaterial).opacity = snapped ? 0.75 : 0.35;
      this.snapMarker.visible = true;
    } else {
      this.snapMarker.visible = false;
    }
    const cursor = point;
    const path = this.interaction.rubber;
    if (path.length && cursor) {
      const pts = [...path, cursor, ...(this.interaction.rubberClosed && path.length > 1 ? [path[0]] : [])];
      const segs: number[] = [];
      for (let i = 0; i + 1 < pts.length; i++) segs.push(...pts[i], ...pts[i + 1]);
      (this.rubber.geometry as LineSegmentsGeometry).setPositions(segs);
      this.rubber.computeLineDistances();
      this.rubber.visible = true;
    } else {
      this.rubber.visible = false;
    }
    this.emit("cursor", cursor, !!this.snap);
    this.requestRender();
  }

  private onDown(e: PointerEvent) {
    const { x, y } = this.local(e);
    this.down = { x, y, button: e.button };
    if (e.button === 2 && this.view.kind === "3d") {
      const pivot = this.pivotAt(x, y);
      this.orbit = { x, y, pivot };
      (this.pivotMarker.geometry.getAttribute("position") as THREE.BufferAttribute).setXYZ(0, pivot.x, pivot.y, pivot.z);
      this.pivotMarker.geometry.getAttribute("position").needsUpdate = true;
      this.pivotMarker.visible = true;
      this.renderer.domElement.setPointerCapture(e.pointerId);
      this.requestRender();
    }
  }

  private onUp(e: PointerEvent) {
    const start = this.down;
    this.down = null;
    if (this.orbit) {
      this.orbit = null;
      this.pivotMarker.visible = false;
      this.requestRender();
    }
    if (!start) return;
    const { x, y } = this.local(e);
    const moved = Math.hypot(x - start.x, y - start.y) > CLICK_PX;
    if (start.button === 2 && !moved) {
      this.emit("finish");
      return;
    }
    if (start.button !== 0) return;
    if (!moved) {
      const { point, snapped } = this.drawPoint(x, y);
      this.emit("click", {
        pick: this.pickAt(x, y),
        snap: snapped ? point : null,
        point: this.interaction.snap ? point : null,
        planePoint: this.planePoint(x, y),
        shift: e.shiftKey,
        ctrl: e.ctrlKey || e.metaKey,
      });
      return;
    }
    if (this.boxDiv.style.display === "block") {
      this.boxDiv.style.display = "none";
      const crossing = x < start.x;
      const r = { x0: Math.min(x, start.x), x1: Math.max(x, start.x), y0: Math.min(y, start.y), y1: Math.max(y, start.y) };
      this.emit("box", { picks: this.boxPicks(r, crossing), crossing, shift: e.shiftKey, ctrl: e.ctrlKey || e.metaKey });
    }
  }

  private onLeave() {
    if (!this.down) {
      this.setHover(null);
      this.snapMarker.visible = false;
      this.rubber.visible = false;
      this.emit("cursor", null, false);
      this.requestRender();
    }
  }

  private boxPicks(r: { x0: number; x1: number; y0: number; y1: number }, crossing: boolean): PickRef[] {
    const pk = this.pickerNow();
    const { refs, pointRef, lineRef, triRef } = this.items;
    const s = pk.s;
    const inside = (o: number) => !Number.isNaN(s[o]) && s[o] >= r.x0 && s[o] <= r.x1 && s[o + 1] >= r.y0 && s[o + 1] <= r.y1;
    const at = (o: number) => (Number.isNaN(s[o]) ? null : { x: s[o], y: s[o + 1] });
    const out = new Map<string, PickRef>();
    for (let i = 0; i < pointRef.length; i++) if (inside(pk.pointAt3(i))) out.set(pickKey(refs[pointRef[i]]), refs[pointRef[i]]);
    if (this.interaction.pick === "all") {
      for (let i = 0; i < lineRef.length; i++) {
        const [oa, ob] = pk.line3(i);
        const [a, b] = [at(oa), at(ob)];
        const [ia, ib] = [inside(oa), inside(ob)];
        const hit = crossing ? ia || ib || (!!a && !!b && segmentHitsRect(a, b, r)) : ia && ib;
        if (hit) out.set(pickKey(refs[lineRef[i]]), refs[lineRef[i]]);
      }
      // Faces: a window takes those wholly inside, a crossing box those it touches.
      const faces = new Map<string, { pick: PickRef; all: boolean; any: boolean }>();
      for (let i = 0; i < triRef.length; i++) {
        const pick = refs[triRef[i]];
        const key = pickKey(pick);
        const f = faces.get(key) ?? { pick, all: true, any: false };
        faces.set(key, f);
        const corners = pk.triangle3(i);
        const ins = corners.map(inside);
        if (!ins.every((x) => x)) f.all = false;
        if (crossing && !f.any) {
          const [a, b, c] = corners.map(at);
          f.any =
            ins.some((x) => x) ||
            (!!a && !!b && !!c && ([[a, b], [b, c], [c, a]] as const).some(([p, q]) => segmentHitsRect(p, q, r))) ||
            (!!a && !!b && !!c && barycentric(r.x0, r.y0, a.x, a.y, b.x, b.y, c.x, c.y) !== null);
        }
      }
      for (const [key, f] of faces) if (crossing ? f.any || f.all : f.all) out.set(key, f.pick);
    }
    return [...out.values()];
  }
}

function segmentHitsRect(a: { x: number; y: number }, b: { x: number; y: number }, r: { x0: number; x1: number; y0: number; y1: number }): boolean {
  const edges: [number, number, number, number][] = [
    [r.x0, r.y0, r.x1, r.y0],
    [r.x1, r.y0, r.x1, r.y1],
    [r.x1, r.y1, r.x0, r.y1],
    [r.x0, r.y1, r.x0, r.y0],
  ];
  const cross = (ax: number, ay: number, bx: number, by: number, cx: number, cy: number) => (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
  return edges.some(([x0, y0, x1, y1]) => {
    const d1 = cross(x0, y0, x1, y1, a.x, a.y);
    const d2 = cross(x0, y0, x1, y1, b.x, b.y);
    const d3 = cross(a.x, a.y, b.x, b.y, x0, y0);
    const d4 = cross(a.x, a.y, b.x, b.y, x1, y1);
    return d1 * d2 < 0 && d3 * d4 < 0;
  });
}
