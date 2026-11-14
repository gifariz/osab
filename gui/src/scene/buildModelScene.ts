// Model -> drawing data: grid, joints, members, rigid links, areas, supports, springs and the loads of one
// load case. The view options (View > Set Display Options) choose the texts and symbols on the objects,
// the object types drawn, the colours and the special effects (extrude, shrink, fill). How an object looks
// when selected or highlighted comes from the same rules (`modelStyles`), so a view can draw the model once
// and change only the looks of the objects a selection touches.

import { areaAxes, centroid, labelPoint } from "../model/area";
import { add, localAxes, pos, scale, sub, unit } from "../model/geometry";
import { t } from "../i18n";
import { AREA_TYPES, FRAME_TYPES, objectTypes } from "../model/objectTypes";
import { areaMidPlaneOffset, insertionText } from "../model/insertion";
import { endOffsets } from "../model/offsets";
import { gridOf, restraintsOf, springsOf, withStories } from "../model/ops";
import { DOFS, FORCES, type Area, type Frame, type Id, type Link, type LoadDir, type MeshInfo, type Model, type ModelNode, type Vec3 } from "../model/types";
import { frameOf, nodeOf } from "./lookup";
import { filteredOut, meshEdges, storedEdges } from "./mesh";
import type { Selection, ViewSpec } from "../state/store";
import { addAreaFace, addAreaLoads, areaCorners, areaSolid, frameSolid } from "./areas";
import { modelUnits } from "../units/display";
import { COLORS } from "./palette";
import { emptyScene, pickKey, type Bounds, type PickStyle, type SceneData, type SceneGlyph } from "./types";
import { defaultViewOptions, type ViewOptions } from "./viewOptions";
import { boundsSize, inView, modelBounds } from "./view";

export { COLORS };

/** Most object texts drawn in one view; beyond it the rest are left out, so the view stays fast. */
export const MAX_TEXTS = 2000;
/** Shrunk members and areas: their size about their centre. */
const SHRINK = 0.85;

export interface ModelSceneOptions {
  view: ViewSpec;
  selection: Partial<Selection>;
  highlight: Partial<Selection>;
  /** Draw the loads of this load case. */
  loadsPattern: Id | null;
  /** Results are drawn on top: show the model faintly, without loads or object texts. */
  dimmed: boolean;
  /** Objects not drawn (View > Hide). */
  hidden?: Partial<Selection>;
  /** What the view shows (View > Set Display Options); missing = the defaults. */
  options?: ViewOptions;
  /** View > Show Analysis Mesh: the mesh to draw on the areas. */
  mesh?: MeshInfo | null;
}

export interface ModelScene {
  scene: SceneData;
  /** Box to fit the view to (model and grid plus room for labels). */
  bounds: Bounds;
  size: number;
  /** Object texts left out because there were more than MAX_TEXTS. */
  textsLeftOut: number;
}

/** Whether a property modifier list changes anything. */
const modified = (mods: Record<string, number | undefined> | undefined) => !!mods && Object.values(mods).some((v) => v !== undefined && v !== 1);

/** Points moved towards `c` (shrunk objects). */
const shrunk = (p: Vec3[], c: Vec3): Vec3[] => p.map((q) => add(c, scale(sub(q, c), SHRINK)));

/** How an object is drawn: colour, dashed lines, face opacity (areas, when drawn with a face), dot size (joints). */
interface Look {
  color: string;
  dashed: boolean;
  opacity?: number;
  size?: number;
}

/** The look of each kind of object, plain (`sel`, `hi` false), selected or highlighted. */
interface Looks {
  frame(e: Frame, sel: boolean, hi: boolean): Look;
  link(l: Link, sel: boolean, hi: boolean): Look;
  area(a: Area, sel: boolean, hi: boolean): Look;
  node(n: ModelNode, sel: boolean, hi: boolean): Look;
}

function looksOf(m: Model, o: ModelSceneOptions): Looks {
  const vo = o.options ?? defaultViewOptions();
  const types = objectTypes(m);
  // Colours: by section (the default), object type or material.
  const pick = (list: string[], i: number) => list[Math.max(i, 0) % list.length];
  const byIndex = (ids: Id[], list: string[]) => new Map(ids.map((id, i) => [id, pick(list, i)]));
  const sectionColor = byIndex(m.sections.map((x) => x.id), COLORS.sections);
  const areaColor = byIndex(m.area_sections.map((x) => x.id), COLORS.areas);
  const frameMaterial = byIndex(m.materials.map((x) => x.id), COLORS.sections);
  const areaMaterial = byIndex(m.materials.map((x) => x.id), COLORS.areas);
  const areaSectionMaterial = new Map(m.area_sections.map((x) => [x.id, x.material]));
  const frameColor = (e: Frame) => {
    if (!e.section) return COLORS.node;
    if (vo.colorBy === "types") return pick(COLORS.sections, FRAME_TYPES.indexOf(types.frames.get(e.id) ?? "beam"));
    if (vo.colorBy === "materials") return frameMaterial.get(e.material) ?? COLORS.node;
    return sectionColor.get(e.section) ?? COLORS.node;
  };
  const areaFill = (a: Area) => {
    if (!a.section) return COLORS.node;
    if (vo.colorBy === "types") return pick(COLORS.areas, AREA_TYPES.indexOf(types.areas.get(a.id) ?? "floor"));
    if (vo.colorBy === "materials") return areaMaterial.get(areaSectionMaterial.get(a.section) ?? "") ?? COLORS.node;
    return areaColor.get(a.section) ?? COLORS.node;
  };
  const extrudeAreas = vo.extrudeAreas && !o.dimmed;
  const diaphragms = vo.diaphragms && !o.dimmed;
  const colorOf = (sel: boolean, hi: boolean, own: () => string) => (hi ? COLORS.highlight : sel ? COLORS.selected : o.dimmed ? COLORS.dimmed : own());
  return {
    // Selected members are dashed; truss members always.
    frame: (e, sel, hi) => ({ color: colorOf(sel, hi, () => frameColor(e)), dashed: sel || e.type === "truss" }),
    link: (_l, sel, hi) => ({ color: colorOf(sel, hi, () => COLORS.link), dashed: sel }),
    area: (a, sel, hi) => {
      const color = colorOf(sel, hi, () => areaFill(a));
      // An opening: its outline only (a see-through face so it can be picked).
      if (a.opening && !o.dimmed) return { color, opacity: sel ? 0.25 : 0.04, dashed: true };
      // A null area (no section): a faint face.
      if (!a.section && !o.dimmed && !extrudeAreas) return { color, opacity: sel ? 0.25 : 0.08, dashed: sel };
      if (extrudeAreas) return { color, dashed: sel };
      // Results are drawn on top: edges only.
      if (o.dimmed) return { color, dashed: false };
      // Fill off: edges only (a nearly clear face keeps the area pickable).
      return { color, opacity: vo.fill ? (sel ? 0.55 : 0.4) : sel ? 0.2 : 0.03, dashed: sel };
    },
    node: (n, sel, hi) => ({
      color: colorOf(sel, hi, () => (diaphragms && n.diaphragm ? COLORS.diaphragm : COLORS.node)),
      dashed: false,
      // Invisible joints: no dot, unless selected or highlighted.
      size: sel || hi ? 9 : vo.jointsInvisible ? 0 : 6,
    }),
  };
}

/**
 * The looks of the selected and highlighted objects (keys by `pickKey`): with the scene built without a
 * selection, drawing these instead of the objects' own looks gives the scene `buildModelScene` builds with it.
 */
export function modelStyles(m: Model, o: ModelSceneOptions): Map<string, PickStyle> {
  const looks = looksOf(m, o);
  const out = new Map<string, PickStyle>();
  const add = <T extends { id: Id }>(kind: "node" | "frame" | "link" | "area", list: T[], look: (x: T, sel: boolean, hi: boolean) => Look, sel: Id[] = [], hi: Id[] = []) => {
    const [s, h] = [new Set(sel), new Set(hi)];
    if (!s.size && !h.size) return;
    for (const x of list) {
      if (!s.has(x.id) && !h.has(x.id)) continue;
      const k = look(x, s.has(x.id), h.has(x.id));
      out.set(pickKey({ kind, id: x.id }), { color: k.color, dashed: k.dashed, ...(k.opacity !== undefined ? { opacity: k.opacity } : {}), ...(k.size !== undefined ? { size: k.size } : {}) });
    }
  };
  add("node", m.nodes, looks.node, o.selection.nodes, o.highlight.nodes);
  add("frame", m.frames, looks.frame, o.selection.frames, o.highlight.frames);
  add("link", m.links, looks.link, o.selection.links, o.highlight.links);
  add("area", m.areas, looks.area, o.selection.areas, o.highlight.areas);
  return out;
}

export function buildModelScene(m: Model, o: ModelSceneOptions): ModelScene {
  const scene = emptyScene();
  const bounds = modelBounds(m);
  const size = boundsSize(bounds);
  const vo = o.options ?? defaultViewOptions();
  if (vo.grid) addGrid(scene, m, o.view, bounds, size);
  const looks = looksOf(m, o);

  const selNodes = new Set(o.selection.nodes);
  const selElems = new Set(o.selection.frames);
  const selLinks = new Set(o.selection.links);
  const hiNodes = new Set(o.highlight.nodes);
  const hiElems = new Set(o.highlight.frames);
  const hiLinks = new Set(o.highlight.links);
  const typeHidden = new Set(vo.hiddenTypes);
  const hidNodes = new Set(o.hidden?.nodes ?? []);
  const typeOut = filteredOut(m, vo.hiddenTypes);
  const hidElems = new Set([...(o.hidden?.frames ?? []), ...typeOut.frames]);
  const hidLinks = new Set(o.hidden?.links ?? []);
  const extrudeFrames = vo.extrudeFrames && !o.dimmed;
  const extrudeAreas = vo.extrudeAreas && !o.dimmed;
  const shrink = vo.shrink && !o.dimmed;
  const diaphragms = vo.diaphragms && !o.dimmed;
  const axisLen = 0.05 * size;
  const addAxes = (at: Vec3, axes: [Vec3, Vec3, Vec3]) =>
    axes.forEach((d, k) => scene.arrows.push({ tail: at, head: add(at, scale(d, axisLen)), color: COLORS.axes[k] }));
  // Object texts: one label per object with its texts stacked, at most MAX_TEXTS labels.
  let texts = 0;
  let textsLeftOut = 0;
  const addText = (position: Vec3, lines: (string | false | undefined)[], className: string) => {
    const shown = lines.filter((x): x is string => !!x);
    if (!shown.length || o.dimmed) return;
    if (texts >= MAX_TEXTS) {
      textsLeftOut++;
      return;
    }
    texts++;
    scene.labels.push({ position, text: shown.join("\n"), className });
  };
  const modTag = t("vo.modTag");

  const offsets = endOffsets(m);
  for (const e of m.frames) {
    const na = nodeOf(m, e.nodes[0]);
    const nb = nodeOf(m, e.nodes[1]);
    if (!na || !nb || hidElems.has(e.id) || !inView(pos(na), o.view) || !inView(pos(nb), o.view)) continue;
    const mid = scale(add(pos(na), pos(nb)), 0.5);
    const [a, b] = shrink ? shrunk([pos(na), pos(nb)], mid) : [pos(na), pos(nb)];
    const { color, dashed } = looks.frame(e, selElems.has(e.id), hiElems.has(e.id));
    // A null frame (no section) is a thin line.
    const isNull = !e.section;
    scene.lines.push({
      a,
      b,
      color,
      width: o.dimmed ? 1.5 : extrudeFrames || isNull ? 1 : e.type === "truss" ? 2 : 3,
      dashed,
      pick: { kind: "frame", id: e.id },
    });
    if (extrudeFrames && !isNull) scene.meshes.push({ positions: frameSolid(m, e.id), color, opacity: 1, shaded: true, pick: { kind: "frame", id: e.id } });
    // End length offsets: the offset parts drawn thicker.
    const [oi, oj] = offsets.get(e.id) ?? [0, 0];
    const full = Math.hypot(...sub(pos(nb), pos(na)));
    if (vo.frame.offsets && !extrudeFrames && !o.dimmed && full > 0) {
      for (const [l, near, far] of [[oi, a, b], [oj, b, a]] as const) {
        if (l > 0) {
          const s = Math.min(l / full, 1);
          scene.lines.push({ a: near, b: add(scale(near, 1 - s), scale(far, s)), color: COLORS.offset, width: 6 });
        }
      }
    }
    // End releases: a dot a little inside each released end.
    for (const [end, near, far] of [["i", a, b], ["j", b, a]] as const) {
      if (vo.frame.releases && e.releases?.[end]?.length && !o.dimmed) {
        scene.points.push({ position: add(scale(near, 0.92), scale(far, 0.08)), color: COLORS.release, size: 8 });
      }
    }
    if (vo.frame.localAxes && !o.dimmed) addAxes(mid, localAxes(pos(na), pos(nb), e.angle ?? 0));
    const f = vo.frame;
    addText(
      mid,
      [f.labels && (e.label ?? e.id), f.ids && e.id, f.sections && (e.section || t("obj.noSection")), f.modifiers && modified(e.modifiers) && modTag, f.insertion && !isNull && t("vo.insTag", { p: insertionText(e.insertion, false) }),
        f.springs && !!e.line_spring && t("vo.springTag", { id: e.line_spring }),
        f.piers && !!e.pier && t("vo.pierTag", { id: e.pier }),
        f.spandrels && !!e.spandrel && t("vo.spandrelTag", { id: e.spandrel }),
      ],
      "lbl-elem",
    );
  }

  // Rigid links: thick lines in their own colour.
  for (const l of m.links) {
    const na = nodeOf(m, l.nodes[0]);
    const nb = nodeOf(m, l.nodes[1]);
    if (!na || !nb || typeHidden.has("link") || hidLinks.has(l.id) || !inView(pos(na), o.view) || !inView(pos(nb), o.view)) continue;
    const mid = scale(add(pos(na), pos(nb)), 0.5);
    const [a, b] = shrink ? shrunk([pos(na), pos(nb)], mid) : [pos(na), pos(nb)];
    const { color, dashed } = looks.link(l, selLinks.has(l.id), hiLinks.has(l.id));
    scene.lines.push({ a, b, color, width: o.dimmed ? 1.5 : 5, dashed, pick: { kind: "link", id: l.id } });
    if (vo.link.localAxes && !o.dimmed) addAxes(mid, localAxes(pos(na), pos(nb), 0));
    const k = vo.link;
    addText(mid, [k.labels && (l.label ?? l.id), k.ids && l.id, k.properties && t("vo.rigid")], "lbl-elem");
  }

  // Areas: a see-through face in the section's colour with its edges (picked after members).
  const selAreas = new Set(o.selection.areas);
  const hiAreas = new Set(o.highlight.areas);
  const hidAreas = new Set([...(o.hidden?.areas ?? []), ...typeOut.areas]);
  const shownArea = new Map<Id, Vec3[]>();
  for (const a of m.areas) {
    const corners = areaCorners(m, a.nodes);
    if (!corners || hidAreas.has(a.id) || !corners.every((q) => inView(q, o.view))) continue;
    shownArea.set(a.id, corners);
    const p = shrink ? shrunk(corners, centroid(corners)) : corners;
    const { color, opacity, dashed } = looks.area(a, selAreas.has(a.id), hiAreas.has(a.id));
    const ref = { kind: "area" as const, id: a.id };
    if (a.opening && !o.dimmed) {
      // An opening: its outline only (a see-through face so it can be picked).
      addAreaFace(scene, p, color, opacity!, color, 1, dashed, ref);
    } else if (!a.section && !o.dimmed && !extrudeAreas) {
      // A null area (no section): a faint face.
      addAreaFace(scene, p, color, opacity!, color, 1, dashed, ref);
    } else if (extrudeAreas) {
      const th = m.area_sections.find((x) => x.id === a.section)?.thickness ?? 0;
      // Drawn at its insertion point: the mid-plane away from the joints for a floor at its top.
      const mid = areaMidPlaneOffset(m, a, corners);
      scene.meshes.push({ positions: areaSolid(p.map((q) => add(q, mid)), th), color, opacity: 1, shaded: true, pick: ref });
      for (let k = 0; k < p.length; k++) scene.lines.push({ a: p[k], b: p[(k + 1) % p.length], color, width: 1, dashed, pick: ref });
    } else if (o.dimmed) {
      // Results are drawn on top (contours, deformed faces): edges only, so nothing fights with them.
      for (let k = 0; k < p.length; k++) scene.lines.push({ a: p[k], b: p[(k + 1) % p.length], color, width: 1, pick: ref });
    } else {
      // Fill off: edges only (a nearly clear face keeps the area pickable).
      addAreaFace(scene, p, color, opacity!, color, 1.5, dashed, ref);
    }
    const ar = vo.area;
    addText(
      labelPoint(p),
      [
        ar.labels && (a.label ?? a.id),
        ar.ids && a.id,
        ar.sections && !a.opening && (a.section || t("obj.noSection")),
        ar.modifiers && modified(a.modifiers) && modTag,
        ar.insertion && !!a.section && t("vo.insTag", { p: insertionText(a.insertion, true) }),
        ar.springs && !!a.area_spring && t("vo.springTag", { id: a.area_spring }),
        ar.piers && !!a.pier && t("vo.pierTag", { id: a.pier }),
        ar.spandrels && !!a.spandrel && t("vo.spandrelTag", { id: a.spandrel }),
      ],
      "lbl-elem",
    );
    if (ar.localAxes && !o.dimmed) addAxes(centroid(corners), areaAxes(corners, a.angle ?? 0));
    // One-way floors (decks, one-way Membranes) show their span direction in plan views.
    if (o.view.kind === "plan" && !o.dimmed && !a.opening && m.area_sections.find((x) => x.id === a.section)?.one_way) {
      for (const [q, r] of spanSymbol(corners, a.angle ?? 0)) scene.lines.push({ a: q, b: r, color: COLORS.span, width: 1.5 });
    }
  }
  // The analysis mesh on the areas shown; stored meshes are always drawn.
  const edges = o.mesh ? meshEdges(m, o.mesh, (id) => shownArea.has(id)) : storedEdges(m, (id) => shownArea.has(id));
  for (const [a, b] of edges) scene.lines.push({ a, b, color: COLORS.mesh, width: 1 });

  const jointsShown = !typeHidden.has("joint");
  const shownNode = (id: Id) => {
    const n = nodeOf(m, id);
    return n && !hidNodes.has(n.id) && inView(pos(n), o.view) ? n : null;
  };
  if (jointsShown) {
    for (const n of m.nodes) {
      const p = pos(n);
      if (hidNodes.has(n.id) || !inView(p, o.view)) continue;
      const { color, size } = looks.node(n, selNodes.has(n.id), hiNodes.has(n.id));
      scene.points.push({ position: p, color, size: size!, pick: { kind: "node", id: n.id } });
      addText(p, [vo.joint.labels && (n.label ?? n.id), vo.joint.ids && n.id], "lbl-node");
    }
  }

  if (vo.supports) {
    for (const s of m.supports) {
      const n = shownNode(s.node);
      if (n) scene.glyphs.push({ position: pos(n), kind: glyphKind(restraintsOf(m, s.node)), color: COLORS.support });
    }
    for (const s of m.springs) {
      const n = shownNode(s.node);
      if (n && springsOf(m, s.node).some((k) => k)) scene.glyphs.push({ position: pos(n), kind: "spring", color: COLORS.spring });
    }
  }

  // Joint masses: an arrow along each translation that has mass, with its value.
  if (vo.joint.masses && jointsShown && !o.dimmed) {
    const U = modelUnits(m);
    const len = 0.06 * size;
    for (const ms of m.masses ?? []) {
      const n = shownNode(ms.node);
      if (!n) continue;
      for (let k = 0; k < 3; k++) {
        const v = ms[DOFS[k]] ?? 0;
        if (!v) continue;
        const d: Vec3 = [0, 0, 0];
        d[k] = 1;
        const head = add(pos(n), scale(d, len));
        scene.arrows.push({ tail: pos(n), head, color: COLORS.mass });
        scene.labels.push({ position: add(head, scale(d, len * 0.15)), text: `${DOFS[k].toUpperCase()} ${U.fmt(v, "mass")}`, className: "lbl-mass" });
      }
    }
  }

  if (diaphragms) addDiaphragmExtents(scene, m, o.view, hidNodes, jointsShown, shownArea);

  if (!o.dimmed && o.loadsPattern !== null) {
    addLoads(scene, m, o.loadsPattern, o.view, size, hidNodes, hidElems);
    addAreaLoads(scene, m, o.loadsPattern, (id) => shownArea.get(id) ?? null);
  }
  // Fit to the grid with room for its bubbles and labels.
  const margin = Math.max(0.06 * size, 0.5) * 1.8;
  const fit: Bounds = { min: bounds.min.map((v) => v - margin) as Vec3, max: bounds.max.map((v) => v + margin) as Vec3 };
  return { scene, bounds: fit, size, textsLeftOut };
}

/**
 * The span symbol of a one-way floor: a line along local axis 1 through a point inside the floor, 0.45 of the
 * floor's length through that point that way (at most 0.9 of its width), with a short slash at each end, the two
 * slashes turned opposite ways.
 */
export function spanSymbol(corners: Vec3[], angle: number): [Vec3, Vec3][] {
  const [d1, d2] = areaAxes(corners, angle);
  const c = labelPoint(corners);
  const dot = (u: Vec3, v: Vec3) => u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
  const uv = corners.map((q) => [dot(sub(q, c), d1), dot(sub(q, c), d2)]);
  /** The floor's length through c along local 1 (k = 0) or local 2 (k = 1). */
  const chord = (k: 0 | 1) => {
    let lo = 0;
    let hi = 0;
    for (let i = 0; i < uv.length; i++) {
      const p = uv[i];
      const q = uv[(i + 1) % uv.length];
      const [pa, pb, qa, qb] = [p[k], p[1 - k], q[k], q[1 - k]];
      if ((pb > 0) === (qb > 0) || pb === qb) continue;
      const x = pa + ((qa - pa) * pb) / (pb - qb);
      lo = Math.min(lo, x);
      hi = Math.max(hi, x);
    }
    return hi - lo;
  };
  const len = Math.min(0.45 * chord(0), 0.9 * chord(1));
  if (!(len > 0)) return [];
  const a = add(c, scale(d1, len / 2));
  const b = add(c, scale(d1, -len / 2));
  const tick = add(scale(d2, 0.18 * len), scale(d1, -0.33 * len));
  return [
    [a, b],
    [a, add(a, tick)],
    [b, sub(b, tick)],
  ];
}

/** Corners of the convex hull of points in plan (x, y), counter-clockwise; fewer than 3 points as they are. */
export function planHull(points: Vec3[]): Vec3[] {
  const p = [...points].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  if (p.length < 3) return p;
  const cross = (o: Vec3, a: Vec3, b: Vec3) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const half = (list: Vec3[]) => {
    const h: Vec3[] = [];
    for (const q of list) {
      while (h.length >= 2 && cross(h[h.length - 2], h[h.length - 1], q) <= 1e-12) h.pop();
      h.push(q);
    }
    h.pop();
    return h;
  };
  const hull = [...half(p), ...half([...p].reverse())];
  // All points on one line: its two ends.
  return hull.length ? hull : [p[0], p[p.length - 1]];
}

/**
 * Diaphragm extent: per diaphragm and story level, dashed lines from the outer joints (the corners of
 * the plan hull of its joints and of its areas' corners) to the centre, a dot there and the name.
 */
function addDiaphragmExtents(scene: SceneData, m: Model, view: ViewSpec, hidNodes: Set<Id>, jointsShown: boolean, shownArea: Map<Id, Vec3[]>) {
  const groups = new Map<string, { id: Id; points: Vec3[] }>();
  const put = (id: Id, p: Vec3) => {
    const key = `${id}|${p[2].toFixed(6)}`;
    if (!groups.has(key)) groups.set(key, { id, points: [] });
    groups.get(key)!.points.push(p);
  };
  if (jointsShown) for (const n of m.nodes) if (n.diaphragm && !hidNodes.has(n.id) && inView(pos(n), view)) put(n.diaphragm, pos(n));
  for (const a of m.areas) {
    const p = a.diaphragm ? shownArea.get(a.id) : undefined;
    if (p) p.forEach((q) => put(a.diaphragm!, q));
  }
  for (const { id, points } of groups.values()) {
    const hull = planHull(points);
    const centre = scale(hull.reduce((s, q) => add(s, q), [0, 0, 0] as Vec3), 1 / hull.length);
    if (hull.length > 1) for (const q of hull) scene.lines.push({ a: q, b: centre, color: COLORS.diaphragm, width: 1, dashed: true });
    scene.points.push({ position: centre, color: COLORS.diaphragm, size: 9 });
    scene.labels.push({ position: centre, text: id, className: "lbl-dia" });
  }
}

function glyphKind(r: boolean[]): SceneGlyph["kind"] {
  if (r.every((x) => x)) return "fixed";
  if (r[0] && r[1] && r[2]) return "pinned";
  if (r[2] && !r[0] && !r[3] && !r[4] && !r[5]) return "roller";
  return "partial";
}

function addGrid(scene: SceneData, m: Model, view: ViewSpec, b: Bounds, size: number) {
  const g = gridOf(m);
  const pad = Math.max(0.06 * size, 0.5);
  const line = (a: Vec3, c: Vec3) => scene.lines.push({ a, b: c, color: COLORS.grid, width: 1 });
  const label = (p: Vec3, text: string) => scene.labels.push({ position: p, text, className: "lbl-grid" });
  const [x0, x1] = [b.min[0] - pad, b.max[0] + pad];
  const [y0, y1] = [b.min[1] - pad, b.max[1] + pad];
  const [z0, z1] = [b.min[2] - pad, b.max[2] + pad];
  const ys = g.y.map((l) => l.coord);
  const flat = (z: number, labels: boolean) => {
    const yLong = ys.length > 1 || view.kind === "plan";
    for (const l of g.x) {
      if (yLong) line([l.coord, y0, z], [l.coord, y1, z]);
      if (labels) label([l.coord, yLong ? y0 - pad * 0.4 : (ys[0] ?? 0), z], l.label);
    }
    for (const l of g.y) {
      line([x0, l.coord, z], [x1, l.coord, z]);
      if (labels && ys.length > 1) label([x0 - pad * 0.4, l.coord, z], l.label);
    }
  };
  switch (view.kind) {
    case "3d":
      g.stories.forEach((s, i) => flat(s.z, i === 0));
      break;
    case "plan":
      flat(view.at, true);
      break;
    case "elev-xz":
    case "elev-yz": {
      const k = view.kind === "elev-xz" ? 0 : 1;
      const lines = view.kind === "elev-xz" ? g.x : g.y;
      const along = (c: number, z: number): Vec3 => (k === 0 ? [c, view.at, z] : [view.at, c, z]);
      const [c0, c1] = k === 0 ? [x0, x1] : [y0, y1];
      for (const l of lines) {
        line(along(l.coord, z0), along(l.coord, z1));
        label(along(l.coord, z1 + pad * 0.4), l.label);
      }
      for (const s of g.stories) {
        line(along(c0, s.z), along(c1, s.z));
        label(along(c1 + pad * 0.9, s.z), s.label);
      }
      break;
    }
  }
}

/** Global vector of a member load direction (local directions use the member's axes). */
function loadVector(dir: LoadDir, axes: [Vec3, Vec3, Vec3]): Vec3 {
  switch (dir) {
    case "X":
      return [1, 0, 0];
    case "Y":
      return [0, 1, 0];
    case "Z":
      return [0, 0, 1];
    case "1":
      return axes[0];
    case "2":
      return axes[1];
    case "3":
      return axes[2];
  }
}

function addLoads(scene: SceneData, m: Model, patternId: Id, view: ViewSpec, size: number, hidNodes: Set<Id>, hidElems: Set<Id>) {
  const c = m.load_patterns.find((x) => x.id === patternId);
  if (!c) return;
  const len = 0.07 * size;
  const color = COLORS.load;
  const U = modelUnits(m);
  // Forces and moments at a point: arrows towards it, double arrows for moments, and the values.
  const pointLoad = (p: Vec3, f: Vec3, mo: Vec3) => {
    const texts: string[] = [];
    for (let k = 0; k < 3; k++) {
      if (f[k] === 0) continue;
      const d: Vec3 = [0, 0, 0];
      d[k] = Math.sign(f[k]);
      scene.arrows.push({ tail: sub(p, scale(d, len)), head: p, color });
      texts.push(`${FORCES[k].toUpperCase()} ${U.fmt(f[k], "force")}`);
    }
    for (let k = 0; k < 3; k++) {
      if (mo[k] === 0) continue;
      const d: Vec3 = [0, 0, 0];
      d[k] = Math.sign(mo[k]);
      scene.arrows.push({ tail: p, head: add(p, scale(d, len)), color, double: true });
      texts.push(`${FORCES[k + 3].toUpperCase()} ${U.fmt(mo[k], "moment")}`);
    }
    if (texts.length) scene.labels.push({ position: sub(p, [len * 0.3, 0, -len * 0.3]), text: texts.join("\n"), className: "lbl-load" });
  };
  for (const l of c.node_loads) {
    const n = nodeOf(m, l.node);
    if (!n || hidNodes.has(n.id) || !inView(pos(n), view)) continue;
    pointLoad(pos(n), [l.fx ?? 0, l.fy ?? 0, l.fz ?? 0], [l.mx ?? 0, l.my ?? 0, l.mz ?? 0]);
  }
  // User wind loads: at their points on the story's level (no joint is there).
  const stories = withStories(m).stories ?? [];
  for (const w of c.wind_loads ?? []) {
    const z = stories.find((s) => s.id === w.story)?.z;
    const p: Vec3 = [w.x, w.y, z ?? 0];
    if (z === undefined || !inView(p, view)) continue;
    pointLoad(p, [w.fx ?? 0, w.fy ?? 0, 0], [0, 0, w.mz ?? 0]);
  }
  for (const l of c.frame_loads) {
    const e = frameOf(m, l.frame);
    const a = e && nodeOf(m, e.nodes[0]);
    const b = e && nodeOf(m, e.nodes[1]);
    if (!e || !a || !b || hidElems.has(e.id) || !inView(pos(a), view) || !inView(pos(b), view)) continue;
    const axes = localAxes(pos(a), pos(b), e.angle ?? 0);
    const along = unit(sub(pos(b), pos(a)));
    const at = (x: number) => add(pos(a), scale(along, x));
    if (l.type === "point") {
      const d = scale(loadVector(l.dir, axes), Math.sign(l.p));
      const head = at(l.at);
      scene.arrows.push({ tail: sub(head, scale(d, len)), head, color });
      scene.labels.push({ position: sub(head, scale(d, len * 1.15)), text: U.fmt(Math.abs(l.p), "force"), className: "lbl-load" });
    } else {
      const total = Math.hypot(...sub(pos(b), pos(a)));
      const from = l.from ?? 0;
      const to = l.to ?? total;
      // Uniform: equal arrows; trapezoidal: arrow lengths follow the load from w1 to w2.
      const [w1, w2] = l.type === "uniform" ? [l.w, l.w] : [l.w1, l.w2];
      const big = Math.max(Math.abs(w1), Math.abs(w2)) || 1;
      const dir = loadVector(l.dir, axes);
      const count = Math.max(2, Math.min(24, Math.round(((to - from) / size) * 14) + 1));
      const tails: Vec3[] = [];
      for (let i = 0; i < count; i++) {
        const s = i / (count - 1);
        const w = w1 + (w2 - w1) * s;
        const head = at(from + (to - from) * s);
        const tail = sub(head, scale(dir, (len * 0.7 * w) / big));
        tails.push(tail);
        if (Math.abs(w) > 1e-12 * big) scene.arrows.push({ tail, head, color });
      }
      for (let i = 1; i < tails.length; i++) scene.lines.push({ a: tails[i - 1], b: tails[i], color, width: 1.5 });
      const label = (x: number, w: number) =>
        scene.labels.push({ position: sub(at(x), scale(dir, (len * 0.85 * w) / big)), text: U.fmt(Math.abs(w), "forcePerLength"), className: "lbl-load" });
      if (l.type === "uniform") label((from + to) / 2, w1);
      else {
        label(from, w1);
        label(to, w2);
      }
    }
  }
}
