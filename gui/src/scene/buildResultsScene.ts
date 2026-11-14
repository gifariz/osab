// Results -> drawing data: deformed shape, member and link force diagrams, shell force contours, support
// and spring reactions. Uses the solver's own local axes and values, so nothing here re-derives the mechanics.

import { t } from "../i18n";
import { centroid } from "../model/area";
import { add, pos, scale, sub } from "../model/geometry";
import { withStories } from "../model/ops";
import type { CaseResult, Component, ElementResult, Model, Results, ShellView, Vec3 } from "../model/types";
import type { DisplayMode, ViewSpec } from "../state/store";
import { addAreaFace, areaCorners, contourMesh, shellCornerValues } from "./areas";
import { frameQuantity, modelUnits, shellQuantity } from "../units/display";
import { areaOf, nodeOf } from "./lookup";
import { cornersOf, filteredOut, jointPositions } from "./mesh";
import { RESULT_COLORS } from "./palette";
import { emptyScene, type SceneData } from "./types";
import { inView } from "./view";

export { RESULT_COLORS };

export interface ResultsSceneOptions {
  view: ViewSpec;
  mode: DisplayMode;
  caseId: string | null;
  component: Component;
  /** Multiplies the automatic scale. */
  scale: number;
  /** Display > Mode Shape: the mode drawn (1-based). */
  modeNumber?: number;
  showValues: boolean;
  tensionSide: boolean;
  /** Typical model length (bounds diagonal). */
  size: number;
  /** Objects not drawn (View > Hide). */
  hidden?: { nodes?: string[]; frames?: string[]; links?: string[]; areas?: string[] };
  /** Shell force contour shown in "shells" mode, and whether it is averaged at joints. */
  shellComponent?: ShellView;
  averaged?: boolean;
  /** View > Show Object Types: types not drawn ("frame:column", "area:opening"…). */
  hiddenTypes?: string[];
}

export interface ResultsScene {
  scene: SceneData;
  /** Largest magnitude shown (displacement or force component), for the legend. */
  maxValue: number;
  /** Colour range of shell contours. */
  legend?: { min: number; max: number };
}

/** Diagram direction for a component: local 2 for P, V2, T, M3; local 3 for V3, M2. */
export function diagramAxis(c: Component): 1 | 2 {
  return c === "V3" || c === "M2" ? 2 : 1;
}

/**
 * Sign of the offset along the diagram axis for a positive value. Moments go on the tension side
 * by default: positive M3 compresses the +2 face, so it is drawn towards -2 (and M2
 * towards -3). Other components are drawn towards the positive axis.
 */
export function diagramSign(c: Component, tensionSide: boolean): 1 | -1 {
  if (c === "M3" || c === "M2") return tensionSide ? -1 : 1;
  return 1;
}

const stationPoint = (m: Model, er: ElementResult, x: number): Vec3 | null => {
  const a = nodeOf(m, er.nodes[0]);
  return a ? add(pos(a), scale(er.axes[0], x)) : null;
};

/** Members and rigid links of a load case (links have the same result fields as members). */
export function resultMembers(c: CaseResult): { er: ElementResult; kind: "frame" | "link" }[] {
  return [...c.frames.map((er) => ({ er, kind: "frame" as const })), ...(c.links ?? []).map((er) => ({ er, kind: "link" as const }))];
}

const elementInView = (m: Model, er: ElementResult, o: ResultsSceneOptions, kind: "frame" | "link") => {
  if (hiddenSet(kind === "link" ? o.hidden?.links : o.hidden?.frames).has(er.element)) return false;
  const a = nodeOf(m, er.nodes[0]);
  const b = nodeOf(m, er.nodes[1]);
  return !!a && !!b && inView(pos(a), o.view) && inView(pos(b), o.view);
};

const nodeShown = (id: string, p: Vec3, o: ResultsSceneOptions) => !hiddenSet(o.hidden?.nodes).has(id) && inView(p, o.view);

/** The ids of a hidden list as a set (made once per list). */
const hiddenSets = new WeakMap<string[], Set<string>>();
const noIds = new Set<string>();
function hiddenSet(ids: string[] | undefined): Set<string> {
  if (!ids?.length) return noIds;
  let s = hiddenSets.get(ids);
  if (!s) hiddenSets.set(ids, (s = new Set(ids)));
  return s;
}

/** Key of a result in the case selectors: the id, plus "|max" or "|min" for an envelope's steps. */
export const resultKey = (c: CaseResult) => (c.step ? `${c.id}|${c.step}` : c.id);

/** Name shown for a result: the case or combination id, with (Max) / (Min) for an envelope. */
export const resultLabel = (c: CaseResult) => (c.step ? t(c.step === "max" ? "res.max" : "res.min", { id: c.id }) : c.id);

/** Index of the result with this key (or plain id), else of the first one (-1: no results). */
export function caseIndex(r: Results, key: string | null): number {
  const i = r.cases.findIndex((c) => resultKey(c) === key);
  const j = i >= 0 ? i : r.cases.findIndex((c) => c.id === key);
  return j >= 0 ? j : r.cases.length ? 0 : -1;
}

/** The result with this key (or plain id), else the first one. */
export function caseOf(r: Results, key: string | null): CaseResult | undefined {
  return r.cases[caseIndex(r, key)];
}

/** The diaphragms' centres of mass (CM) and rigidity (CR) at the story a plan view shows. */
function addCentres(scene: SceneData, m: Model, r: Results, view: ViewSpec) {
  if (view.kind !== "plan") return;
  const stories = withStories(m).stories ?? [];
  for (const c of r.diaphragm_centers ?? []) {
    const z = stories.find((s) => s.id === c.story)?.z;
    if (z === undefined || Math.abs(z - view.at) > 1e-6) continue;
    for (const [x, y, color, key] of [
      [c.xcm, c.ycm, RESULT_COLORS.centreMass, "vp.centreMass"],
      [c.xcr, c.ycr, RESULT_COLORS.centreRigidity, "vp.centreRigidity"],
    ] as const) {
      if (x === null || x === undefined || y === null || y === undefined) continue;
      scene.points.push({ position: [x, y, z], color, size: 10 });
      scene.labels.push({ position: [x, y, z], text: t(key, { id: c.diaphragm }), className: "lbl-centre" });
    }
  }
}

export function buildResultsScene(m: Model, r: Results, options: ResultsSceneOptions): ResultsScene {
  const scene = emptyScene();
  if (options.mode === "modes") return modeShape(scene, m, r, options);
  addCentres(scene, m, r, options.view);
  const c = caseOf(r, options.caseId);
  if (!c || options.mode === "model") return { scene, maxValue: 0 };
  // Objects of the types filtered out are treated as hidden.
  const out = filteredOut(m, options.hiddenTypes);
  const o: ResultsSceneOptions = {
    ...options,
    hidden: { ...options.hidden, frames: [...(options.hidden?.frames ?? []), ...out.frames], areas: [...(options.hidden?.areas ?? []), ...out.areas] },
  };
  if (o.mode === "deformed") return deformed(scene, m, r, c, o);
  if (o.mode === "forces") return forces(scene, m, c, o);
  if (o.mode === "shells") return shells(scene, m, r, c, o);
  return reactions(scene, m, c, o);
}

/**
 * Whether the edge of an element of `area` between joints `a` and `b` was drawn already: elements of one area
 * share their inner edges, and each is drawn once (the same line twice looks the same).
 */
function edgeOnce() {
  const seen = new Set<string>();
  return (area: string, a: string, b: string) => {
    const key = a < b ? `${area}|${a}|${b}` : `${area}|${b}|${a}`;
    if (seen.has(key)) return true;
    seen.add(key);
    return false;
  };
}

/** Corners of an area if it is shown in this view (not hidden, every corner in the view plane). */
function shownArea(m: Model, o: ResultsSceneOptions, id: string, nodes: string[]): Vec3[] | null {
  if (hiddenSet(o.hidden?.areas).has(id)) return null;
  const p = areaCorners(m, nodes);
  return p && p.every((q) => inView(q, o.view)) ? p : null;
}

/** Whether the area object with this id is shown (its mesh elements are drawn with it). */
function areaShown(m: Model, o: ResultsSceneOptions, id: string): boolean {
  const a = areaOf(m, id);
  return !!a && !!shownArea(m, o, a.id, a.nodes);
}

function shells(scene: SceneData, m: Model, r: Results, c: CaseResult, o: ResultsSceneOptions): ResultsScene {
  const comp = o.shellComponent ?? "M11";
  // Elements of the analysis mesh (corners at mesh joints `~1`…) or areas analysed as one element.
  const at = jointPositions(m, r.mesh);
  const list = (c.shells ?? []).filter((s) => areaShown(m, o, s.area) && cornersOf(at, s.nodes));
  const values = shellCornerValues(list, comp, !!o.averaged);
  let min = Infinity;
  let max = -Infinity;
  for (const v of values.values()) for (const x of v) [min, max] = [Math.min(min, x), Math.max(max, x)];
  if (!list.length) [min, max] = [0, 0];
  // Value labels at the largest and smallest corner (hover an area for all its corners).
  const extremes: { at: Vec3; v: number }[] = [];
  const drawn = edgeOnce();
  for (const s of list) {
    const p = cornersOf(at, s.nodes)!;
    const v = values.get(s)!;
    const pick = { kind: "area" as const, id: s.area };
    scene.meshes.push(contourMesh(p, v, min, max, pick));
    for (let k = 0; k < p.length; k++) {
      if (!drawn(s.area, s.nodes[k], s.nodes[(k + 1) % p.length])) scene.lines.push({ a: p[k], b: p[(k + 1) % p.length], color: RESULT_COLORS.edge, width: 1, pick });
    }
    // Element corner values sit a little inside their element.
    const mid = centroid(p);
    p.forEach((q, k) => {
      const at = o.averaged ? q : add(q, scale(sub(mid, q), 0.18));
      if (v[k] === max && !extremes.some((e) => e.v === max)) extremes.push({ at, v: v[k] });
      if (v[k] === min && !extremes.some((e) => e.v === min)) extremes.push({ at, v: v[k] });
    });
  }
  const range = Math.max(Math.abs(min), Math.abs(max));
  if (o.showValues) for (const e of extremes) scene.labels.push({ position: e.at, text: modelUnits(m).fmt(e.v, shellQuantity(comp), range), className: e.v < 0 ? "lbl-value neg" : "lbl-value" });
  return { scene, maxValue: range, legend: { min, max } };
}

function deformed(scene: SceneData, m: Model, r: Results, c: CaseResult, o: ResultsSceneOptions): ResultsScene {
  let max = 0;
  for (const d of c.displacements) max = Math.max(max, Math.hypot(d.ux, d.uy, d.uz));
  const members = resultMembers(c);
  for (const { er } of members) for (const s of er.stations) max = Math.max(max, Math.hypot(s.ux, s.uy, s.uz));
  const k = max > 0 ? ((0.08 * o.size) / max) * o.scale : 0;
  for (const { er, kind } of members) {
    if (!elementInView(m, er, o, kind)) continue;
    const color = kind === "link" ? RESULT_COLORS.link : RESULT_COLORS.deformed;
    let prev: Vec3 | null = null;
    for (const s of er.stations) {
      const p0 = stationPoint(m, er, s.x);
      if (!p0) break;
      const p = add(p0, scale([s.ux, s.uy, s.uz], k));
      if (prev) scene.lines.push({ a: prev, b: p, color, width: kind === "link" ? 4 : 2.5, pick: { kind, id: er.element } });
      prev = p;
    }
  }
  // Areas: their mesh elements when the model was meshed, else each area as its one element.
  const at = jointPositions(m, r.mesh);
  const disp = new Map(c.displacements.map((d) => [d.node, d]));
  const moved = (nodes: string[]): Vec3[] | null => {
    const p = cornersOf(at, nodes);
    if (!p) return null;
    const out: Vec3[] = [];
    for (let i = 0; i < nodes.length; i++) {
      const d = disp.get(nodes[i]);
      if (!d) return null;
      out.push(add(p[i], scale([d.ux, d.uy, d.uz], k)));
    }
    return out;
  };
  const elements = r.mesh?.shells ?? [];
  const meshed = new Set(elements.map((el) => el.parent));
  const faces = [
    ...elements.map((el) => ({ area: el.parent, nodes: el.nodes })),
    ...m.areas.filter((a) => !meshed.has(a.id) && a.section && !a.opening).map((a) => ({ area: a.id, nodes: a.nodes })),
  ];
  const drawn = edgeOnce();
  for (const f of faces) {
    if (!areaShown(m, o, f.area)) continue;
    const p = moved(f.nodes);
    const skip = (k: number) => drawn(f.area, f.nodes[k], f.nodes[(k + 1) % f.nodes.length]);
    if (p) addAreaFace(scene, p, RESULT_COLORS.deformed, 0.18, RESULT_COLORS.deformed, elements.length ? 1 : 1.5, false, { kind: "area", id: f.area }, undefined, skip);
  }
  for (const d of c.displacements) {
    const n = nodeOf(m, d.node);
    if (!n || !nodeShown(d.node, pos(n), o)) continue;
    const p = add(pos(n), scale([d.ux, d.uy, d.uz], k));
    scene.points.push({ position: p, color: RESULT_COLORS.deformed, size: 6, pick: { kind: "node", id: d.node } });
  }
  return { scene, maxValue: max };
}

/**
 * Display > Mode Shape: mode `o.modeNumber` of the modal case, drawn from its joint displacements (frames
 * and their mesh pieces as straight lines, areas as their elements). Every point carries its displacement as
 * `motion`, so a view can swing the drawn shape for the animation without building it again.
 */
function modeShape(scene: SceneData, m: Model, r: Results, o: ResultsSceneOptions): ResultsScene {
  const mode = r.modal?.modes.find((x) => x.mode === (o.modeNumber ?? 1)) ?? r.modal?.modes[0];
  if (!mode) return { scene, maxValue: 0 };
  const out = filteredOut(m, o.hiddenTypes);
  let max = 0;
  for (const d of mode.shape) max = Math.max(max, Math.hypot(d.ux, d.uy, d.uz));
  const k = max > 0 ? ((0.1 * o.size) / max) * o.scale : 0;
  const at = jointPositions(m, r.mesh);
  const disp = new Map(mode.shape.map((d) => [d.node, d]));
  // Each point's displacement at full size (the animation swings it) and where it is drawn then.
  const motion = (n: string): Vec3 => {
    const d = disp.get(n)!;
    return scale([d.ux, d.uy, d.uz], k);
  };
  const moved = (n: string): Vec3 | null => {
    const p = at.get(n);
    return p && disp.has(n) ? add(p, motion(n)) : null;
  };
  const hidden = new Set([...(o.hidden?.frames ?? []), ...out.frames]);
  const pieces = new Map<string, string[][]>();
  for (const el of r.mesh?.frames ?? []) {
    const list = pieces.get(el.parent);
    if (list) list.push(el.nodes);
    else pieces.set(el.parent, [el.nodes]);
  }
  for (const e of m.frames) {
    if (hidden.has(e.id)) continue;
    const [a, b] = [nodeOf(m, e.nodes[0]), nodeOf(m, e.nodes[1])];
    if (!a || !b || !inView(pos(a), o.view) || !inView(pos(b), o.view)) continue;
    for (const [i, j] of pieces.get(e.id) ?? [e.nodes]) {
      const [p, q] = [moved(i), moved(j)];
      if (p && q) scene.lines.push({ a: p, b: q, color: RESULT_COLORS.deformed, width: 2.5, pick: { kind: "frame", id: e.id }, motion: [motion(i), motion(j)] });
    }
  }
  const elements = r.mesh?.shells ?? [];
  const meshed = new Set(elements.map((el) => el.parent));
  const faces = [
    ...elements.map((el) => ({ area: el.parent, nodes: el.nodes })),
    ...m.areas.filter((a) => !meshed.has(a.id) && a.section && !a.opening).map((a) => ({ area: a.id, nodes: a.nodes })),
  ];
  const oh: ResultsSceneOptions = { ...o, hidden: { ...o.hidden, areas: [...(o.hidden?.areas ?? []), ...out.areas] } };
  const drawn = edgeOnce();
  for (const f of faces) {
    if (!areaShown(m, oh, f.area)) continue;
    const p = f.nodes.map(moved);
    const skip = (k: number) => drawn(f.area, f.nodes[k], f.nodes[(k + 1) % f.nodes.length]);
    if (p.every((x) => x)) addAreaFace(scene, p as Vec3[], RESULT_COLORS.deformed, 0.18, RESULT_COLORS.deformed, 1, false, { kind: "area", id: f.area }, f.nodes.map(motion), skip);
  }
  for (const d of mode.shape) {
    const n = nodeOf(m, d.node);
    if (!n || !nodeShown(d.node, pos(n), o)) continue;
    scene.points.push({ position: moved(d.node)!, color: RESULT_COLORS.deformed, size: 6, pick: { kind: "node", id: d.node }, motion: motion(d.node) });
  }
  return { scene, maxValue: max };
}

function forces(scene: SceneData, m: Model, c: CaseResult, o: ResultsSceneOptions): ResultsScene {
  const comp = o.component;
  let max = 0;
  const members = resultMembers(c);
  for (const { er } of members) for (const s of er.stations) max = Math.max(max, Math.abs(s[comp]));
  const k = max > 0 ? ((0.1 * o.size) / max) * o.scale : 0;
  const axisIndex = diagramAxis(comp);
  const sign = diagramSign(comp, o.tensionSide);
  for (const { er, kind } of members) {
    if (!elementInView(m, er, o, kind)) continue;
    const pick = { kind, id: er.element };
    const dir = scale(er.axes[axisIndex], sign * k);
    const base = (x: number) => stationPoint(m, er, x)!;
    const tip = (x: number, v: number) => add(base(x), scale(dir, v));
    const st = er.stations;
    const pos_: number[] = [];
    const neg: number[] = [];
    const tri = (out: number[], a: Vec3, b: Vec3, cc: Vec3) => out.push(...a, ...b, ...cc);
    for (let i = 0; i + 1 < st.length; i++) {
      const [s0, s1] = [st[i], st[i + 1]];
      const [v0, v1] = [s0[comp], s1[comp]];
      if (s1.x - s0.x <= 0) {
        // A jump at a point load: close the diagram with a straight edge.
        scene.lines.push({ a: tip(s0.x, v0), b: tip(s1.x, v1), color: RESULT_COLORS.outline, width: 1.5, pick });
        continue;
      }
      scene.lines.push({ a: tip(s0.x, v0), b: tip(s1.x, v1), color: RESULT_COLORS.outline, width: 1.5, pick });
      if (v0 * v1 < 0) {
        // Split where the diagram crosses zero so each part gets its sign's colour.
        const xz = s0.x + ((s1.x - s0.x) * v0) / (v0 - v1);
        tri(v0 > 0 ? pos_ : neg, base(s0.x), tip(s0.x, v0), base(xz));
        tri(v1 > 0 ? pos_ : neg, base(xz), tip(s1.x, v1), base(s1.x));
      } else {
        const out = v0 + v1 >= 0 ? pos_ : neg;
        tri(out, base(s0.x), tip(s0.x, v0), tip(s1.x, v1));
        tri(out, base(s0.x), tip(s1.x, v1), base(s1.x));
      }
    }
    // Closing edges at the ends.
    scene.lines.push({ a: base(0), b: tip(0, st[0][comp]), color: RESULT_COLORS.outline, width: 1 });
    const last = st[st.length - 1];
    scene.lines.push({ a: base(last.x), b: tip(last.x, last[comp]), color: RESULT_COLORS.outline, width: 1 });
    if (pos_.length) scene.meshes.push({ positions: pos_, color: RESULT_COLORS.positive, opacity: 0.55 });
    if (neg.length) scene.meshes.push({ positions: neg, color: RESULT_COLORS.negative, opacity: 0.55 });
    if (o.showValues) {
      for (const s of diagramLabelStations(er, comp)) {
        scene.labels.push({
          position: tip(s.x, s[comp]),
          text: modelUnits(m).fmt(s[comp], frameQuantity(comp), max),
          className: s[comp] < 0 ? "lbl-value neg" : "lbl-value",
        });
      }
    }
  }
  return { scene, maxValue: max };
}

/** Stations worth a value label: both ends, and the largest interior value when it is a peak. */
export function diagramLabelStations(er: ElementResult, comp: Component) {
  const st = er.stations;
  if (st.length === 0) return [];
  const out = [st[0], st[st.length - 1]];
  let peak = st[0];
  for (const s of st) if (Math.abs(s[comp]) > Math.abs(peak[comp])) peak = s;
  const ends = Math.max(Math.abs(st[0][comp]), Math.abs(st[st.length - 1][comp]));
  const interior = peak.x > 0.02 * er.length && peak.x < 0.98 * er.length;
  if (interior && Math.abs(peak[comp]) > ends * (1 + 1e-6)) out.push(peak);
  return out;
}

function reactions(scene: SceneData, m: Model, c: CaseResult, o: ResultsSceneOptions): ResultsScene {
  let max = 0;
  for (const r of c.reactions) max = Math.max(max, Math.abs(r.fx), Math.abs(r.fy), Math.abs(r.fz));
  const len = 0.07 * o.size;
  const names = ["FX", "FY", "FZ", "MX", "MY", "MZ"] as const;
  for (const r of c.reactions) {
    const n = nodeOf(m, r.node);
    if (!n || !nodeShown(r.node, pos(n), o)) continue;
    const p = pos(n);
    const values = [r.fx, r.fy, r.fz, r.mx, r.my, r.mz];
    const all = Math.max(...values.map(Math.abs));
    const texts: string[] = [];
    values.forEach((v, k) => {
      if (Math.abs(v) <= 1e-9 * all || v === 0) return;
      const d: Vec3 = [0, 0, 0];
      d[k % 3] = Math.sign(v);
      if (k < 3) scene.arrows.push({ tail: add(p, scale(d, -len)), head: p, color: RESULT_COLORS.reaction });
      else scene.arrows.push({ tail: p, head: add(p, scale(d, len * 0.8)), color: RESULT_COLORS.reaction, double: true });
      texts.push(`${names[k]} ${modelUnits(m).fmt(v, k < 3 ? "force" : "moment", all)}`);
    });
    if (texts.length) scene.labels.push({ position: add(p, [len * 0.25, 0, -len * 0.45]), text: texts.join("\n"), className: "lbl-value" });
  }
  return { scene, maxValue: max };
}
