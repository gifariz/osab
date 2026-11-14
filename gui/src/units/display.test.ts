import { beforeEach, describe, expect, it } from "vitest";

import { addNode, newModel, setFrameLoads } from "../model/ops";
import type { Model, Results } from "../model/types";
import { buildModelScene } from "../scene/buildModelScene";
import { viewName } from "../scene/view";
import { startModel, useStore } from "../state/store";
import { frameQuantity, modelUnits, shellQuantity, unitFormat } from "./display";

const results: Results = { title: "", units: { force: "kN", length: "m" }, notes: [], cases: [] };

describe("values in display units", () => {
  it("input boxes show and read values in the display units", () => {
    const si = unitFormat("SI");
    expect(si.show(0.3, "sectionLength")).toBe("300");
    expect(si.read("300", "sectionLength")).toBeCloseTo(0.3, 12);
    expect(si.read("", "force", true)).toBe(0);
    expect(si.read("", "force")).toBeNull();
    expect(si.read("abc", "force")).toBeNull();
    const us = unitFormat("US");
    expect(us.show(0.3048, "distance")).toBe("1");
    expect(us.read("1", "force")).toBeCloseTo(4.4482216152605, 12);
    expect(us.fmt(4.4482216152605, "force")).toBe("1");
    expect(us.label("forcePerArea")).toBe("lb/ft²");
  });

  it("force components map to forces or moments, shell components to per-length values", () => {
    expect(frameQuantity("P")).toBe("force");
    expect(frameQuantity("V3")).toBe("force");
    expect(frameQuantity("M3")).toBe("moment");
    expect(frameQuantity("T")).toBe("moment");
    expect(shellQuantity("M11")).toBe("momentPerLength");
    expect(shellQuantity("MMAX")).toBe("momentPerLength");
    expect(shellQuantity("F12")).toBe("forcePerLength");
    expect(shellQuantity("VMAX")).toBe("forcePerLength");
  });

  it("a new model keeps its display units in the file; the data stays in kN and m", () => {
    const m = newModel({ xSpacings: [6.096], ySpacings: [], storyHeights: [3.048], units: "US" });
    expect(m.gui?.units).toBe("US");
    expect(m.units).toEqual({ force: "kN", length: "m" });
    expect(modelUnits(m).system).toBe("US");
    expect(newModel({ xSpacings: [6], ySpacings: [], storyHeights: [4] }).gui?.units).toBeUndefined();
  });

  it("load labels and view names follow the model's units", () => {
    let m: Model = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [4] });
    m = addNode(m, [0, 0, 4]).model;
    m = addNode(m, [6, 0, 4]).model;
    m = { ...m, frames: [{ id: "B1", type: "frame", nodes: ["1", "2"], material: "Steel", section: "FSEC1" }] };
    m = setFrameLoads(m, "Dead", ["B1"], "uniform", { type: "uniform", dir: "Z", w: -14.593902937206364 }, "add");
    const opts = { view: { kind: "3d" as const, at: 0 }, selection: {}, highlight: {}, labels: false, loadsPattern: "Dead", dimmed: false };
    const text = (model: Model) => buildModelScene(model, opts).scene.labels.map((l) => l.text);
    expect(text(m)).toContain("14.59"); // kN/m
    expect(text({ ...m, gui: { ...m.gui, units: "US" } })).toContain("1"); // kip/ft
    expect(viewName({ ...m, gui: { ...m.gui, units: "US" } }, { kind: "plan", at: 3.048 })).toBe("Plan View - Z = 10");
  });
});

describe("switching display units", () => {
  beforeEach(() => useStore.getState().load(startModel(), "Untitled.osab", null));

  it("keeps the results, is not an undo step and survives undo", () => {
    const s = useStore.getState();
    s.edit((m) => addNode(m, [0, 0, 0]).model);
    s.setResults(results);
    useStore.getState().setUnitSystem("MKS");
    let st = useStore.getState();
    expect(st.model.gui?.units).toBe("MKS");
    expect(st.results).toBe(results);
    expect(st.dirty).toBe(true);
    expect(st.past).toHaveLength(1); // only the joint
    st.undo();
    st = useStore.getState();
    expect(st.model.nodes).toHaveLength(0);
    expect(st.model.gui?.units).toBe("MKS");
    st.redo();
    expect(useStore.getState().model.gui?.units).toBe("MKS");
  });
});
