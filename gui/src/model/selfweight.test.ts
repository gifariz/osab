import { describe, expect, it } from "vitest";

import { checkModel } from "./check";
import { EditError, newModel, normalizeModel, upsertLoadPattern, upsertMaterial } from "./ops";

const fresh = () => newModel({ xSpacings: [6], ySpacings: [], storyHeights: [4] });

describe("self-weight", () => {
  it("a new model has the defaults: Dead x 1 and unit weights", () => {
    const m = fresh();
    expect(m.load_patterns[0].self_weight).toBe(1);
    expect(Object.fromEntries(m.materials.map((x) => [x.id, x.weight]))).toEqual({ Steel: 76.9729, Concrete: 23.5631 });
    // Self-weight counts as a load: no "has no loads" warning.
    expect(checkModel(m).some((i) => i.message.includes("has no loads"))).toBe(false);
  });

  it("multipliers and weights are kept, set and checked", () => {
    let m = fresh();
    m = upsertLoadPattern(m, null, { id: "SDL", type: "super-dead", self_weight: 0 });
    expect(m.load_patterns[2]).toMatchObject({ id: "SDL", type: "super-dead" });
    expect(m.load_patterns[2].self_weight).toBeUndefined();
    m = upsertLoadPattern(m, "Dead", { id: "DL", type: "dead", self_weight: 0.5 });
    expect(m.load_patterns[0]).toMatchObject({ id: "DL", self_weight: 0.5 });
    m = upsertLoadPattern(m, "DL", { id: "DL", type: "dead" });
    expect(m.load_patterns[0].self_weight).toBe(0.5);
    expect(() => upsertLoadPattern(m, "DL", { id: "DL", type: "dead", self_weight: Number.NaN })).toThrow(EditError);
    const steel = m.materials[0];
    expect(() => upsertMaterial(m, steel.id, { ...steel, weight: -1 })).toThrow(/0 or more/);
    expect(upsertMaterial(m, steel.id, { ...steel, weight: 0 }).materials[0].weight).toBeUndefined();
    const back = normalizeModel(JSON.parse(JSON.stringify(m)));
    expect(back.load_patterns[0].self_weight).toBe(0.5);
    expect(back.materials[1].weight).toBe(23.5631);
  });
});
