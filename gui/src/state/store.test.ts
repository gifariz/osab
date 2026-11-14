import { beforeEach, describe, expect, it } from "vitest";

import { addFrame, addLink, addNode, deleteObjects, EditError } from "../model/ops";
import type { Results } from "../model/types";
import { startModel, useStore } from "./store";

const results: Results = { title: "", units: { force: "kN", length: "m" }, notes: ["n"], cases: [] };

describe("store", () => {
  beforeEach(() => useStore.getState().load(startModel(), "Untitled.osab", null));

  it("records undo and redo, and marks the file changed", () => {
    const s = useStore.getState();
    s.edit((m) => addNode(m, [0, 0, 0]).model);
    s.edit((m) => addNode(m, [6, 0, 0]).model);
    expect(useStore.getState().model.nodes).toHaveLength(2);
    expect(useStore.getState().dirty).toBe(true);
    useStore.getState().undo();
    expect(useStore.getState().model.nodes).toHaveLength(1);
    useStore.getState().redo();
    expect(useStore.getState().model.nodes).toHaveLength(2);
  });

  it("clears results when the model changes and says so", () => {
    useStore.getState().setResults(results);
    useStore.getState().setDisplay({ mode: "forces" });
    useStore.getState().edit((m) => addNode(m, [0, 0, 0]).model);
    const s = useStore.getState();
    expect(s.results).toBeNull();
    expect(s.display.mode).toBe("model");
    expect(s.notice?.text).toMatch(/Results cleared/);
  });

  it("shows edit errors as a notice instead of throwing", () => {
    const ok = useStore.getState().edit(() => {
      throw new EditError("nope");
    });
    expect(ok).toBe(false);
    expect(useStore.getState().notice).toEqual({ text: "nope", level: "error" });
  });

  it("selects, toggles and drops deleted objects from the selection", () => {
    const s = useStore.getState();
    s.edit((m) => addNode(m, [0, 0, 0]).model);
    s.select([{ kind: "node", id: "1" }], "replace");
    s.select([{ kind: "node", id: "1" }], "toggle");
    expect(useStore.getState().selection.nodes).toEqual([]);
    s.select([{ kind: "node", id: "1" }], "add");
    s.edit((m) => ({ ...m, nodes: [] }));
    expect(useStore.getState().selection.nodes).toEqual([]);
  });
});

describe("hidden objects", () => {
  beforeEach(() => {
    useStore.getState().load(startModel(), "Untitled.osab", null);
    const s = useStore.getState();
    s.edit((m) => addNode(m, [0, 0, 0]).model);
    s.edit((m) => addNode(m, [0, 0, 4]).model);
    s.edit((m) => addFrame(m, "1", "2", { type: "frame", material: "Steel", section: "FSEC1" }).model);
    s.edit((m) => addNode(m, [6, 0, 0]).model);
  });

  it("hiding drops objects from the selection, and hidden objects cannot be selected", () => {
    const s = useStore.getState();
    s.select([{ kind: "frame", id: "M1" }, { kind: "node", id: "3" }], "add");
    s.hide({ nodes: [], frames: ["M1"] });
    expect(useStore.getState().selection).toEqual({ nodes: ["3"], frames: [], links: [], areas: [] });
    s.select([{ kind: "frame", id: "M1" }], "add");
    expect(useStore.getState().selection.frames).toEqual([]);
    s.show({ nodes: [], frames: ["M1"] });
    expect(useStore.getState().hidden).toEqual({ nodes: [], frames: [], links: [], areas: [] });
  });

  it("show only keeps the chosen members with their joints; deleting prunes the hidden list", () => {
    const s = useStore.getState();
    s.showOnly({ nodes: [], frames: ["M1"] });
    expect(useStore.getState().hidden).toEqual({ nodes: ["3"], frames: [], links: [], areas: [] });
    s.edit((m) => deleteObjects(m, ["3"], []));
    expect(useStore.getState().hidden.nodes).toEqual([]);
    s.hide({ nodes: ["1"], frames: [] });
    s.showAll();
    expect(useStore.getState().hidden).toEqual({ nodes: [], frames: [], links: [], areas: [] });
  });

  it("selects, hides and shows only rigid links like members", () => {
    const s = useStore.getState();
    s.edit((m) => addLink(m, "2", "3").model); // joint 2 (0,0,4) to joint 3 (6,0,0): end i is 3
    s.select([{ kind: "link", id: "K1" }, { kind: "frame", id: "M1" }], "add");
    expect(useStore.getState().selection).toEqual({ nodes: [], frames: ["M1"], links: ["K1"], areas: [] });
    s.hide({ links: ["K1"] });
    expect(useStore.getState().selection.links).toEqual([]);
    s.select([{ kind: "link", id: "K1" }], "add");
    expect(useStore.getState().selection.links).toEqual([]);
    s.showOnly({ links: ["K1"] });
    expect(useStore.getState().hidden).toEqual({ nodes: ["1"], frames: ["M1"], links: [], areas: [] });
    s.edit((m) => deleteObjects(m, [], [], ["K1"]));
    expect(useStore.getState().model.links).toEqual([]);
  });
});
