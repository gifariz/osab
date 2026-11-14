import { describe, expect, it } from "vitest";

import { addAreaAt } from "./areaOps";
import { addFrame, addLink, addNode, newModel, normalizeModel, setNodeLoads, setRestraints, setSprings } from "./ops";
import { namesByKind, renameObjects } from "./relabel";
import type { Model, Vec3 } from "./types";

// A model as old files have it: plain numbers for joints and members.
function oldStyle(): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
  const node = (p: Vec3) => {
    const r = addNode(m, p);
    m = r.model;
    return r.id;
  };
  const [a, b, c, d] = [node([0, 0, 0]), node([0, 0, 4]), node([6, 0, 4]), node([6, 6, 4])];
  const props = { type: "frame" as const, material: "Steel", section: "FSEC1" };
  m = addFrame(m, a, b, props).model;
  m = addFrame(m, b, c, props).model;
  m = addFrame(m, a, c, props).model;
  m = addLink(m, c, d).model;
  m = addAreaAt(m, [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
  m = setRestraints(m, [a], [true, true, true, true, true, true]);
  m = setSprings(m, [d], [1000, 0, 0, 0, 0, 0], "replace");
  m = setNodeLoads(m, "Dead", [b], { fx: 5 }, "add");
  m.load_patterns[0].frame_loads.push({ type: "uniform", frame: m.frames[1].id, dir: "Z", w: -2 });
  m.load_patterns[0].area_loads.push({ area: m.areas[0].id, dir: "Z", q: -3 });
  m.masses = [{ node: d, ux: 2 }];
  // Old names: joints 1, 2…, members 1, 2, 3, link 4, area 1.
  const old = (ids: string[]) => new Map(ids.map((id, k) => [id, String(k + 1)]));
  return renameObjects(m, { nodes: old(m.nodes.map((n) => n.id)), frames: old(m.frames.map((f) => f.id)), links: new Map([[m.links[0].id, "4"]]), areas: old(m.areas.map((x) => x.id)) });
}

describe("renaming objects", () => {
  it("names every object as the app does and carries the names into everything that refers to them", () => {
    const m = oldStyle();
    // Old names, apart from joints already numbered: members 1, 2, 3, link 4, area 1.
    const old = renameObjects(m, { nodes: new Map(m.nodes.map((n, k) => [n.id, "N" + (k + 1)])), frames: new Map(), links: new Map(), areas: new Map() });
    expect(old.frames.map((f) => [f.id, ...f.nodes])).toEqual([["1", "N1", "N2"], ["2", "N2", "N3"], ["3", "N1", "N3"]]);
    const r = renameObjects(old, namesByKind(old));
    expect(r.nodes.map((n) => n.id)).toEqual(m.nodes.map((_, k) => `${k + 1}`));
    expect(r.frames.map((f) => [f.id, ...f.nodes])).toEqual([["M1", "1", "2"], ["M2", "2", "3"], ["M3", "1", "3"]]);
    expect(r.links).toEqual([{ id: "K1", type: "rigid", nodes: ["3", "4"] }]);
    expect(r.areas[0].id).toBe("A1");
    expect(r.areas[0].nodes).toEqual(old.areas[0].nodes.map((n) => n.slice(1)));
    expect(r.supports.map((s) => s.node)).toEqual(["1"]);
    expect(r.springs.map((s) => s.node)).toEqual(["4"]);
    expect(r.masses).toEqual([{ node: "4", ux: 2 }]);
    const p = r.load_patterns[0];
    expect(p.node_loads).toEqual([{ node: "2", fx: 5 }]);
    expect(p.frame_loads.map((l) => l.frame)).toEqual(["M2"]);
    expect(p.area_loads.map((l) => l.area)).toEqual(["A1"]);
    // Nothing else changes, and the file reads back the same.
    expect({ ...r, nodes: [], frames: [], links: [], areas: [], supports: [], springs: [], masses: [], load_patterns: [] }).toEqual({ ...m, nodes: [], frames: [], links: [], areas: [], supports: [], springs: [], masses: [], load_patterns: [] });
    expect(normalizeModel(JSON.parse(JSON.stringify(r)))).toEqual(r);
  });

  it("refuses names that would clash", () => {
    const m = oldStyle();
    expect(() => renameObjects(m, { nodes: new Map([["1", "2"]]), frames: new Map(), links: new Map(), areas: new Map() })).toThrow(/two joints named 2/);
    expect(() => renameObjects(m, { nodes: new Map(), frames: new Map([["1", "4"]]), links: new Map(), areas: new Map() })).toThrow(/frames or links named 4/);
  });
});
