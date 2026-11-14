// Hosts the viewport: feeds it scene data from the store and turns its events into edits.

import { useEffect, useMemo, useRef } from "react";

import { loadCase, loadMode, useCursor } from "../actions";
import { t } from "../i18n";
import { BackendError, backend } from "../ipc/backend";
import { plain } from "../ipc/resultCases";
import { addAreaAt } from "../model/areaOps";
import { sameLabel } from "../model/labels";
import { dist, pos } from "../model/geometry";
import { modelUnits } from "../units/display";
import { SAME_POINT, addFrame, addLink, addNode, nodeById, normalizeModel } from "../model/ops";
import type { Results, ShellView, Vec3 } from "../model/types";
import { rampColor } from "../scene/areas";
import { MAX_TEXTS, buildModelScene, modelStyles } from "../scene/buildModelScene";
import { buildResultsScene } from "../scene/buildResultsScene";
import { inView, snapPoints } from "../scene/view";
import { COLORS, setSceneTheme } from "../scene/palette";
import { snapStep, usePrefs, useTheme } from "../state/prefs";
import { selectionKey, useStore } from "../state/store";
import type { PickRef, SceneData } from "../scene/types";
import type { ClickEvent, Viewport } from "../viewport/Viewport";
import { ThreeViewport } from "../viewport/three/ThreeViewport";
import { shellQuantity, useUnits } from "../units/format";

declare global {
  interface Window {
    /** Test hooks: screen position of a model point, the app state and preferences, and loading a model file's JSON. */
    __osab?: {
      project(p: Vec3): { x: number; y: number } | null;
      state: typeof useStore.getState;
      prefs: typeof usePrefs.getState;
      open(raw: unknown, name: string): void;
      /** The current results with every case's values (results on demand fetches them all). */
      allResults(): Promise<Results | null>;
    };
  }
}

/**
 * Ends Draw Area (an area or an opening; Enter, right-click, or a click on the first corner): three or more
 * corners make the area, with its missing joints, in one undoable edit; fewer are dropped.
 */
export function finishArea() {
  const s = useStore.getState();
  const corners = s.areaCorners;
  s.setAreaCorners([]);
  if (s.tool === "area" && corners.length >= 3) {
    s.edit((m) => addAreaAt(m, corners, s.drawProps.areaSection, { opening: !!s.drawProps.opening }).model);
  }
}

/** The picked objects, with those of the same label on every story in All stories mode. */
export function withSameLabel(picks: PickRef[]): PickRef[] {
  const s = useStore.getState();
  if (!s.allStories || !picks.length) return picks;
  const ids = { nodes: [] as string[], frames: [] as string[], links: [] as string[], areas: [] as string[] };
  for (const p of picks) ids[selectionKey[p.kind]].push(p.id);
  const all = sameLabel(s.model, ids);
  const kinds = { nodes: "node", frames: "frame", links: "link", areas: "area" } as const;
  return (Object.keys(kinds) as (keyof typeof kinds)[]).flatMap((k) => all[k].map((id) => ({ kind: kinds[k], id }) as PickRef));
}

/** The model point of a click while drawing or picking: a snap point, a clicked joint, or the point under the mouse. */
function clickPoint(e: ClickEvent): Vec3 | null {
  const s = useStore.getState();
  let point: Vec3 | null = e.snap;
  if (!point && e.pick?.kind === "node") {
    const n = nodeById(s.model, e.pick.id);
    if (n) point = pos(n);
  }
  return point ?? e.point;
}

function onClick(e: ClickEvent) {
  const s = useStore.getState();
  if (s.pick) {
    // Points for a dialog (Move, Replicate).
    const p = clickPoint(e);
    if (p) s.addPickPoint(p);
    else s.notify(t("vp.click3d"));
    return;
  }
  if (s.tool === "select") {
    if (e.pick) {
      // Clicking an object adds it to the selection (or removes it again).
      s.select(withSameLabel([e.pick]), "toggle");
      if ((e.pick.kind === "frame" || e.pick.kind === "link") && s.results && s.display.mode === "forces") s.setMemberPanel(e.pick.id);
    }
    return;
  }
  const point = clickPoint(e);
  if (!point) {
    s.notify(t("vp.click3d"));
    return;
  }
  if (s.tool === "measure") {
    // Measure: the first click sets the start, the second gives the distance (the model is not changed).
    const a = s.measureStart;
    if (!a) {
      s.setMeasureStart(point);
      s.notify(t("measure.second"));
      return;
    }
    const U = modelUnits(s.model);
    const f = (v: number) => U.fmt(v, "distance");
    s.notify(t("measure.result", { d: f(dist(a, point)), dx: f(point[0] - a[0]), dy: f(point[1] - a[1]), dz: f(point[2] - a[2]), unit: U.label("distance") }));
    s.setMeasureStart(null);
    return;
  }
  if (s.tool === "joint") {
    s.edit((m) => addNode(m, point, { free: true }).model);
    return;
  }
  if (s.tool === "area") {
    // Collect corners until Enter, right-click or a click on the first corner.
    const corners = s.areaCorners;
    if (corners.length >= 3 && dist(corners[0], point) <= SAME_POINT) return finishArea();
    if (corners.some((c) => dist(c, point) <= SAME_POINT)) return;
    s.setAreaCorners([...corners, point]);
    return;
  }
  // Draw Frame / Draw Rigid Link: each click adds a member (or link) from the previous joint, until Esc
  // or right-click.
  let nodeId = "";
  const ok = s.edit((m) => {
    const r = addNode(m, point);
    nodeId = r.id;
    if (s.chainStart === null || s.chainStart === r.id) return r.model;
    if (s.tool === "link") return addLink(r.model, s.chainStart, r.id).model;
    return addFrame(r.model, s.chainStart, r.id, s.drawProps).model;
  });
  if (ok) useStore.getState().setChainStart(nodeId);
}

export function ViewportPanel() {
  const host = useRef<HTMLDivElement>(null);
  const vp = useRef<Viewport | null>(null);
  const model = useStore((s) => s.model);
  const selection = useStore((s) => s.selection);
  const highlight = useStore((s) => s.highlight);
  const view = useStore((s) => s.view);
  const display = useStore((s) => s.display);
  const results = useStore((s) => s.results);
  const tool = useStore((s) => s.tool);
  const chainStart = useStore((s) => s.chainStart);
  const areaCorners = useStore((s) => s.areaCorners);
  const measureStart = useStore((s) => s.measureStart);
  const hidden = useStore((s) => s.hidden);
  const meshView = useStore((s) => s.meshView);
  const pick = useStore((s) => s.pick);
  const step = usePrefs(snapStep);
  const theme = useTheme();
  // Scene colours follow the theme; the scenes below depend on `theme` so they rebuild.
  setSceneTheme(theme);

  useEffect(() => {
    const v = new ThreeViewport(host.current!);
    vp.current = v;
    const offs = [
      v.on("click", onClick),
      v.on("box", (e) => {
        if (useStore.getState().tool === "select") useStore.getState().select(withSameLabel(e.picks), "add");
      }),
      v.on("hover", (p) => useStore.getState().setHover(p)),
      v.on("cursor", (p, snapped) => useCursor.setState({ p, snapped })),
      v.on("finish", () => {
        useStore.getState().setChainStart(null);
        useStore.getState().setMeasureStart(null);
        finishArea();
      }),
    ];
    window.__osab = {
      project: (p) => v.project(p),
      state: useStore.getState,
      prefs: usePrefs.getState,
      open: (raw, name) => useStore.getState().load(normalizeModel(raw), name, null),
      allResults: async () => {
        const r = useStore.getState().results;
        if (!r) return null;
        const modal = r.modal && { ...r.modal, modes: await Promise.all(r.modal.modes.map(async (m, i) => (m.pending ? { ...m, shape: await backend.modeShape(i), pending: false } : m))) };
        return plain({ ...r, ...(modal ? { modal } : {}), cases: await Promise.all(r.cases.map((c, i) => (c.pending ? backend.resultCase(i) : c))) });
      },
    };
    const fit = () => v.zoomToFit();
    window.addEventListener("osab-fit", fit);
    return () => {
      window.removeEventListener("osab-fit", fit);
      offs.forEach((f) => f());
      v.dispose();
      vp.current = null;
    };
  }, []);

  // View > Show Analysis Mesh: the run's mesh, or the mesh step's alone before a run.
  const options = display.options;
  const mesh = options.mesh ? (results?.mesh ?? meshView) : null;
  useEffect(() => {
    if (!options.mesh || results || meshView) return;
    const m = model;
    backend.mesh(m).then(
      (v) => useStore.getState().model === m && useStore.getState().setMeshView(v.mesh),
      (e) => useStore.getState().notify(e instanceof BackendError ? e.report.message : String(e), "error"),
    );
  }, [options.mesh, results, meshView, model]);

  const showResults = results !== null && display.mode !== "model";
  // Results on demand: the case shown is fetched when its values are not here yet.
  useEffect(() => {
    void loadCase(display.caseId);
  }, [results, display.caseId]);
  // The mode shown: its shape is fetched when it is not here yet.
  useEffect(() => {
    if (showResults && display.mode === "modes") void loadMode(display.modeNumber);
  }, [results, showResults, display.mode, display.modeNumber]);
  // Display > Mode Shape, animated: the shape swings between +1 and -1 times its scale (the viewport moves
  // the drawn shape; nothing is built again).
  const animating = showResults && display.mode === "modes" && display.animate;
  useEffect(() => {
    if (!animating) {
      vp.current?.setSwing(1);
      return;
    }
    let frame = 0;
    const start = performance.now();
    const step = (now: number) => {
      vp.current?.setSwing(Math.cos((2 * Math.PI * (now - start)) / 1600));
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => {
      cancelAnimationFrame(frame);
      vp.current?.setSwing(1);
    };
  }, [animating]);
  // The model is drawn without the selection; the selected and highlighted objects only change their looks.
  const sceneOptions = { view, loadsPattern: display.loadsPattern, dimmed: showResults, hidden, options, mesh };
  const base = useMemo(
    () => buildModelScene(model, { ...sceneOptions, selection: {}, highlight: {} }),
    [model, view, display.loadsPattern, showResults, hidden, options, mesh, theme],
  );
  const styles = useMemo(
    () => modelStyles(model, { ...sceneOptions, selection, highlight }),
    [model, view, selection, highlight, display.loadsPattern, showResults, hidden, options, mesh, theme],
  );
  // Too many object texts for one view: say how many were left out.
  const leftOut = base.textsLeftOut;
  const capped = leftOut > 0;
  useEffect(() => {
    if (capped) useStore.getState().notify(t("vo.capped", { n: leftOut, max: MAX_TEXTS }));
  }, [capped]);
  // The results scene: its legend, and its items handed to the viewport once (it keeps only what it draws;
  // asked again, e.g. by a new viewport, they are built again).
  const overlay = useMemo(() => {
    if (!showResults || !results) return null;
    const build = () =>
      buildResultsScene(model, results, {
        view,
        mode: display.mode,
        caseId: display.caseId,
        component: display.component,
        scale: display.scale,
        modeNumber: display.modeNumber,
        showValues: display.showValues,
        tensionSide: display.tensionSide,
        size: base.size,
        hidden,
        shellComponent: display.shellComponent,
        averaged: display.averaged,
        hiddenTypes: options.hiddenTypes,
      });
    const first = build();
    let built: SceneData | null = first.scene;
    const scene = () => {
      const d = built ?? build().scene;
      built = null;
      return d;
    };
    return { legend: first.legend, maxValue: first.maxValue, scene };
  }, [showResults, results, model, view, display.mode, display.caseId, display.component, display.scale, display.modeNumber, display.showValues, display.tensionSide, display.shellComponent, display.averaged, options.hiddenTypes, base.size, hidden, theme]);

  useEffect(() => {
    vp.current?.setColors(COLORS.background, COLORS.hover);
  }, [theme]);

  useEffect(() => {
    vp.current?.setScene(base.scene, overlay?.scene ?? null);
  }, [base, overlay]);

  useEffect(() => {
    vp.current?.setStyles(styles);
  }, [styles]);

  // Perspective or parallel 3D view (before the view is set, so a new 3D view starts with the right camera).
  useEffect(() => {
    vp.current?.setParallel(options.parallel);
  }, [options.parallel]);

  // Fit when the view changes or a model is loaded; otherwise keep the user's zoom.
  const loadId = useStore((s) => s.loadId);
  const lastFit = useRef("");
  useEffect(() => {
    const key = `${view.kind}:${view.at}:${loadId}`;
    vp.current?.setView(view, base.bounds, key !== lastFit.current);
    lastFit.current = key;
  }, [view, base.bounds, loadId]);

  useEffect(() => {
    const drawing = tool !== "select" || pick !== null;
    const hiddenNodes = new Set(hidden.nodes);
    const joints = model.nodes.filter((n) => !hiddenNodes.has(n.id)).map(pos);
    const snaps = drawing ? [...snapPoints(model, view), ...joints.filter((p) => inView(p, view))] : [];
    vp.current?.setSnapPoints(snaps);
    const start = chainStart ? nodeById(model, chainStart) : undefined;
    vp.current?.setInteraction({
      pick: drawing ? "nodes" : "all",
      snap: drawing,
      step,
      box: !drawing,
      rubber: pick ? pick.points : tool === "area" ? areaCorners : tool === "measure" ? (measureStart ? [measureStart] : []) : (tool === "frame" || tool === "link") && start ? [pos(start)] : [],
      rubberClosed: !pick && tool === "area",
    });
  }, [tool, model, view, chainStart, areaCorners, measureStart, step, hidden, pick]);

  const legend = overlay?.legend;
  return (
    <div className={`viewport tool-${tool}`} ref={host} data-testid="viewport">
      {legend && <ContourLegend min={legend.min} max={legend.max} comp={display.shellComponent} />}
    </div>
  );
}

/** Colour scale of the shell contours, high at the top. */
function ContourLegend({ min, max, comp }: { min: number; max: number; comp: ShellView }) {
  const U = useUnits();
  const q = shellQuantity(comp);
  const steps = 8;
  const range = Math.max(Math.abs(min), Math.abs(max));
  const rows = Array.from({ length: steps + 1 }, (_, i) => {
    const f = 1 - i / steps;
    const [r, g, b] = rampColor(f);
    return { value: min + (max - min) * f, color: `rgb(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)})` };
  });
  return (
    <div className="contour-legend" data-testid="contour-legend">
      <div className="legend-unit">{U.label(q)}</div>
      {rows.map((r, i) => (
        <div key={i} className="legend-row">
          <span className="swatch" style={{ background: r.color }} />
          <span>{U.fmt(r.value, q, range)}</span>
        </div>
      ))}
    </div>
  );
}

/** Fits the whole model in the view (View > Zoom to Fit). */
export function zoomToFit() {
  window.dispatchEvent(new Event("osab-fit"));
}
