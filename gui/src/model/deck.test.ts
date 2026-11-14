import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { checkModel } from "./check";
import { deckConcrete, deckDepth, deckProblem, newDeck, sectionWeight } from "./deck";
import { upsertAreaSection } from "./areaOps";
import { EditError, newModel, normalizeModel } from "./ops";
import type { AreaSection, Model } from "./types";

const bench = (name: string): Model => normalizeModel(JSON.parse(readFileSync(new URL(`../../../tests/benchmarks/${name}/model.json`, import.meta.url), "utf8")));

describe("deck sections", () => {
  it("weigh their concrete per area plus the deck (the probe's filled deck: 2.4268 kN/m2)", () => {
    const m = bench("b73_deck_floors");
    const df = m.area_sections.find((s) => s.id === "DF")!;
    expect(deckConcrete(df.deck!)).toBeCloseTo(0.09875, 12);
    expect(sectionWeight(m, df)).toBeCloseTo(0.09875 * 23.563121 + 0.1, 10);
    const ds = m.area_sections.find((s) => s.id === "DS")!;
    expect(sectionWeight(m, ds)).toBeCloseTo(0.15 * 23.563121, 10);
    expect(sectionWeight(m, { ...ds, deck: { ...ds.deck!, type: "unfilled", unit_weight: 0.1 } })).toBeCloseTo(0.1, 12);
    expect(deckDepth(df.deck!)).toBeCloseTo(0.14, 12);
  });

  it("are read back as they were saved: deck data, Membrane, one-way", () => {
    const m = bench("b73_deck_floors");
    const again = normalizeModel(JSON.parse(JSON.stringify(m)));
    expect(again.area_sections).toEqual(m.area_sections);
    expect(m.area_sections.every((s) => s.property_type === "deck" && s.type === "membrane" && s.one_way && s.deck)).toBe(true);
  });

  it("are always a one-way Membrane with the total depth as thickness when saved", () => {
    const m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
    const sec: AreaSection = { id: "D2", type: "shell-thin", material: "Concrete", thickness: 0.3, property_type: "deck", slab_type: "drop", deck: { ...newDeck("Steel"), slab_depth: 0.08 } };
    const s = upsertAreaSection(m, null, sec).area_sections.find((x) => x.id === "D2")!;
    expect(s).toMatchObject({ type: "membrane", one_way: true, property_type: "deck", thickness: 0.08 + 0.075 });
    expect(s.slab_type).toBeUndefined();
    // Turned into a slab, the deck data go.
    const slab = upsertAreaSection(m, null, { ...sec, property_type: "slab" }).area_sections.find((x) => x.id === "D2")!;
    expect(slab.deck).toBeUndefined();
    expect(slab.type).toBe("shell-thin");
  });

  it("need a deck material that exists and sensible values", () => {
    const m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
    const sec: AreaSection = { id: "D2", type: "membrane", material: "Concrete", thickness: 0, property_type: "deck", deck: newDeck("Nope") };
    expect(() => upsertAreaSection(m, null, sec)).toThrow(EditError);
    expect(deckProblem({ ...newDeck("Steel"), rib_spacing: 0 })).toBe("err.deckFilled");
    expect(deckProblem({ ...newDeck("Steel"), type: "unfilled", shear_thickness: 0 })).toBe("err.deckUnfilled");
    expect(deckProblem({ ...newDeck("Steel"), type: "solid", slab_depth: 0 })).toBe("err.deckSolid");
    expect(deckProblem({ ...newDeck("Steel"), rib_depth: -1 })).toBe("err.deckNegative");
    expect(deckProblem(newDeck("Steel"))).toBeNull();
  });

  it("are in a new model (Deck1) and noted by Check Model on a wall", () => {
    const m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
    expect(m.area_sections.find((s) => s.id === "Deck1")?.property_type).toBe("deck");
    const b = bench("b73_deck_floors");
    // Stand the DF floor up: a wall with a deck section gets a note.
    const wall = structuredClone(b);
    for (const n of wall.nodes) if (n.id === "DF.t2" || n.id === "DF.t3") Object.assign(n, { y: 20, z: 7 });
    expect(checkModel(wall).some((i) => i.areas?.includes("DF") && i.level === "note")).toBe(true);
    expect(checkModel(b).some((i) => i.areas?.includes("DF"))).toBe(false);
  });
});
