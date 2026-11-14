// App state: the model (with undo), selection, tools, view, results and open dialogs.
// Model edits go through `edit`, which records undo, marks the file changed, brings the objects' labels
// up to date and clears results.

import { create } from "zustand";

import { t } from "../i18n";
import { withLabels } from "../model/labels";
import { EditError, gridOf, newModel, setNotRun } from "../model/ops";
import { filteredOut } from "../scene/mesh";
import type { PickRef } from "../scene/types";
import { defaultViewOptions, viewOptionsOf, type ViewOptions } from "../scene/viewOptions";
import type { CaseResult, Component, FrameKind, Id, MeshInfo, ModeShape, Model, Results, ShellView, Vec3 } from "../model/types";
import { LOADED_CASES, LOADED_MODES, headOf, pendingCase } from "../ipc/resultCases";
import { unitSystemOf, type UnitSystem } from "../units/units";

export type { PickRef };

/** The model with its display units set (undo and redo keep the chosen units). */
function withUnits(model: Model, system: UnitSystem): Model {
  if (unitSystemOf(model.gui?.units) === system) return model;
  return { ...model, gui: { ...model.gui, units: system } };
}

export type Tool = "select" | "joint" | "frame" | "link" | "area" | "measure";
export type ViewKind = "3d" | "plan" | "elev-xz" | "elev-yz";

/** Which view: 3D, plan at z = at, elevation X-Z at y = at, or elevation Y-Z at x = at. */
export interface ViewSpec {
  kind: ViewKind;
  at: number;
}

export type DisplayMode = "model" | "deformed" | "forces" | "shells" | "reactions" | "modes";

export interface Display {
  mode: DisplayMode;
  /** Load case shown in results views. */
  caseId: Id | null;
  component: Component;
  /** Multiplies the automatic scale of deformed shapes and diagrams. */
  scale: number;
  showValues: boolean;
  /** Draw moment diagrams on the tension side (the default). */
  tensionSide: boolean;
  /** Load case whose loads are drawn on the model (null = none). */
  loadsPattern: Id | null;
  /** View > Set Display Options (Ctrl+W): texts, symbols, object types and effects (saved in the model file). */
  options: ViewOptions;
  /** Shell force shown as contours (Display > Shell Forces). */
  shellComponent: ShellView;
  /** Contours averaged at joints (smooth) or per element corner (not averaged). */
  averaged: boolean;
  /** Display > Mode Shape: the mode shown (1-based) and whether it moves. */
  modeNumber: number;
  animate: boolean;
}

/** Joints, members, links and areas (by id). */
export interface Selection {
  nodes: Id[];
  frames: Id[];
  links: Id[];
  areas: Id[];
}

export const SELECTION_KEYS = ["nodes", "frames", "links", "areas"] as const;
export type SelectionKey = (typeof SELECTION_KEYS)[number];
/** Selection list of each pick kind. */
export const selectionKey: Record<PickRef["kind"], SelectionKey> = { node: "nodes", frame: "frames", link: "links", area: "areas" };

/** Builds a selection with `f` for each kind. */
const mapSelection = (f: (key: SelectionKey) => Id[]): Selection =>
  Object.fromEntries(SELECTION_KEYS.map((k) => [k, f(k)])) as unknown as Selection;

/** A full selection from a partial one (missing kinds = none). */
export const fullSelection = (o: Partial<Selection>): Selection => mapSelection((k) => o[k] ?? []);

/** Keeps the ids that `keep` accepts. */
function filterSelection(sel: Selection, keep: (key: SelectionKey, id: Id) => boolean): Selection {
  return mapSelection((k) => sel[k].filter((id) => keep(k, id)));
}

export const selectionCount = (o: Selection) => SELECTION_KEYS.reduce((n, k) => n + o[k].length, 0);

export type DialogName =
  | "new"
  | "materials"
  | "sections"
  | "patterns"
  | "cases"
  | "combos"
  | "restraints"
  | "springs"
  | "jointLoads"
  | "frameLoads"
  | "frameAssign"
  | "frameModifiers"
  | "frameReleases"
  | "frameOffsets"
  | "frameInsertion"
  | "areaInsertion"
  | "diaphragms"
  | "assignDiaphragm"
  | "jointMasses"
  | "massSource"
  | "pDelta"
  | "spectrumFunctions"
  | "areaModifiers"
  | "areaSections"
  | "areaAssign"
  | "areaLoads"
  | "divideAreas"
  | "jointCoords"
  | "grid"
  | "storyData"
  | "tables"
  | "about"
  | "examples"
  | "welcome"
  | "check"
  | "preferences"
  | "analysisOptions"
  | "runCases"
  | "groups"
  | "pierLabels"
  | "spandrelLabels"
  | "meshSettings"
  | "angleLimits"
  | "areaMesh"
  | "floorMeshing"
  | "specialJoint"
  | "lineSpringProps"
  | "areaSpringProps"
  | "lineSprings"
  | "areaSprings"
  | "move"
  | "replicate"
  | "selectType"
  | "showLoads"
  | "displayOptions";

/** Points picked in the view for an open dialog (Move, Replicate): the dialog waits, hidden, until done. */
export interface PointPick {
  need: 1 | 2;
  points: Vec3[];
  done(points: Vec3[]): void;
}

export interface Notice {
  text: string;
  level: "info" | "error";
}

export interface DrawProps {
  type: FrameKind;
  material: Id;
  section: Id;
  /** Area section of areas being drawn. */
  areaSection: Id;
  /** Draw Area draws openings (the "Opening" choice of the area section list). */
  opening?: boolean;
}

interface State {
  model: Model;
  filePath: string | null;
  fileName: string;
  dirty: boolean;
  past: Model[];
  future: Model[];
  selection: Selection;
  hover: PickRef | null;
  tool: Tool;
  drawProps: DrawProps;
  /** First joint of the member being drawn (Draw Frame chains from joint to joint). */
  chainStart: Id | null;
  /** Corners clicked so far while drawing an area. */
  areaCorners: Vec3[];
  /** Measure: the first point of the distance being measured. */
  measureStart: Vec3 | null;
  /** Points being picked for a dialog (it is hidden meanwhile); null when not picking. */
  pick: PointPick | null;
  view: ViewSpec;
  results: Results | null;
  /** Goes up with every new set of results (a run, or results opened with the model). */
  resultsRun: number;
  /** Indices of the cases whose values are loaded, most recent last (results on demand). */
  loadedCases: number[];
  /** Indices of the modes whose shapes are loaded, most recent last. */
  loadedModes: number[];
  /** The analysis mesh of the current model (View > Show Analysis Mesh), fetched on demand. */
  meshView: MeshInfo | null;
  running: boolean;
  display: Display;
  dialog: DialogName | null;
  notice: Notice | null;
  /** Solver notes from the last run. */
  messages: string[];
  /** Member shown in the member diagram panel. */
  memberPanel: Id | null;
  /** Objects to point out, e.g. the joint named in an "unstable" error. */
  highlight: Selection;
  /** Goes up each time a model is loaded or created (the view then fits it). */
  loadId: number;
  /** Objects hidden in the views (not saved in the model file). */
  hidden: Selection;
  /** Left panel tab: the model explorer tree or the properties of the current tool/selection. */
  leftTab: "model" | "properties";
  /** One story / All stories: with all stories, a click in the views selects the objects of the same label on every story. */
  allStories: boolean;
  setAllStories(on: boolean): void;
  /** Item a Define dialog opens on (e.g. the material double-clicked in the explorer). */
  dialogItem: Id | null;

  edit(fn: (m: Model) => Model, keepSelection?: boolean): boolean;
  /** Display units: saved in the model file, but not an edit (no undo, results stay). */
  setUnitSystem(system: UnitSystem): void;
  /** The cases set not to run (Analyze > Load Cases to Run): saved in the model file, but not an edit (no undo, results stay). */
  setRunFlags(notRun: Id[]): boolean;
  /** Drops the results of the cases at `indices` (and with `modal` the modal results); the rest stay. */
  dropResults(indices: number[], modal: boolean): void;
  undo(): void;
  redo(): void;
  load(model: Model, fileName: string, filePath: string | null): void;
  markSaved(fileName: string, filePath: string | null): void;
  select(picks: PickRef[], mode: "replace" | "add" | "toggle"): void;
  clearSelection(): void;
  setTool(tool: Tool): void;
  setMeasureStart(p: Vec3 | null): void;
  setView(view: ViewSpec): void;
  setDisplay(patch: Partial<Display>): void;
  setResults(results: Results | null): void;
  /** A case's values, fetched for results on demand; the oldest loaded case beyond a few is dropped again. */
  putCase(run: number, index: number, values: CaseResult): void;
  /** A mode's shape, fetched when a view shows the mode; the oldest loaded shape beyond a few is dropped again. */
  putModeShape(run: number, index: number, shape: ModeShape): void;
  setMeshView(mesh: MeshInfo | null): void;
  setDrawProps(patch: Partial<DrawProps>): void;
  setHover(h: PickRef | null): void;
  setChainStart(id: Id | null): void;
  setAreaCorners(points: Vec3[]): void;
  /** Starts picking `need` points in the view for the open dialog; `done` gets them. */
  startPick(need: 1 | 2, done: (points: Vec3[]) => void): void;
  addPickPoint(p: Vec3): void;
  cancelPick(): void;
  openDialog(d: DialogName | null, item?: Id | null): void;
  /** Hides objects (they leave the selection too). */
  hide(objects: Partial<Selection>): void;
  show(objects: Partial<Selection>): void;
  showAll(): void;
  /** Hides every joint, member and link except these. */
  showOnly(objects: Partial<Selection>): void;
  deselect(objects: Partial<Selection>): void;
  setLeftTab(tab: "model" | "properties"): void;
  notify(text: string, level?: Notice["level"]): void;
  setRunning(r: boolean): void;
  setMemberPanel(id: Id | null): void;
  setHighlight(h: Partial<Selection>): void;
}

export const UNDO_LIMIT = 200;

export function startModel(): Model {
  return newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
}

/** The view a model opens in: the X-Z elevation for plane-frame models, 3D otherwise. */
export function defaultView(m: Model): ViewSpec {
  if (m.dimension === "2D") return { kind: "elev-xz", at: m.nodes[0]?.y ?? gridOf(m).y[0]?.coord ?? 0 };
  if (m.dimension === "grid") return { kind: "plan", at: m.nodes[0]?.z ?? gridOf(m).stories.at(-1)?.z ?? 0 };
  return { kind: "3d", at: 0 };
}

function defaultDrawProps(m: Model, current?: DrawProps): DrawProps {
  // "" (None: null objects) stays chosen once picked.
  const keep = <T extends { id: Id }>(list: T[], id: Id | undefined) => (id === "" || list.some((x) => x.id === id) ? id! : list[0]?.id ?? "");
  return {
    type: current?.type ?? "frame",
    material: keep(m.materials, current?.material),
    section: keep(m.sections, current?.section),
    areaSection: keep(m.area_sections, current?.areaSection),
    ...(current?.opening ? { opening: true } : {}),
  };
}

const emptySelection: Selection = fullSelection({});

export const useStore = create<State>()((set, get) => {
  const model = startModel();
  return {
    model,
    filePath: null,
    fileName: "Untitled.osab",
    dirty: false,
    past: [],
    future: [],
    selection: emptySelection,
    hover: null,
    tool: "select",
    drawProps: defaultDrawProps(model),
    chainStart: null,
    areaCorners: [],
    measureStart: null,
    pick: null,
    view: defaultView(model),
    results: null,
    resultsRun: 0,
    loadedCases: [],
    loadedModes: [],
    meshView: null,
    running: false,
    display: {
      mode: "model",
      caseId: null,
      component: "M3",
      scale: 1,
      showValues: true,
      tensionSide: true,
      loadsPattern: null,
      options: defaultViewOptions(),
      shellComponent: "M11",
      averaged: false,
      modeNumber: 1,
      animate: true,
    },
    dialog: null,
    notice: null,
    messages: [],
    memberPanel: null,
    highlight: emptySelection,
    loadId: 0,
    hidden: emptySelection,
    leftTab: "model",
    allStories: false,
    dialogItem: null,

    edit(fn, keepSelection = true) {
      const s = get();
      let next: Model;
      try {
        next = fn(s.model);
      } catch (e) {
        if (e instanceof EditError) {
          set({ notice: { text: e.message, level: "error" } });
          return false;
        }
        throw e;
      }
      if (next === s.model) return true;
      next = withLabels(next);
      const nodes = new Set(next.nodes.map((n) => n.id));
      const ids: Record<SelectionKey, Set<Id>> = {
        nodes,
        frames: new Set(next.frames.map((e) => e.id)),
        links: new Set(next.links.map((l) => l.id)),
        areas: new Set(next.areas.map((a) => a.id)),
      };
      const patterns = new Set(next.load_patterns.map((c) => c.id));
      const display = { ...s.display };
      if (display.loadsPattern !== null && !patterns.has(display.loadsPattern)) display.loadsPattern = null;
      const hadResults = s.results !== null;
      if (hadResults) display.mode = "model";
      set({
        model: next,
        dirty: true,
        past: [...s.past, s.model].slice(-UNDO_LIMIT),
        future: [],
        selection: keepSelection ? filterSelection(s.selection, (k, id) => ids[k].has(id)) : emptySelection,
        hidden: filterSelection(s.hidden, (k, id) => ids[k].has(id)),
        chainStart: s.chainStart && nodes.has(s.chainStart) ? s.chainStart : null,
        drawProps: defaultDrawProps(next, s.drawProps),
        results: null,
        meshView: null,
        memberPanel: null,
        highlight: emptySelection,
        display,
        notice: hadResults ? { text: t("msg.resultsCleared"), level: "info" } : s.notice,
      });
      return true;
    },

    setUnitSystem(system) {
      const s = get();
      if (unitSystemOf(s.model.gui?.units) === system) return;
      set({ model: withUnits(s.model, system), dirty: true });
    },

    undo() {
      const s = get();
      const prev = s.past.at(-1);
      if (!prev) return;
      set({
        model: withUnits(prev, unitSystemOf(s.model.gui?.units)),
        past: s.past.slice(0, -1),
        future: [s.model, ...s.future],
        dirty: true,
        results: null,
        meshView: null,
        chainStart: null,
        areaCorners: [],
        measureStart: null,
        memberPanel: null,
        drawProps: defaultDrawProps(prev, s.drawProps),
        display: { ...s.display, mode: "model" },
        selection: emptySelection,
      });
    },

    redo() {
      const s = get();
      const next = s.future[0];
      if (!next) return;
      set({
        model: withUnits(next, unitSystemOf(s.model.gui?.units)),
        past: [...s.past, s.model],
        future: s.future.slice(1),
        dirty: true,
        results: null,
        meshView: null,
        chainStart: null,
        areaCorners: [],
        measureStart: null,
        memberPanel: null,
        drawProps: defaultDrawProps(next, s.drawProps),
        display: { ...s.display, mode: "model" },
        selection: emptySelection,
      });
    },

    load(opened, fileName, filePath) {
      const model = withLabels(opened);
      set({
        model,
        fileName,
        filePath,
        dirty: false,
        past: [],
        future: [],
        selection: emptySelection,
        chainStart: null,
        areaCorners: [],
        measureStart: null,
        tool: "select",
        view: defaultView(model),
        results: null,
        meshView: null,
        messages: [],
        memberPanel: null,
        highlight: emptySelection,
        drawProps: defaultDrawProps(model),
        display: { ...get().display, mode: "model", caseId: null, loadsPattern: null, options: viewOptionsOf(model.gui?.viewOptions) },
        loadId: get().loadId + 1,
        hidden: emptySelection,
      });
    },

    markSaved(fileName, filePath) {
      set({ fileName, filePath, dirty: false });
    },

    select(picks, mode) {
      const s = get();
      const cur = mode === "replace" ? emptySelection : s.selection;
      const sets = Object.fromEntries(SELECTION_KEYS.map((k) => [k, new Set(cur[k])])) as Record<SelectionKey, Set<Id>>;
      // Hidden objects, and objects of a type not drawn (View > Set Display Options), cannot be selected.
      const types = s.display.options.hiddenTypes;
      const out = filteredOut(s.model, types);
      const typeOut: Record<SelectionKey, (id: Id) => boolean> = {
        nodes: () => types.includes("joint"),
        links: () => types.includes("link"),
        frames: (id) => out.frames.has(id),
        areas: (id) => out.areas.has(id),
      };
      for (const p of picks) {
        const key = selectionKey[p.kind];
        if (s.hidden[key].includes(p.id) || typeOut[key](p.id)) continue;
        if (mode === "toggle" && sets[key].has(p.id)) sets[key].delete(p.id);
        else sets[key].add(p.id);
      }
      set({ selection: mapSelection((k) => [...sets[k]]) });
    },

    setAllStories(on) {
      set({ allStories: on });
    },

    clearSelection() {
      set({ selection: emptySelection });
    },

    setTool(tool) {
      // Drawing settings live on the Properties tab.
      set({ tool, chainStart: null, areaCorners: [], measureStart: null, ...(tool !== "select" && tool !== "measure" ? { leftTab: "properties" as const } : {}) });
    },

    setView(view) {
      set({ view });
    },

    setRunFlags(notRun) {
      const s = get();
      try {
        const model = setNotRun(s.model, notRun);
        if (JSON.stringify(model.not_run ?? []) !== JSON.stringify(s.model.not_run ?? [])) set({ model, dirty: true });
        return true;
      } catch (e) {
        if (!(e instanceof EditError)) throw e;
        set({ notice: { text: e.message, level: "error" } });
        return false;
      }
    },

    dropResults(indices, modal) {
      const s = get();
      if (!s.results) return;
      const cases = s.results.cases.filter((_, i) => !indices.includes(i));
      const results: Results = { ...s.results, cases };
      if (modal) delete results.modal;
      if (!cases.length && !results.modal) {
        set({ results: null, resultsRun: s.resultsRun + 1, loadedCases: [], loadedModes: [], display: { ...s.display, mode: "model" } });
        return;
      }
      const shift = (i: number) => i - indices.filter((j) => j < i).length;
      const key = (c: Results["cases"][number]) => (c.step ? `${c.id}|${c.step}` : c.id);
      const caseId = cases.some((c) => key(c) === s.display.caseId) ? s.display.caseId : cases[0] ? key(cases[0]) : null;
      set({
        results,
        resultsRun: s.resultsRun + 1,
        loadedCases: s.loadedCases.filter((i) => !indices.includes(i)).map(shift),
        ...(modal ? { loadedModes: [] } : {}),
        display: { ...s.display, caseId, ...(cases.length ? {} : { mode: "model" as const }) },
      });
    },

    setDisplay(patch) {
      set({ display: { ...get().display, ...patch } });
    },

    putCase(run, index, values) {
      const s = get();
      if (!s.results || s.resultsRun !== run || !s.results.cases[index]) return;
      const cases = [...s.results.cases];
      cases[index] = values;
      const loaded = [...s.loadedCases.filter((i) => i !== index), index];
      while (loaded.length > LOADED_CASES) {
        const old = loaded.shift()!;
        const c = cases[old];
        cases[old] = pendingCase(headOf(c));
      }
      set({ results: { ...s.results, cases }, loadedCases: loaded });
    },

    putModeShape(run, index, shape) {
      const s = get();
      const modal = s.results?.modal;
      if (!modal || s.resultsRun !== run || !modal.modes[index]) return;
      const modes = [...modal.modes];
      modes[index] = { ...modes[index], shape, pending: false };
      const loaded = [...s.loadedModes.filter((i) => i !== index), index];
      while (loaded.length > LOADED_MODES) {
        const old = loaded.shift()!;
        modes[old] = { ...modes[old], shape: [], pending: true };
      }
      set({ results: { ...s.results!, modal: { ...modal, modes } }, loadedModes: loaded });
    },

    setResults(results) {
      const s = get();
      const key = (c: Results["cases"][number]) => (c.step ? `${c.id}|${c.step}` : c.id);
      const caseId = results && !results.cases.some((c) => key(c) === s.display.caseId) ? (results.cases[0] ? key(results.cases[0]) : null) : s.display.caseId;
      set({ results, resultsRun: s.resultsRun + 1, loadedCases: [], loadedModes: [], messages: results?.notes ?? [], display: { ...s.display, caseId } });
    },

    setMeshView(meshView) {
      set({ meshView });
    },

    setDrawProps(patch) {
      set({ drawProps: { ...get().drawProps, ...patch } });
    },

    setHover(hover) {
      const h = get().hover;
      if (h?.kind === hover?.kind && h?.id === hover?.id) return;
      set({ hover });
    },

    setChainStart(chainStart) {
      set({ chainStart });
    },

    setMeasureStart(measureStart) {
      set({ measureStart });
    },

    setAreaCorners(areaCorners) {
      set({ areaCorners });
    },

    startPick(need, done) {
      set({ pick: { need, points: [], done }, notice: { text: t(need === 1 ? "pick.one" : "pick.first"), level: "info" } });
    },

    addPickPoint(p) {
      const pick = get().pick;
      if (!pick) return;
      const points = [...pick.points, p];
      if (points.length < pick.need) {
        set({ pick: { ...pick, points }, notice: { text: t("pick.second"), level: "info" } });
        return;
      }
      set({ pick: null, notice: null });
      pick.done(points);
    },

    cancelPick() {
      set({ pick: null, notice: null });
    },

    openDialog(dialog, item = null) {
      set({ dialog, dialogItem: item, pick: null });
    },

    hide(objects) {
      const s = get();
      const o = fullSelection(objects);
      const sets = Object.fromEntries(SELECTION_KEYS.map((k) => [k, new Set([...s.hidden[k], ...o[k]])])) as Record<SelectionKey, Set<Id>>;
      set({
        hidden: mapSelection((k) => [...sets[k]]),
        selection: filterSelection(s.selection, (k, id) => !sets[k].has(id)),
        memberPanel: s.memberPanel && (sets.frames.has(s.memberPanel) || sets.links.has(s.memberPanel)) ? null : s.memberPanel,
      });
    },

    show(objects) {
      const s = get();
      const o = fullSelection(objects);
      set({ hidden: filterSelection(s.hidden, (k, id) => !o[k].includes(id)) });
    },

    showAll() {
      set({ hidden: emptySelection });
    },

    showOnly(objects) {
      const s = get();
      const o = fullSelection(objects);
      const keepNodes = new Set(o.nodes);
      const keepFrames = new Set(o.frames);
      const keepLinks = new Set(o.links);
      const keepAreas = new Set(o.areas);
      // Keep the joints of the members, links and areas shown, so they stay connected on screen.
      for (const e of s.model.frames) if (keepFrames.has(e.id)) e.nodes.forEach((n) => keepNodes.add(n));
      for (const l of s.model.links) if (keepLinks.has(l.id)) l.nodes.forEach((n) => keepNodes.add(n));
      for (const a of s.model.areas) if (keepAreas.has(a.id)) a.nodes.forEach((n) => keepNodes.add(n));
      get().showAll();
      get().hide({
        nodes: s.model.nodes.map((n) => n.id).filter((n) => !keepNodes.has(n)),
        frames: s.model.frames.map((e) => e.id).filter((e) => !keepFrames.has(e)),
        links: s.model.links.map((l) => l.id).filter((l) => !keepLinks.has(l)),
        areas: s.model.areas.map((a) => a.id).filter((a) => !keepAreas.has(a)),
      });
    },

    deselect(objects) {
      const s = get();
      const o = fullSelection(objects);
      set({ selection: filterSelection(s.selection, (k, id) => !o[k].includes(id)) });
    },

    setLeftTab(leftTab) {
      set({ leftTab });
    },

    notify(text, level = "info") {
      set({ notice: { text, level } });
    },

    setRunning(running) {
      set({ running });
    },

    setMemberPanel(memberPanel) {
      set({ memberPanel });
    },

    setHighlight(highlight) {
      set({ highlight: fullSelection(highlight) });
    },
  };
});
