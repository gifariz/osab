import { describe, expect, it } from "vitest";

import { EditError, deleteLoadPattern, newModel, normalizeModel, setPDelta, upsertLoadPattern } from "./ops";

const model = () => upsertLoadPattern(newModel({ xSpacings: [6], ySpacings: [], storyHeights: [3] }), null, { id: "SDL", type: "super-dead" });

describe("P-Delta options in the app", () => {
  it("a new model has none; none is left out of the model", () => {
    const m = model();
    expect(m.p_delta).toBeUndefined();
    expect(setPDelta({ ...m, p_delta: { method: "mass" } }, { method: "none" }).p_delta).toBeUndefined();
  });

  it("keeps the mass method without loads and the load method with its patterns and factors", () => {
    let m = setPDelta(model(), { method: "mass", loads: [{ pattern: "SDL", scale: 2 }] });
    expect(m.p_delta).toEqual({ method: "mass" });
    m = setPDelta(m, { method: "loads", loads: [{ pattern: "Dead", scale: 1.2 }, { pattern: "SDL", scale: 0.5 }] });
    expect(m.p_delta).toEqual({ method: "loads", loads: [{ pattern: "Dead", scale: 1.2 }, { pattern: "SDL", scale: 0.5 }] });
    // Read back from a file; files without patterns name load cases.
    expect(normalizeModel(JSON.parse(JSON.stringify(m))).p_delta).toEqual(m.p_delta);
    expect(normalizeModel({ ...JSON.parse(JSON.stringify(m)), p_delta: { method: "loads", loads: [{ case: "SDL", scale: 1 }] } }).p_delta).toEqual({
      method: "loads",
      loads: [{ pattern: "SDL", scale: 1 }],
    });
    expect(normalizeModel({ ...JSON.parse(JSON.stringify(m)), p_delta: { method: "none" } }).p_delta).toBeUndefined();
  });

  it("refuses a load method without loads, unknown patterns and factors that are not numbers", () => {
    const m = model();
    expect(() => setPDelta(m, { method: "loads", loads: [] })).toThrow(EditError);
    expect(() => setPDelta(m, { method: "loads", loads: [{ pattern: "X", scale: 1 }] })).toThrow(EditError);
    expect(() => setPDelta(m, { method: "loads", loads: [{ pattern: "SDL", scale: NaN }] })).toThrow(EditError);
  });

  it("follows a renamed load pattern and keeps the one it uses from being deleted", () => {
    let m = setPDelta(model(), { method: "loads", loads: [{ pattern: "SDL", scale: 1 }] });
    m = upsertLoadPattern(m, "SDL", { id: "SDL2", type: "super-dead" });
    expect(m.p_delta?.loads).toEqual([{ pattern: "SDL2", scale: 1 }]);
    expect(() => deleteLoadPattern(m, "SDL2")).toThrow(EditError);
  });
});
