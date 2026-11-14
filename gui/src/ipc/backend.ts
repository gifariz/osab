// Everything the UI needs from outside the browser: the solver, files, dialogs, the window.
// In the desktop app this goes through Tauri commands. In a normal browser (the web demo, and
// `npm run dev:web` for development and the click tests) the same solver runs as WebAssembly in a
// background thread (solver.worker.ts).

import { t } from "../i18n";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";

import { normalizeModel } from "../model/ops";
import type { CaseResult, Material, MeshInfo, Model, ModeShape, Results, SolverError, SpectrumFunction } from "../model/types";
import type { ShapeSpec } from "../model/sectionShapes";
import { BrowserSolver } from "./browserSolver";
import { decodeCase, decodeShape, fromSummary, type ResultsSummary } from "./resultCases";
import type { SolverCommand } from "./wasm";

/**
 * The browser's size limit: analysis joints (object joints + mesh joints) per run; about 12 s for a meshed
 * slab in Chrome on the development PC (2026-10-23: 2,601 joints 1.6 s, 5,041 4.4 s, 7,396 10 s, 10,201
 * 17 s). The desktop app has none.
 */
export const BROWSER_MAX_JOINTS = 8000;

/**
 * Largest model file the browser opens, in MB: large models open and can be edited and saved (only a run
 * has the joint limit above); this stops wrong files.
 */
export const BROWSER_MAX_FILE_MB = 200;

export interface Oriented {
  model: Model;
  reversed: string[];
}

/** The analysis mesh only (View > Show Analysis Mesh before a run), with the mesh step's notes. */
export interface MeshView {
  notes: string[];
  mesh: MeshInfo;
}

export interface OpenedFile {
  path: string | null;
  name: string;
  model: Model;
  /** Results saved with the model (desktop: `<name>.results.json`, when they belong to it). */
  results?: Results;
}

/** A section shape's numbers (solver: `osab_mesh::section::SectionProps`) and its outline to draw. */
export interface ShapeProps {
  A: number;
  I22: number;
  I33: number;
  I23: number;
  J: number;
  As2: number;
  As3: number;
  depth: number;
  width: number;
  centroid_offset: [number, number];
  self_weight?: number;
  outline: [number, number][][];
  /** A list section's plain shape (its dimensions in the model's unit), kept for drawing it. */
  like?: ShapeSpec;
  fill_outline?: [number, number][][];
}

/** A spectrum function's points (period, value) and, for ASCE 7-16, Fa, Fv, SDS and SD1. */
export interface SpectrumPoints {
  points: [number, number][];
  asce7_16?: { fa: number; fv: number; sds: number; sd1: number };
}

export interface Backend {
  readonly kind: "desktop" | "browser";
  /** Runs the analysis: the results come with every case pending (`ipc/resultCases.ts`). */
  /** `force`: run even when the analysis is estimated not to fit in memory (desktop only). */
  analyze(model: Model, force?: boolean): Promise<Results>;
  /** The values of a case of the last run (or of the results opened with the model), by its index. */
  resultCase(index: number): Promise<CaseResult>;
  /** The shape of a mode of the last run's modal case, by its index in the modes. */
  modeShape(index: number): Promise<ModeShape>;
  /** Deletes the results of the cases at `indices` (and with `modal` the modal results); the rest close up. */
  deleteResults(indices: number[], modal: boolean): Promise<void>;
  mesh(model: Model): Promise<MeshView>;
  orient(model: Model): Promise<Oriented>;
  /** A spectrum function's points as an analysis uses them (and ASCE 7-16's coefficients); an error says what is wrong. */
  spectrum(f: SpectrumFunction): Promise<SpectrumPoints>;
  /** A section shape's properties and outline as the analysis uses them (lengths in `length`, the model's unit); an error says what is wrong. */
  section(shape: ShapeSpec, materials: Material[], length?: string): Promise<ShapeProps>;
  /** The steel section list: [name, type] of every shape. */
  sectionList(): Promise<[string, string][]>;
  /** Whether a running analysis can be stopped (the browser's solver runs in a worker). */
  readonly canStop: boolean;
  /** Stops the running analysis; it ends with an error of kind "stopped". */
  stop(): void;
  /** Opens a model: the file at `path`, or one picked in a dialog. */
  openModel(path?: string): Promise<OpenedFile | null>;
  /** The model file the app was started with (a double-clicked `.osab` file), if any. */
  startupFile(): Promise<string | null>;
  /**
   * Saves to `path`, or asks for a file name when it is null. Returns null when cancelled. With
   * `withResults` (the results shown belong to this model) the desktop app saves them next to it.
   */
  saveModel(model: Model, path: string | null, suggestedName: string, withResults?: boolean): Promise<{ path: string | null; name: string } | null>;
  /**
   * Saves bytes the window made (a table export) as a file: the desktop app asks where (`filter`: the file
   * type), the browser downloads it. Returns the name saved, or null when cancelled.
   */
  saveFile(suggestedName: string, data: Uint8Array, filter: { name: string; extensions: string[] }): Promise<string | null>;
  confirm(message: string, title: string): Promise<boolean>;
  setTitle(title: string): void;
  /** Window frame colours (title bar); null follows Windows. */
  setTheme(theme: "light" | "dark" | null): void;
  /** Asks before closing the window when there are unsaved changes. */
  onCloseRequested(isDirty: () => boolean, askDiscard: () => Promise<boolean>): void;
}

/** A failed solver or file call; `report` says what went wrong (and which joint, if known). */
export class BackendError extends Error {
  constructor(public report: SolverError) {
    super(report.message);
  }
}

const toError = (e: unknown): BackendError => {
  if (e instanceof BackendError) return e;
  if (e && typeof e === "object" && "message" in e && "kind" in e) return new BackendError(e as SolverError);
  if (e instanceof Error) return new BackendError({ kind: "unknown", message: e.message });
  return new BackendError({ kind: "unknown", message: String(e) });
};

const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;
// Models are saved as `.osab` (JSON inside); `.json` files still open.
const openFilters = () => [{ name: t("msg.fileFilter"), extensions: ["osab", "json"] }];
const saveFilters = () => [{ name: t("msg.fileFilter"), extensions: ["osab"] }];
/** The name a model is saved under: `.osab`, also for a model opened from a `.json` file. */
export const osabName = (name: string) => (/\.(json|osab)$/i.test(name) ? name.replace(/\.json$/i, ".osab") : `${name}.osab`);

class DesktopBackend implements Backend {
  readonly kind = "desktop" as const;
  readonly canStop = false;
  /** The summary of the results kept in the Rust process (for reading their cases' numbers). */
  private summary: ResultsSummary | null = null;

  private keep(s: ResultsSummary): Results {
    this.summary = s;
    return fromSummary(s);
  }

  stop() {
    // The run is in the Rust process and cannot be stopped from here.
  }

  async analyze(model: Model, force = false): Promise<Results> {
    try {
      return this.keep(await invoke<ResultsSummary>("analyze", { model, force }));
    } catch (e) {
      throw toError(e);
    }
  }

  async resultCase(index: number): Promise<CaseResult> {
    try {
      const bytes = await invoke<ArrayBuffer>("result_case", { index });
      const s = this.summary;
      if (!s?.cases[index]) throw new BackendError({ kind: "no_results", message: "no results for this case" });
      return decodeCase(bytes, s.cases[index], s.geometry);
    } catch (e) {
      throw toError(e);
    }
  }

  async modeShape(index: number): Promise<ModeShape> {
    try {
      const bytes = await invoke<ArrayBuffer>("result_mode", { index });
      return decodeShape(bytes, this.summary?.modal?.joints);
    } catch (e) {
      throw toError(e);
    }
  }

  async deleteResults(indices: number[], modal: boolean) {
    try {
      await invoke("delete_results", { indices, modal });
    } catch (e) {
      throw toError(e);
    }
    const s = this.summary;
    if (s) this.summary = { ...s, cases: s.cases.filter((_, i) => !indices.includes(i)), ...(modal ? { modal: undefined } : {}) };
  }

  async mesh(model: Model): Promise<MeshView> {
    try {
      return await invoke<MeshView>("mesh_model", { model });
    } catch (e) {
      throw toError(e);
    }
  }

  async orient(model: Model): Promise<Oriented> {
    try {
      const o = await invoke<Oriented>("orient_model", { model });
      return { model: normalizeModel(o.model), reversed: o.reversed };
    } catch (e) {
      throw toError(e);
    }
  }

  async section(shape: ShapeSpec, materials: Material[], length?: string): Promise<ShapeProps> {
    try {
      return await invoke<ShapeProps>("section_shape", { input: { shape, materials, length } });
    } catch (e) {
      throw toError(e);
    }
  }

  async sectionList(): Promise<[string, string][]> {
    return await invoke<[string, string][]>("section_list");
  }

  async spectrum(f: SpectrumFunction): Promise<SpectrumPoints> {
    try {
      return await invoke<SpectrumPoints>("spectrum_function", { function: f });
    } catch (e) {
      throw toError(e);
    }
  }

  async startupFile(): Promise<string | null> {
    return await invoke<string | null>("startup_file");
  }

  async openModel(given?: string): Promise<OpenedFile | null> {
    const path = given ?? (await open({ multiple: false, directory: false, filters: openFilters() }));
    if (typeof path !== "string") return null;
    try {
      const raw = await invoke<unknown>("open_model", { path });
      const saved = await invoke<ResultsSummary | null>("open_results", { path });
      return { path, name: baseName(path), model: normalizeModel(raw), ...(saved ? { results: this.keep(saved) } : {}) };
    } catch (e) {
      throw toError(e);
    }
  }

  async saveModel(model: Model, path: string | null, suggestedName: string, withResults = false) {
    const target = path ?? (await save({ filters: saveFilters(), defaultPath: osabName(suggestedName) }));
    if (!target) return null;
    try {
      await invoke("save_model", { path: target, model, withResults });
    } catch (e) {
      throw toError(e);
    }
    return { path: target, name: baseName(target) };
  }

  async saveFile(suggestedName: string, data: Uint8Array, filter: { name: string; extensions: string[] }) {
    const target = await save({ filters: [filter], defaultPath: suggestedName });
    if (!target) return null;
    try {
      await invoke("write_file", data, { headers: { path: encodeURIComponent(target) } });
    } catch (e) {
      throw toError(e);
    }
    return baseName(target);
  }

  confirm(message: string, title: string) {
    return ask(message, { title, kind: "warning" });
  }

  setTitle(title: string) {
    void getCurrentWindow().setTitle(title);
    document.title = title;
  }

  setTheme(theme: "light" | "dark" | null) {
    void getCurrentWindow().setTheme(theme);
  }

  onCloseRequested(isDirty: () => boolean, askDiscard: () => Promise<boolean>) {
    const win = getCurrentWindow();
    void win.onCloseRequested(async (event) => {
      if (!isDirty()) return;
      event.preventDefault();
      if (await askDiscard()) await win.destroy();
    });
  }
}

/** The browser's own errors in the UI language (the solver's other messages are shown as they come). */
export function browserError(e: SolverError): SolverError {
  if (e.kind === "too_big") return { ...e, message: t("msg.tooBig", { joints: e.joints ?? "?", limit: e.limit ?? "?" }) };
  if (e.kind === "stopped") return { ...e, message: t("msg.runStopped") };
  if (e.kind === "crash") return { ...e, message: e.message ? t("msg.solverCrashed", { error: e.message }) : t("msg.solverOutOfMemory") };
  if (e.kind === "load") return { ...e, message: t("msg.solverLoad", { error: e.message }) };
  return e;
}

class BrowserBackend implements Backend {
  async startupFile(): Promise<string | null> {
    return null;
  }

  readonly kind = "browser" as const;
  readonly canStop = true;
  private solver = new BrowserSolver(() => new Worker(new URL("./solver.worker.ts", import.meta.url), { type: "module" }));

  private async call<T>(command: SolverCommand | "case" | "mode" | "delete", model: unknown): Promise<T> {
    const reply = await this.solver.call<T>(command, model, BROWSER_MAX_JOINTS);
    if (reply.error) throw new BackendError(browserError(reply.error));
    return reply.ok as T;
  }

  stop() {
    this.solver.stop();
  }

  async analyze(model: Model) {
    return fromSummary(await this.call<ResultsSummary>("analyze", model));
  }

  resultCase(index: number) {
    return this.call<CaseResult>("case", index);
  }

  modeShape(index: number) {
    return this.call<ModeShape>("mode", index);
  }

  async deleteResults(indices: number[], modal: boolean) {
    await this.call("delete", { indices, modal });
  }

  mesh(model: Model) {
    return this.call<MeshView>("mesh", model);
  }

  async orient(model: Model) {
    const o = await this.call<Oriented>("orient", model);
    return { model: normalizeModel(o.model), reversed: o.reversed };
  }

  section(shape: ShapeSpec, materials: Material[], length?: string) {
    return this.call<ShapeProps>("section", { shape, materials, length });
  }

  sectionList() {
    return this.call<[string, string][]>("section_list", {});
  }

  spectrum(f: SpectrumFunction) {
    return this.call<SpectrumPoints>("spectrum", f);
  }

  openModel(): Promise<OpenedFile | null> {
    return new Promise((resolve, reject) => {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = ".osab,.json,application/json";
      input.onchange = async () => {
        const file = input.files?.[0];
        if (!file) return resolve(null);
        if (file.size > BROWSER_MAX_FILE_MB * 1e6) {
          const mb = (file.size / 1e6).toFixed(1);
          return reject(new BackendError({ kind: "io", message: t("msg.fileTooBig", { name: file.name, mb, max: BROWSER_MAX_FILE_MB }) }));
        }
        try {
          resolve({ path: null, name: file.name, model: normalizeModel(JSON.parse(await file.text())) });
        } catch (e) {
          reject(new BackendError({ kind: "json", message: t("msg.cannotRead", { name: file.name, error: e instanceof Error ? e.message : String(e) }) }));
        }
      };
      input.click();
    });
  }

  async saveModel(model: Model, _path: string | null, suggestedName: string) {
    const blob = new Blob([JSON.stringify(model, null, 2) + "\n"], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = osabName(suggestedName);
    a.click();
    URL.revokeObjectURL(a.href);
    return { path: null, name: osabName(suggestedName) };
  }

  async saveFile(suggestedName: string, data: Uint8Array) {
    const blob = new Blob([data as BlobPart]);
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = suggestedName;
    a.click();
    URL.revokeObjectURL(a.href);
    return suggestedName;
  }

  async confirm(message: string) {
    return window.confirm(message);
  }

  setTitle(title: string) {
    document.title = title;
  }

  setTheme() {
    // The browser draws its own frame.
  }

  onCloseRequested(isDirty: () => boolean) {
    // The browser shows its own "leave site?" prompt.
    window.addEventListener("beforeunload", (e) => {
      if (isDirty()) e.preventDefault();
    });
  }
}

export const backend: Backend = isTauri() ? new DesktopBackend() : new BrowserBackend();
