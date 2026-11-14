// The browser's solver in a background thread (Web Worker), so the page stays responsive during a run.
// Loads the WebAssembly module (`osab.wasm`, built by `npm run build:wasm`) on the first call and
// answers one call at a time; after a crash the next call loads a fresh module. A run's results stay
// here: the page gets their summary and asks for each case it shows ("case", with its index) and, when the
// summary leaves a mode's shape out, for the shape ("mode", with the mode's index).

import type { Results } from "../model/types";
import { summarize } from "./resultCases";
import wasmUrl from "./osab.wasm?url";
import { WasmSolver, type Reply, type SolverCommand } from "./wasm";

export interface SolverRequest {
  id: number;
  command: SolverCommand | "case" | "mode" | "delete";
  /** The model; for "case" and "mode", the case's or mode's index; for "delete", the cases' indices and whether the modal results go. */
  model: unknown;
  /** Size limit for `analyze` (analysis joints; 0: none). */
  maxJoints: number;
}

export interface SolverResponse {
  id: number;
  reply: Reply;
}

let module: Promise<WasmSolver> | null = null;

async function load() {
  const res = await fetch(wasmUrl);
  if (!res.ok) throw new Error(`${wasmUrl}: ${res.status} ${res.statusText}`);
  return WasmSolver.load(await res.arrayBuffer());
}

/** The last run's results. */
let last: Results | null = null;

async function answer({ command, model, maxJoints }: SolverRequest): Promise<Reply> {
  if (command === "delete") {
    const { indices, modal } = model as { indices: number[]; modal: boolean };
    if (last) last = { ...last, cases: last.cases.filter((_, i) => !indices.includes(i)), ...(modal ? { modal: undefined } : {}) };
    return { ok: null };
  }
  if (command === "case") {
    const c = last?.cases[model as number];
    return c ? { ok: c } : { error: { kind: "no_results", message: "no results for this case" } };
  }
  if (command === "mode") {
    const m = last?.modal?.modes[model as number];
    return m ? { ok: m.shape } : { error: { kind: "no_results", message: "no shape for this mode" } };
  }
  let solver: WasmSolver;
  try {
    solver = await (module ??= load());
  } catch (e) {
    module = null;
    return { error: { kind: "load", message: String(e) } };
  }
  const reply = solver.call(command, model, maxJoints);
  if (solver.stopped) module = null;
  if (command === "analyze" && reply.ok) {
    last = reply.ok as Results;
    return { ok: summarize(last) };
  }
  return reply;
}

self.onmessage = async (e: MessageEvent<SolverRequest>) => {
  const response: SolverResponse = { id: e.data.id, reply: await answer(e.data) };
  postMessage(response);
};
