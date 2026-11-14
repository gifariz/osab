import { describe, expect, it } from "vitest";

import { EditError, deleteLoadCase, newModel, normalizeModel, setLoadCase, setMassSource, upsertCombination } from "./ops";
import { deleteSpectrumCase, deleteSpectrumFunction, setSpectrumCase, setSpectrumFunction } from "./spectrum";
import type { Model, SpectrumCase } from "./types";

const base = (): Model => {
  let m = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [3] });
  m = setSpectrumFunction(m, null, { id: "RS", points: [[0, 0.4], [0.5, 1], [2, 0.25]] });
  return m;
};
const rsx: SpectrumCase = { id: "RSX", loads: [{ dir: "X", function: "RS", scale: 9.80665 }] };

describe("response spectrum functions", () => {
  it("adds tables and ASCE 7-16 functions, keeping only what differs from the defaults", () => {
    let m = base();
    expect(m.spectrum_functions).toEqual([{ id: "RS", points: [[0, 0.4], [0.5, 1], [2, 0.25]] }]);
    m = setSpectrumFunction(m, null, { id: "A", damping: 0.02, asce7_16: { ss: 0.8, s1: 0.35, tl: 8, site_class: "D" }, points: [[0, 1]] });
    expect(m.spectrum_functions![1]).toEqual({ id: "A", damping: 0.02, asce7_16: { ss: 0.8, s1: 0.35, tl: 8, site_class: "D" } });
  });

  it("refuses bad tables and inputs", () => {
    const m = base();
    const bad = (f: Parameters<typeof setSpectrumFunction>[2]) => expect(() => setSpectrumFunction(m, null, f)).toThrow(EditError);
    bad({ id: "X", points: [] });
    bad({ id: "X", points: [[0, 1], [0, 2]] });
    bad({ id: "X", points: [[-1, 1]] });
    bad({ id: "X", damping: 1, points: [[0, 1]] });
    bad({ id: "X", asce7_16: { ss: 0, s1: 0.3, tl: 8, site_class: "D" } });
    bad({ id: "X", asce7_16: { ss: 1, s1: 0.3, tl: 8, site_class: "F" } });
    bad({ id: "RS", points: [[0, 1]] });
  });

  it("renaming follows into the cases; deleting is refused while a case uses it", () => {
    let m = setSpectrumCase(base(), null, rsx);
    m = setSpectrumFunction(m, "RS", { ...m.spectrum_functions![0], id: "DESIGN" });
    expect(m.spectrum_cases![0].loads[0].function).toBe("DESIGN");
    expect(() => deleteSpectrumFunction(m, "DESIGN")).toThrow(/RSX/);
    m = deleteSpectrumCase(m, "RSX");
    expect(deleteSpectrumFunction(m, "DESIGN").spectrum_functions).toBeUndefined();
  });
});

describe("response spectrum cases", () => {
  it("turn on the modal case; names are shared with load cases; defaults left out", () => {
    let m = base();
    delete m.modal;
    m = setSpectrumCase(m, null, { ...rsx, damping: 0.05, modal_combination: "cqc", directional_combination: "srss", abs_factor: 0.3, eccentricity: 0 });
    expect(m.modal).toEqual({ id: "Modal", modes: 12 });
    expect(m.spectrum_cases).toEqual([rsx]);
    expect(() => setSpectrumCase(m, null, { ...rsx, id: m.load_cases[0].id })).toThrow(/already exists/);
    expect(() => setLoadCase(m, null, { id: "RSX", loads: [] })).toThrow(/already exists/);
    expect(() => setMassSource(m, {}, null)).toThrow(/RSX/);
    m = setSpectrumCase(m, "RSX", { ...rsx, directional_combination: "absolute", abs_factor: 0.3, eccentricity: 0.05 });
    expect(m.spectrum_cases![0]).toEqual({ ...rsx, directional_combination: "absolute", abs_factor: 0.3, eccentricity: 0.05 });
  });

  it("refuses bad cases", () => {
    const m = base();
    const bad = (c: SpectrumCase) => expect(() => setSpectrumCase(m, null, c)).toThrow(EditError);
    bad({ id: "Q", loads: [] });
    bad({ id: "Q", loads: [{ dir: "X", function: "NOPE", scale: 1 }] });
    bad({ id: "Q", loads: [{ dir: "X", function: "RS", scale: NaN }] });
    bad({ ...rsx, id: "Q", damping: -0.1 });
    bad({ ...rsx, id: "Q", eccentricity: -1 });
    bad({ ...rsx, id: "Q", directional_combination: "cqc3" });
    bad({ ...rsx, id: "Modal" });
  });

  it("are parts of combinations: renaming follows, deleting is refused", () => {
    let m = setSpectrumCase(base(), null, rsx);
    m = upsertCombination(m, null, { id: "C", type: "linear-add", items: [{ case: m.load_cases[0].id, scale: 1 }, { case: "RSX", scale: 1 }] });
    m = setSpectrumCase(m, "RSX", { ...rsx, id: "EQX" });
    expect(m.load_combinations[0].items[1]).toEqual({ case: "EQX", scale: 1 });
    expect(() => deleteSpectrumCase(m, "EQX")).toThrow(/C/);
    expect(() => deleteLoadCase(m, "EQX")).toThrow(/C/);
  });

  it("are read from files", () => {
    const m = normalizeModel({
      spectrum_functions: [{ id: 1, points: [["0", 0.4]] }, { id: "A", asce7_16: { ss: 1, s1: 0.4, tl: 8, site_class: "C", fa: 1.2 } }],
      spectrum_cases: [{ id: "E", loads: [{ dir: "y", function: 1, scale: "2", angle: 30 }], directional_combination: "cqc3", eccentricity: 0.05 }],
    });
    expect(m.spectrum_functions).toEqual([{ id: "1", points: [[0, 0.4]] }, { id: "A", asce7_16: { ss: 1, s1: 0.4, tl: 8, site_class: "C", fa: 1.2 } }]);
    expect(m.spectrum_cases).toEqual([{ id: "E", loads: [{ dir: "Y", function: "1", scale: 2, angle: 30 }], directional_combination: "cqc3", eccentricity: 0.05 }]);
  });
});
