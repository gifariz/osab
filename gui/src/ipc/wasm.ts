// The solver as a WebAssembly module (the `web/` crate, built by `npm run build:wasm` into
// `src/ipc/osab.wasm`): one call, JSON in, JSON out, with the same replies as the desktop app's
// Tauri commands. The browser's solver worker uses it; the Vitest test runs it in Node.

import type { SolverError } from "../model/types";

export type SolverCommand = "analyze" | "mesh" | "orient" | "spectrum" | "section" | "section_list";

export interface Reply<T = unknown> {
  ok?: T;
  error?: SolverError;
}

interface Exports {
  memory: WebAssembly.Memory;
  alloc(len: number): number;
  call(cmd: number, cmdLen: number, input: number, inputLen: number, maxJoints: number): void;
  reply_ptr(): number;
  reply_len(): number;
}

export class WasmSolver {
  /** True once the module has stopped (a bug or out of memory); load a new one for the next call. */
  stopped = false;

  private constructor(private ex: Exports) {}

  static async load(bytes: BufferSource): Promise<WasmSolver> {
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return new WasmSolver(instance.exports as unknown as Exports);
  }

  /**
   * Runs `command` on `model` (an object or JSON text). `maxJoints` stops `analyze` on bigger analysis
   * models (0: no limit). A stopped module answers with an error of kind "crash"; its message is the
   * solver's when it left one, else empty (out of memory).
   */
  call<T>(command: SolverCommand, model: unknown, maxJoints = 0): Reply<T> {
    if (this.stopped) throw new Error("the solver module has stopped; load a new one");
    const cmd = this.write(command);
    const input = this.write(typeof model === "string" ? model : JSON.stringify(model));
    try {
      this.ex.call(cmd.ptr, cmd.len, input.ptr, input.len, maxJoints);
    } catch {
      this.stopped = true;
      return this.read<T>() ?? { error: { kind: "crash", message: "" } };
    }
    return this.read<T>()!;
  }

  private write(text: string) {
    const bytes = new TextEncoder().encode(text);
    const ptr = this.ex.alloc(bytes.length);
    new Uint8Array(this.ex.memory.buffer, ptr, bytes.length).set(bytes);
    return { ptr, len: bytes.length };
  }

  private read<T>(): Reply<T> | null {
    const len = this.ex.reply_len();
    if (!len) return null;
    return JSON.parse(new TextDecoder().decode(new Uint8Array(this.ex.memory.buffer, this.ex.reply_ptr(), len)));
  }
}
