// User actions that need the backend (solver, files) or touch several parts of the state.

import { t } from "./i18n";
import { BackendError, backend } from "./ipc/backend";
import { checkModel } from "./model/check";
import { EXAMPLES, exampleModel } from "./model/examples";
import { deleteObjects, modelForRun, moveNode, newModel, resultsToDelete, withStories, type NewModelOptions } from "./model/ops";
import { storeMeshes, userMeshOf } from "./model/storedMesh";
import type { Id, Model, SolverError, UserMesh, Vec3 } from "./model/types";
import { caseIndex } from "./scene/buildResultsScene";
import { inView } from "./scene/view";
import { defaultViewOptions, type ViewOptions } from "./scene/viewOptions";
import { create } from "zustand";
import { selectionCount, useStore } from "./state/store";

/** Mouse position in model coordinates, for the status bar (kept apart to avoid re-rendering the app). */
export const useCursor = create<{ p: Vec3 | null; snapped: boolean }>(() => ({ p: null, snapped: false }));

const get = () => useStore.getState();

export async function confirmDiscard(): Promise<boolean> {
  if (!get().dirty) return true;
  return backend.confirm(t("msg.unsaved", { name: get().fileName }), t("msg.unsavedTitle"));
}

export async function createModel(o: NewModelOptions) {
  if (!(await confirmDiscard())) return false;
  get().load(newModel(o), "Untitled.osab", null);
  return true;
}

export async function openFile(path?: string) {
  if (!(await confirmDiscard())) return;
  try {
    const f = await backend.openModel(path);
    if (!f) return;
    get().load(f.model, f.name, f.path);
    if (f.results) get().setResults(f.results);
    get().notify(t(f.results ? "msg.openedWithResults" : "msg.opened", { name: f.name }));
  } catch (e) {
    get().notify(e instanceof BackendError ? e.message : String(e), "error");
  }
}

/** File > Open Example: loads one of the bundled benchmark models. */
export async function openExample(id: string) {
  const ex = EXAMPLES.find((e) => e.id === id);
  if (!ex || !(await confirmDiscard())) return;
  get().load(exampleModel(ex), ex.file, null);
  get().openDialog(null);
  get().notify(t("msg.opened", { name: ex.file }));
}

const fetching = new Set<string>();

/**
 * Results on demand: fetches the values of the case with this key when they are not here yet (a view or
 * table is about to show it). Nothing to do when they are.
 */
export async function loadCase(key: string | null) {
  const s = get();
  const r = s.results;
  if (!r) return;
  const i = caseIndex(r, key);
  if (i < 0 || !r.cases[i].pending) return;
  const run = s.resultsRun;
  const tag = `${run}:${i}`;
  if (fetching.has(tag)) return;
  fetching.add(tag);
  try {
    get().putCase(run, i, await backend.resultCase(i));
  } catch {
    if (get().resultsRun === run) get().notify(t("msg.caseGone"), "error");
  } finally {
    fetching.delete(tag);
  }
}

/** Fetches the shape of mode `modeNumber` (1-based) when it is not here yet (a view is about to show it). */
export async function loadMode(modeNumber: number) {
  const s = get();
  const modes = s.results?.modal?.modes;
  if (!modes?.length) return;
  const found = modes.findIndex((m) => m.mode === modeNumber);
  const i = found >= 0 ? found : 0;
  if (!modes[i].pending) return;
  const run = s.resultsRun;
  const tag = `${run}:mode:${i}`;
  if (fetching.has(tag)) return;
  fetching.add(tag);
  try {
    get().putModeShape(run, i, await backend.modeShape(i));
  } catch {
    if (get().resultsRun === run) get().notify(t("msg.caseGone"), "error");
  } finally {
    fetching.delete(tag);
  }
}

/** The model with the view's display options in `gui.viewOptions` (left out while they are the defaults). */
export function withViewOptions(m: Model, options: ViewOptions): Model {
  if (JSON.stringify(options) !== JSON.stringify(defaultViewOptions())) return { ...m, gui: { ...m.gui, viewOptions: options } };
  if (!m.gui || !("viewOptions" in m.gui)) return m;
  const { viewOptions: _, ...gui } = m.gui;
  return { ...m, gui };
}

export async function saveFile(saveAs = false) {
  const s = get();
  try {
    // The results shown belong to the model as it is (any edit clears them): they are saved with it.
    const r = await backend.saveModel(withViewOptions(withStories(s.model), s.display.options), saveAs ? null : s.filePath, s.fileName, s.results !== null);
    if (!r) return;
    get().markSaved(r.name, r.path);
    get().notify(t("msg.saved", { name: r.name }));
  } catch (e) {
    get().notify(e instanceof BackendError ? e.message : String(e), "error");
  }
}

/**
 * Edit > Store Mesh: keeps the current auto mesh of the selected areas in the model (user mesh), so
 * it stays fixed whatever the mesh settings. Openings, null areas and Membrane floors have none to keep.
 */
export async function storeSelectedMeshes() {
  const s = get();
  const model = s.model;
  try {
    const view = await backend.mesh(model);
    if (get().model !== model) return;
    const meshes = new Map<Id, UserMesh>();
    for (const id of s.selection.areas) {
      const um = userMeshOf(model, view.mesh, id);
      if (um) meshes.set(id, um);
    }
    if (!meshes.size) return get().notify(t("msg.noMeshToStore"), "error");
    if (get().edit((m) => storeMeshes(m, meshes))) get().notify(t("msg.meshStored", { list: [...meshes.keys()].join(", ") }));
  } catch (e) {
    get().notify(e instanceof BackendError ? e.message : String(e), "error");
  }
}

/** Runs the analysis. Pre-run errors stop it; warnings are shown with the option to run anyway. */
export async function runAnalysis(ignoreWarnings = false, force = false) {
  const s = get();
  if (s.running) return;
  // Notes only inform (shown in Check Model); they do not hold up a run.
  const issues = checkModel(s.model).filter((i) => i.level !== "note");
  const errors = issues.filter((i) => i.level === "error");
  if (errors.length || (!ignoreWarnings && issues.length)) {
    s.setHighlight({ nodes: issues.flatMap((i) => i.nodes ?? []), frames: issues.flatMap((i) => i.frames ?? []) });
    s.openDialog("check");
    return;
  }
  s.setRunning(true);
  s.setHighlight({ nodes: [], frames: [] });
  try {
    const results = await backend.analyze(withStories(modelForRun(s.model, s.results)), force);
    if (get().model !== s.model) return; // edited while running
    get().setResults(results);
    // Envelopes (max/min) have no equilibrium check.
    const ok = results.cases.every((c) => !c.equilibrium || (c.equilibrium.relative_error <= 1e-6 && c.equilibrium.residual <= 1e-6));
    get().setDisplay({ mode: "deformed" });
    get().setLeftTab("properties");
    get().notify(ok ? t("msg.analysisOk") : t("msg.analysisBad"), ok ? "info" : "error");
  } catch (e) {
    const report: SolverError = e instanceof BackendError ? e.report : { kind: "unknown", message: String(e) };
    get().setResults(null);
    if (report.kind === "too_big_for_memory") {
      // The estimate says the analysis will not fit: say so, and let the user run it anyway.
      const gb = (v?: number) => (v === undefined ? "?" : v.toFixed(1));
      const message = t("msg.tooBigForMemory", { needed: gb(report.needed_gb), free: gb(report.free_gb) });
      get().notify(message, "error");
      tooBig = message;
      return;
    }
    if (report.node) get().setHighlight({ nodes: [report.node], frames: [] });
    get().notify(report.message, report.kind === "stopped" ? "info" : "error");
  } finally {
    get().setRunning(false);
    if (tooBig) {
      const message = tooBig;
      tooBig = null;
      const ask = `${message}\n\n${t("msg.tooBigForMemoryAsk")}`;
      if (await backend.confirm(ask, t("msg.tooBigForMemoryTitle"))) await runAnalysis(true, true);
    }
  }
}

/** The message of a run that stopped because it would not fit in memory: the user is then asked whether to
 * run it anyway. */
let tooBig: string | null = null;

/** Deletes the results of these cases (Analyze > Load Cases to Run) and of what is made from them. */
export async function deleteCaseResults(ids: Id[]) {
  const s = get();
  if (!s.results) return;
  const del = resultsToDelete(s.model, ids);
  const indices = s.results.cases.flatMap((c, i) => (del.ids.has(c.id) ? [i] : []));
  const modal = del.modal && s.results.modal !== undefined;
  if (!indices.length && !modal) return;
  try {
    await backend.deleteResults(indices, modal);
    get().dropResults(indices, modal);
  } catch (e) {
    get().notify(e instanceof BackendError ? e.message : String(e), "error");
  }
}

export function deleteSelection() {
  const { selection, edit } = get();
  if (!selectionCount(selection)) return;
  edit((m) => deleteObjects(m, selection.nodes, selection.frames, selection.links, selection.areas), false);
}

export function selectAllInView() {
  const s = get();
  const m = s.model;
  const inViewNodes = new Set(m.nodes.filter((n) => inView([n.x, n.y, n.z], s.view)).map((n) => n.id));
  const hiddenNodes = new Set(s.hidden.nodes);
  const hiddenFrames = new Set(s.hidden.frames);
  const hiddenLinks = new Set(s.hidden.links);
  const hiddenAreas = new Set(s.hidden.areas);
  const nodes = [...inViewNodes].filter((n) => !hiddenNodes.has(n));
  const shown = (ends: Id[]) => inViewNodes.has(ends[0]) && inViewNodes.has(ends[1]);
  const frames = m.frames.filter((e) => shown(e.nodes) && !hiddenFrames.has(e.id)).map((e) => e.id);
  const links = m.links.filter((l) => shown(l.nodes) && !hiddenLinks.has(l.id)).map((l) => l.id);
  const areas = m.areas.filter((a) => a.nodes.every((n) => inViewNodes.has(n)) && !hiddenAreas.has(a.id)).map((a) => a.id);
  useStore.setState({ selection: { nodes, frames, links, areas } });
}

/** Moves a joint, then lets the solver re-apply the end order to its members. */
export async function moveJoint(id: Id, p: Vec3) {
  let moved: Model | null = null;
  const ok = get().edit((m) => (moved = moveNode(m, id, p)));
  if (!ok || !moved) return;
  try {
    const o = await backend.orient(moved);
    // Links follow the end order too (silently: they carry no loads or angle).
    const linksTurned =
      JSON.stringify(o.model.links) !== JSON.stringify((moved as Model).links) ||
      JSON.stringify(o.model.areas) !== JSON.stringify((moved as Model).areas);
    if (o.reversed.length || linksTurned) useStore.setState({ model: o.model });
    if (o.reversed.length) get().notify(t("msg.reversed", { ids: o.reversed.join(", ") }));
  } catch (e) {
    get().notify(e instanceof BackendError ? e.message : String(e), "error");
  }
}
