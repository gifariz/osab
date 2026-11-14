import { describe, expect, it } from "vitest";

import { EditError, combosNotRun, deleteLoadCase, finishedCases, modelForRun, newModel, normalizeModel, resultsToDelete, runCaseIds, setLoadCase, setMassSource, setNotRun, upsertCombination } from "./ops";
import { deleteSpectrumCase, setSpectrumCase, setSpectrumFunction } from "./spectrum";
import type { CaseResult, Model, Results } from "./types";

/** Load cases A and B, the modal case, a spectrum case RS and combinations over them. */
const base = (): Model => {
  let m = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [3] });
  m = setLoadCase(m, null, { id: "A", loads: [] });
  m = setLoadCase(m, null, { id: "B", loads: [] });
  m = setSpectrumFunction(m, null, { id: "F", points: [[0, 1]] });
  m = setSpectrumCase(m, null, { id: "RS", loads: [{ dir: "X", function: "F", scale: 1 }] });
  m = upsertCombination(m, null, { id: "AB", type: "linear-add", items: [{ case: "A", scale: 1 }, { case: "B", scale: 1 }] });
  m = upsertCombination(m, null, { id: "ARS", type: "linear-add", items: [{ case: "A", scale: 1 }, { case: "RS", scale: 1 }] });
  m = upsertCombination(m, null, { id: "ENV", type: "envelope", items: [{ combo: "AB", scale: 1 }] });
  return m;
};

describe("load cases to run", () => {
  it("lists load cases, the modal case and spectrum cases; keeps them in that order", () => {
    const m = base();
    const ids = runCaseIds(m);
    expect(ids.slice(-3)).toEqual(["B", "Modal", "RS"]);
    expect(setNotRun(m, ["RS", "A"]).not_run).toEqual(["A", "RS"]);
    expect(setNotRun(setNotRun(m, ["A"]), []).not_run).toBeUndefined();
    expect(() => setNotRun(m, ["Nope"])).toThrow(EditError);
    expect(() => setNotRun(m, ids)).toThrow(/At least one case/);
  });

  it("combinations with a part that does not run have no results, nested too", () => {
    const m = base();
    expect(combosNotRun(m, ["B"])).toEqual(["AB", "ENV"]);
    expect(combosNotRun(m, ["RS"])).toEqual(["ARS"]);
    expect(combosNotRun(m, ["Modal"])).toEqual([]);
  });

  it("follow renames and deletions of cases and of the modal case", () => {
    let m = setNotRun(base(), ["B", "Modal", "RS"]);
    m = setLoadCase(m, "B", { id: "B2", loads: [] });
    m = setSpectrumCase(m, "RS", { id: "EQ", loads: [{ dir: "X", function: "F", scale: 1 }] });
    m = setMassSource(m, {}, { id: "Eigen", modes: 12 });
    expect(m.not_run).toEqual(["B2", "Eigen", "EQ"]);
    m = deleteSpectrumCase({ ...m, load_combinations: [] }, "EQ");
    m = deleteLoadCase(m, "B2");
    expect(m.not_run).toEqual(["Eigen"]);
    m = setMassSource(m, {}, null);
    expect(m.not_run).toBeUndefined();
  });

  it("are read from files", () => {
    expect(normalizeModel({ not_run: ["A", 2] }).not_run).toEqual(["A", "2"]);
    expect(normalizeModel({ not_run: [] }).not_run).toBeUndefined();
  });

  it("finished cases run again, so their results stay; deleting results takes what is made from them", () => {
    const m = setNotRun(base(), ["A", "B", "Modal"]);
    const head = (id: string, extra: Partial<CaseResult> = {}): CaseResult => ({ id, displacements: [], reactions: [], frames: [], links: [], ...extra });
    const r = { cases: [head("A"), head("RS"), head("ARS", { combination: "linear-add" })], modal: {} } as unknown as Results;
    expect([...finishedCases(m, r)]).toEqual(["A", "RS", "Modal"]);
    expect(modelForRun(m, r).not_run).toEqual(["B"]);
    expect(modelForRun(m, null).not_run).toEqual(["A", "B", "Modal"]);
    expect("not_run" in modelForRun(setNotRun(m, ["A"]), r)).toBe(false);
    const del = resultsToDelete(m, ["Modal"]);
    expect(del.modal).toBe(true);
    expect([...del.ids].sort()).toEqual(["ARS", "Modal", "RS"]);
    expect([...resultsToDelete(m, ["B"]).ids].sort()).toEqual(["AB", "B", "ENV"]);
  });
});
