// The worker client: replies matched by id, Stop ends the worker and a new one starts on the next call.

import { describe, expect, it } from "vitest";

import { BrowserSolver, type WorkerLike } from "./browserSolver";
import type { SolverRequest, SolverResponse } from "./solver.worker";

/** A worker that keeps its requests until the test answers them. */
class FakeWorker implements WorkerLike {
  requests: SolverRequest[] = [];
  terminated = false;
  onmessage: ((e: MessageEvent<SolverResponse>) => void) | null = null;
  onerror: ((e: ErrorEvent) => void) | null = null;
  postMessage(message: SolverRequest) {
    this.requests.push(message);
  }
  terminate() {
    this.terminated = true;
  }
  answer(i: number, reply: SolverResponse["reply"]) {
    this.onmessage!({ data: { id: this.requests[i].id, reply } } as MessageEvent<SolverResponse>);
  }
}

function setup() {
  const workers: FakeWorker[] = [];
  const solver = new BrowserSolver(() => {
    workers.push(new FakeWorker());
    return workers[workers.length - 1];
  });
  return { solver, workers };
}

describe("browser solver client", () => {
  it("starts one worker and matches replies to calls", async () => {
    const { solver, workers } = setup();
    const a = solver.call("analyze", { title: "a" }, 5000);
    const b = solver.call("mesh", { title: "b" }, 5000);
    expect(workers).toHaveLength(1);
    expect(workers[0].requests.map((r) => [r.command, r.maxJoints])).toEqual([
      ["analyze", 5000],
      ["mesh", 5000],
    ]);
    workers[0].answer(1, { ok: "mesh" });
    workers[0].answer(0, { error: { kind: "unstable", message: "m", node: "3" } });
    expect(await b).toEqual({ ok: "mesh" });
    expect((await a).error!.node).toBe("3");
  });

  it("stop ends the worker, answers waiting calls with 'stopped' and the next call starts a new worker", async () => {
    const { solver, workers } = setup();
    const run = solver.call("analyze", {}, 0);
    solver.stop();
    expect(workers[0].terminated).toBe(true);
    expect((await run).error!.kind).toBe("stopped");
    const next = solver.call("analyze", {}, 0);
    expect(workers).toHaveLength(2);
    workers[1].answer(0, { ok: 1 });
    expect(await next).toEqual({ ok: 1 });
    // A late reply from the ended worker is ignored.
    workers[0].answer(0, { ok: "late" });
  });

  it("a worker that fails to load answers with 'load' and is replaced", async () => {
    const { solver, workers } = setup();
    const run = solver.call("orient", {}, 0);
    workers[0].onerror!({ message: "script error" } as ErrorEvent);
    expect((await run).error).toEqual({ kind: "load", message: "script error" });
    void solver.call("orient", {}, 0);
    expect(workers).toHaveLength(2);
  });
});
