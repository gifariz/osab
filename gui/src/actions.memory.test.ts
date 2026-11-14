// A run that the solver estimates will not fit in memory: the app says so with the numbers, asks whether to
// run it anyway, and runs it again with `force` only when the user agrees.
import { beforeEach, describe, expect, it, vi } from "vitest";

const analyze = vi.fn();
const confirm = vi.fn();
vi.mock("./ipc/backend", async (importOriginal) => {
  const real = await importOriginal<typeof import("./ipc/backend")>();
  return { ...real, backend: { ...real.backend, analyze, confirm } };
});

const { runAnalysis } = await import("./actions");
const { BackendError } = await import("./ipc/backend");
const { useStore } = await import("./state/store");
const { exampleModel, EXAMPLES } = await import("./model/examples");

const tooBig = new BackendError({ kind: "too_big_for_memory", message: "solver text", needed_gb: 41.2, free_gb: 27.04 });

describe("a run too big for memory", () => {
  beforeEach(() => {
    analyze.mockReset();
    confirm.mockReset();
    useStore.getState().load(exampleModel(EXAMPLES[0]), "example.osab", null);
  });

  it("asks with the numbers and runs again with force when the user agrees", async () => {
    analyze.mockRejectedValueOnce(tooBig).mockResolvedValueOnce({ cases: [] });
    confirm.mockResolvedValueOnce(true);
    await runAnalysis(true);
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(confirm.mock.calls[0][0]).toContain("41.2");
    expect(confirm.mock.calls[0][0]).toContain("27.0");
    expect(analyze.mock.calls.map((c) => c[1])).toEqual([false, true]);
    expect(useStore.getState().running).toBe(false);
  });

  it("stops when the user does not want to run it anyway", async () => {
    analyze.mockRejectedValueOnce(tooBig);
    confirm.mockResolvedValueOnce(false);
    await runAnalysis(true);
    expect(analyze).toHaveBeenCalledTimes(1);
    expect(useStore.getState().results).toBeNull();
  });
});
