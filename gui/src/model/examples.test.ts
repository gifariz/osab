// The example models open cleanly: no Check Model errors or warnings (F5 runs at once), objects named
// and labelled as the app does, with the defaults of drawn objects, texts in both languages, distinct file names.

import { describe, expect, it } from "vitest";

import { en } from "../i18n/en";
import { id } from "../i18n/id";
import { checkModel } from "./check";
import { addArea } from "./areaOps";
import { EXAMPLES, exampleModel } from "./examples";
import { addFrame, addNode } from "./ops";

describe("example models", () => {
  it.each(EXAMPLES.map((e) => [e.id, e] as const))("%s opens and runs without a Check Model stop", (_, ex) => {
    const m = exampleModel(ex);
    expect(m.nodes.length).toBeGreaterThan(3);
    expect(checkModel(m).filter((i) => i.level !== "note")).toEqual([]);
    for (const n of m.nodes) expect([n.id, n.label]).toEqual([expect.stringMatching(/^\d+$/), expect.stringMatching(/^J\d+$/)]);
    for (const f of m.frames) expect([f.id, f.label]).toEqual([expect.stringMatching(/^M\d+$/), expect.stringMatching(/^[CBD]\d+$/)]);
    for (const a of m.areas) expect([a.id, a.label]).toEqual([expect.stringMatching(/^A\d+$/), expect.stringMatching(/^[FW]\d+$/)]);
    for (const key of [ex.title, ex.description]) {
      expect(en[key]).toBeTruthy();
      expect(id[key]).toBeTruthy();
    }
  });

  // Each object as the app would draw it: the same joints and section in the model without that object. So a
  // new default of drawn objects that the examples miss fails here.
  it.each(EXAMPLES.map((e) => [e.id, e] as const))("%s: every object has the defaults of the same object drawn", (_, ex) => {
    const m = exampleModel(ex);
    const same = (o: object, drawn: object, skip: string[]) => {
      for (const [k, v] of Object.entries(drawn)) if (!skip.includes(k)) expect([o, k, (o as Record<string, unknown>)[k]]).toEqual([o, k, v]);
    };
    for (const f of m.frames.filter((x) => x.section)) {
      const without = { ...m, frames: m.frames.filter((x) => x.id !== f.id) };
      const { model, id } = addFrame(without, f.nodes[0], f.nodes[1], { type: f.type, material: f.material, section: f.section });
      same(f, model.frames.find((x) => x.id === id)!, ["id", "nodes"]);
    }
    for (const a of m.areas.filter((x) => x.section && !x.opening)) {
      const without = { ...m, areas: m.areas.filter((x) => x.id !== a.id) };
      const { model, id } = addArea(without, a.nodes, a.section);
      same(a, model.areas.find((x) => x.id === id)!, ["id", "nodes"]);
    }
    if (m.dimension !== "3D") return;
    for (const n of m.nodes) {
      const without = { ...m, nodes: m.nodes.filter((x) => x.id !== n.id) };
      const { model, id } = addNode(without, [n.x, n.y, n.z]);
      same(n, model.nodes.find((x) => x.id === id)!, ["id", "x", "y", "z"]);
    }
  });

  it("the L-shaped floor: automatic end offsets, beams at 8, floors at the top in the semi-rigid D1", () => {
    const m = exampleModel(EXAMPLES.find((e) => e.id === "b76")!);
    const beams = m.frames.filter((f) => f.label?.startsWith("B"));
    const floors = m.areas.filter((a) => a.label?.startsWith("F") && a.section && !a.opening);
    expect(beams.length).toBeGreaterThan(0);
    expect(floors.length).toBeGreaterThan(0);
    for (const f of m.frames) expect(f.end_offsets).toBe("auto");
    for (const f of beams) expect(f.insertion).toEqual({ point: 8 });
    for (const a of floors) expect([a.insertion, a.diaphragm]).toEqual([{ point: "top" }, "D1"]);
    expect(m.diaphragms).toEqual([{ id: "D1", semi_rigid: true }]);
  });

  it("have distinct ids and file names", () => {
    expect(new Set(EXAMPLES.map((e) => e.id)).size).toBe(EXAMPLES.length);
    expect(new Set(EXAMPLES.map((e) => e.file)).size).toBe(EXAMPLES.length);
  });
});
