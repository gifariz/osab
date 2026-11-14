// The navigation cube of the 3D view: a small cube in the view's bottom-right corner (the contour legend has the top) that turns with the camera.
// Clicking one of its 26 parts (6 faces, 12 edges, 8 corners) turns the view to look from that side; dragging it
// orbits the view; the house button goes back to the default 3D view.

import * as THREE from "three";

/** Directions (from the model to the camera) of the 26 parts: faces, edges and corners of a 3 x 3 x 3 cube. */
const PARTS: THREE.Vector3[] = [];
for (const x of [-1, 0, 1]) for (const y of [-1, 0, 1]) for (const z of [-1, 0, 1]) if (x || y || z) PARTS.push(new THREE.Vector3(x, y, z));

/** The face labels: Front looks along +Y (the X-Z elevation), Right along -X (the Y-Z elevation), Top down. */
export type FaceName = "top" | "bottom" | "front" | "back" | "left" | "right";
const FACES: [FaceName, THREE.Vector3][] = [
  ["right", new THREE.Vector3(1, 0, 0)],
  ["left", new THREE.Vector3(-1, 0, 0)],
  ["back", new THREE.Vector3(0, 1, 0)],
  ["front", new THREE.Vector3(0, -1, 0)],
  ["top", new THREE.Vector3(0, 0, 1)],
  ["bottom", new THREE.Vector3(0, 0, -1)],
];

const SIZE = 76;
const DRAG_PX = 4;

function faceTexture(text: string, dark: boolean): THREE.CanvasTexture {
  const c = document.createElement("canvas");
  c.width = c.height = 256;
  const g = c.getContext("2d")!;
  g.fillStyle = dark ? "#30363d" : "#eef1f5";
  g.fillRect(0, 0, 256, 256);
  g.fillStyle = dark ? "#e6edf3" : "#1f2328";
  // As large as fits: the label may reach close to the face's edges.
  let px = 92;
  g.font = `bold ${px}px sans-serif`;
  while (px > 36 && g.measureText(text).width > 240) g.font = `bold ${(px -= 4)}px sans-serif`;
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.fillText(text, 128, 134);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

export interface NavCubeHandlers {
  /** Turn the view to look from `dir` (model to camera, unit length). */
  turnTo(dir: THREE.Vector3): void;
  /** Orbit the view by a drag of (dx, dy) pixels. */
  orbit(dx: number, dy: number): void;
  /** Back to the default 3D view. */
  home(): void;
}

export class NavCube {
  readonly element: HTMLDivElement;
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera = new THREE.OrthographicCamera(-1.8, 1.8, 1.8, -1.8, 0.1, 20);
  private parts: THREE.Mesh[] = [];
  private faceMaterials: THREE.MeshBasicMaterial[] = [];
  private hovered: THREE.Mesh | null = null;
  private drag: { x: number; y: number; moved: boolean } | null = null;
  private ray = new THREE.Raycaster();
  private cleanup: (() => void)[] = [];

  constructor(
    container: HTMLElement,
    private handlers: NavCubeHandlers,
    labels: Record<FaceName, string>,
    homeTitle: string,
    dark: boolean,
  ) {
    this.element = document.createElement("div");
    this.element.className = "nav-cube";
    this.element.dataset.testid = "nav-cube";
    Object.assign(this.element.style, { position: "absolute", right: "8px", bottom: "8px", width: `${SIZE}px`, height: `${SIZE}px`, zIndex: "2" });
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    // Drawn at least at double resolution and scaled down by the browser: sharper labels on a small cube.
    this.renderer.setPixelRatio(Math.max(2, window.devicePixelRatio));
    this.renderer.setSize(SIZE, SIZE);
    this.renderer.domElement.style.display = "block";
    this.renderer.domElement.style.cursor = "pointer";
    this.element.appendChild(this.renderer.domElement);
    const home = document.createElement("button");
    home.className = "nav-cube-home";
    home.title = homeTitle;
    home.setAttribute("aria-label", homeTitle);
    home.textContent = "⌂";
    Object.assign(home.style, { position: "absolute", left: "-4px", top: "-4px", padding: "0 3px", lineHeight: "15px", fontSize: "12px" });
    home.addEventListener("click", () => this.handlers.home());
    this.element.appendChild(home);
    container.appendChild(this.element);

    // The cube: one box with a label on each face, its outline, and 26 invisible parts (faces, edges, corners)
    // over it for picking; the part under the mouse lights up.
    const order = [new THREE.Vector3(1, 0, 0), new THREE.Vector3(-1, 0, 0), new THREE.Vector3(0, 1, 0), new THREE.Vector3(0, -1, 0), new THREE.Vector3(0, 0, 1), new THREE.Vector3(0, 0, -1)];
    const box = new THREE.Mesh(
      new THREE.BoxGeometry(2, 2, 2),
      // BoxGeometry's material order: +X, -X, +Y, -Y, +Z, -Z.
      order.map((n) => {
        const [name] = FACES.find(([, f]) => f.equals(n))!;
        const map = faceTexture(labels[name], dark);
        // The sharpest filtering the graphics card has for a face seen at a slant.
        map.anisotropy = this.renderer.capabilities.getMaxAnisotropy();
        const face = new THREE.MeshBasicMaterial({ map });
        this.faceMaterials.push(face);
        return face;
      }),
    );
    this.scene.add(box);
    this.fixLabels(order);
    const edges = new THREE.LineSegments(
      new THREE.EdgesGeometry(new THREE.BoxGeometry(2, 2, 2)),
      new THREE.LineBasicMaterial({ color: dark ? "#8b949e" : "#8c959f" }),
    );
    this.scene.add(edges);
    const s = (k: number) => (k === 0 ? 1.3 : 0.35);
    for (const d of PARTS) {
      const mesh = new THREE.Mesh(
        new THREE.BoxGeometry(s(d.x), s(d.y), s(d.z)),
        new THREE.MeshBasicMaterial({ color: "#58a6ff", transparent: true, opacity: 0, depthWrite: false }),
      );
      // Just outside the box, so a lit part is not hidden by the face under it.
      mesh.position.set(d.x * 0.835, d.y * 0.835, d.z * 0.835);
      mesh.userData.dir = d.clone().normalize();
      this.parts.push(mesh);
      this.scene.add(mesh);
    }

    const canvas = this.renderer.domElement;
    const on = <K extends keyof HTMLElementEventMap>(t: K, f: (e: HTMLElementEventMap[K]) => void) => {
      canvas.addEventListener(t, f as EventListener);
      this.cleanup.push(() => canvas.removeEventListener(t, f as EventListener));
    };
    on("pointerdown", (e) => {
      canvas.setPointerCapture(e.pointerId);
      this.drag = { x: e.clientX, y: e.clientY, moved: false };
    });
    on("pointermove", (e) => {
      if (this.drag) {
        const dx = e.clientX - this.drag.x;
        const dy = e.clientY - this.drag.y;
        if (!this.drag.moved && Math.hypot(dx, dy) < DRAG_PX) return;
        this.drag.moved = true;
        this.drag.x = e.clientX;
        this.drag.y = e.clientY;
        this.handlers.orbit(dx, dy);
        return;
      }
      this.setHover(this.partAt(e));
    });
    on("pointerup", (e) => {
      const d = this.drag;
      this.drag = null;
      if (d && !d.moved) {
        const part = this.partAt(e);
        if (part) this.handlers.turnTo(part.userData.dir.clone());
      }
    });
    on("pointerleave", () => this.setHover(null));
    on("contextmenu", (e) => e.preventDefault());
    // Seen from the default 3D view's side until the view first draws.
    this.update(new THREE.Vector3(0.6, 0.85, -0.5).normalize(), new THREE.Vector3(0, 0, 1));
  }

  /** Turns the face labels so they read upright with Z up (Top and Bottom with Y up, Bottom read from below). */
  private fixLabels(order: THREE.Vector3[]) {
    order.forEach((d, i) => {
      const tex = this.faceMaterials[i].map!;
      tex.center.set(0.5, 0.5);
      // BoxGeometry lays the +X and -X sides' textures with Y up: a quarter turn puts their text upright with Z up.
      if (d.x > 0) tex.rotation = Math.PI / 2;
      else if (d.x < 0) tex.rotation = -Math.PI / 2;
      else if (d.z < 0) tex.rotation = Math.PI;
    });
  }

  private partAt(e: PointerEvent): THREE.Mesh | null {
    const r = this.renderer.domElement.getBoundingClientRect();
    const p = new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    this.ray.setFromCamera(p, this.camera);
    const hit = this.ray.intersectObjects(this.parts, false)[0];
    return (hit?.object as THREE.Mesh) ?? null;
  }

  private setHover(m: THREE.Mesh | null) {
    if (m === this.hovered) return;
    const tint = (mesh: THREE.Mesh, on: boolean) => {
      (mesh.material as THREE.MeshBasicMaterial).opacity = on ? 0.45 : 0;
    };
    if (this.hovered) tint(this.hovered, false);
    this.hovered = m;
    if (m) tint(m, true);
    this.render();
  }

  /** Follows the view's camera: the cube is seen from the same direction with the same up. */
  update(viewForward: THREE.Vector3, viewUp: THREE.Vector3) {
    this.camera.position.copy(viewForward).multiplyScalar(-6);
    this.camera.up.copy(viewUp);
    this.camera.lookAt(0, 0, 0);
    this.camera.updateProjectionMatrix();
    this.render();
  }

  setVisible(on: boolean) {
    this.element.style.display = on ? "" : "none";
  }

  private render() {
    this.renderer.render(this.scene, this.camera);
  }

  dispose() {
    for (const f of this.cleanup) f();
    this.renderer.dispose();
    this.element.remove();
  }
}
