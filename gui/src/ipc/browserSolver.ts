// The browser's side of the solver worker (solver.worker.ts): sends calls, matches the replies, and
// stops a run by ending the worker (the next call starts a new one).

import type { SolverError } from "../model/types";
import type { SolverRequest, SolverResponse } from "./solver.worker";
import type { Reply, SolverCommand } from "./wasm";

/** What this needs from a Worker (a fake one in tests). */
export interface WorkerLike {
  postMessage(message: SolverRequest): void;
  terminate(): void;
  onmessage: ((e: MessageEvent<SolverResponse>) => void) | null;
  onerror: ((e: ErrorEvent) => void) | null;
}

export class BrowserSolver {
  private worker: WorkerLike | null = null;
  private pending = new Map<number, (reply: Reply) => void>();
  private nextId = 1;

  constructor(private startWorker: () => WorkerLike) {}

  /** Runs one command in the worker; errors come back in the reply, as from the module itself. */
  call<T>(command: SolverCommand | "case" | "mode" | "delete", model: unknown, maxJoints: number): Promise<Reply<T>> {
    const worker = (this.worker ??= this.start());
    const id = this.nextId++;
    return new Promise((resolve) => {
      this.pending.set(id, resolve as (reply: Reply) => void);
      worker.postMessage({ id, command, model, maxJoints });
    });
  }

  /** Ends the worker; calls still waiting get an error of kind "stopped". */
  stop() {
    this.end({ kind: "stopped", message: "" });
  }

  private start(): WorkerLike {
    const worker = this.startWorker();
    worker.onmessage = (e) => {
      const done = this.pending.get(e.data.id);
      this.pending.delete(e.data.id);
      done?.(e.data.reply);
    };
    // The worker script itself failed (it catches the solver's own errors).
    worker.onerror = (e) => this.end({ kind: "load", message: e.message ?? "" });
    return worker;
  }

  private end(error: SolverError) {
    this.worker?.terminate();
    this.worker = null;
    const waiting = [...this.pending.values()];
    this.pending.clear();
    for (const done of waiting) done({ error });
  }
}
