import { describe, expect, it } from "vitest";

import { firstOfType, guessMaterialType } from "./materials";
import { EditError, deleteMaterial, newModel, normalizeModel, upsertMaterial } from "./ops";

const fresh = () => newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });

describe("material types", () => {
  it("new models have a steel and a concrete material with their strengths", () => {
    const m = fresh();
    expect(m.materials.map((x) => [x.id, x.type, x.fy, x.fu, x.fc])).toEqual([
      ["Steel", "steel", 345e3, 450e3, undefined],
      ["Concrete", "concrete", undefined, undefined, 28e3],
    ]);
    expect(firstOfType(m, "steel")).toBe("Steel");
    expect(firstOfType(m, "other")).toBeUndefined();
  });

  it("files without types get one from E; each type keeps only its own strengths", () => {
    expect(guessMaterialType(200e6)).toBe("steel");
    expect(guessMaterialType(25e6)).toBe("concrete");
    expect(guessMaterialType(70e3)).toBe("other");
    const m = normalizeModel({
      materials: [
        { id: "S", E: 2e8, nu: 0.3 },
        { id: "C", E: 2.5e7, nu: 0.2, fy: 400e3, fc: 30e3 },
        { id: "T", type: "other", E: 1e6, nu: 0.3, fy: 1 },
      ],
    });
    expect(m.materials.map((x) => [x.id, x.type, x.fy, x.fc])).toEqual([
      ["S", "steel", undefined, undefined],
      ["C", "concrete", undefined, 30e3],
      ["T", "other", undefined, undefined],
    ]);
    expect(normalizeModel(JSON.parse(JSON.stringify(m))).materials).toEqual(m.materials);
  });

  it("saving drops the strengths of other types and refuses strengths of 0 or less", () => {
    const m = upsertMaterial(fresh(), "Steel", { id: "Steel", type: "other", E: 200e6, nu: 0.3, fy: 345e3 });
    expect(m.materials[0]).toEqual({ id: "Steel", type: "other", E: 200e6, nu: 0.3 });
    expect(() => upsertMaterial(fresh(), "Steel", { id: "Steel", type: "steel", E: 200e6, nu: 0.3, fy: 0 })).toThrow(EditError);
  });

  it("a deck's steel follows a rename and counts as a use", () => {
    const m = upsertMaterial(fresh(), "Steel", { id: "S355", type: "steel", E: 200e6, nu: 0.3 });
    expect(m.area_sections.find((s) => s.id === "Deck1")?.deck?.steel).toBe("S355");
    expect(() => deleteMaterial(m, "S355")).toThrow(EditError);
  });
});
