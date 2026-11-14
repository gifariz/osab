import { describe, expect, it } from "vitest";

import { areaAxes, areaOf, insidePoints, isFloor, labelPoint, shapeProblem, triangulate } from "./area";
import { addArea, addAreaAt, areaPoints, areasUsing, assignArea, deleteAreaSection, divideAreas, setAreaLoads, setAreaMesh, setOpening, upsertAreaSection } from "./areaOps";
import { checkModel } from "./check";
import { EditError, addNode, copyDefinition, deleteMaterial, deleteObjects, moveNode, newModel, normalizeModel, setMeshSettings, setNodeLoads, setRestraints, upsertMaterial } from "./ops";
import type { Model, Vec3 } from "./types";


function withJoints(points: Vec3[]): { m: Model; ids: string[] } {
  let m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
  const ids: string[] = [];
  for (const p of points) {
    const r = addNode(m, p);
    m = r.model;
    ids.push(r.id);
  }
  return { m, ids };
}

describe("area operations", () => {
  it("adds floors counterclockwise from above and walls as drawn, from the first lowest corner", () => {
    const { m, ids } = withJoints([[0, 0, 4], [0, 6, 4], [6, 6, 4], [6, 0, 4]]);
    const f = addArea(m, ids, "Slab1");
    expect(f.id).toBe("A1");
    expect(f.model.areas[0].nodes).toEqual([ids[0], ids[3], ids[2], ids[1]]);

    const w = addAreaAt(f.model, [[6, 0, 4], [0, 0, 4], [0, 0, 0], [6, 0, 0]], "Wall1");
    expect(w.id).toBe("A2");
    const wall = w.model.areas[1];
    const p = wall.nodes.map((n) => w.model.nodes.find((x) => x.id === n)!).map((n) => [n.x, n.y, n.z]);
    // Started at the first lowest corner (0, 0, 0), direction kept.
    expect(p).toEqual([[0, 0, 0], [6, 0, 0], [6, 0, 4], [0, 0, 4]]);
    expect(() => addArea(w.model, ids, "Slab1")).toThrow(EditError);
    expect(() => addArea(w.model, [ids[0], ids[0], ids[1], ids[2]], "Slab1")).toThrow(EditError);
    expect(() => addArea(m, ids, "Nope")).toThrow(EditError);
    // A non-convex quad is fine when meshed (the default), not as one element.
    const dart: Vec3[] = [[0, 0, 0], [4, 0, 0], [1, 1, 0], [0, 4, 0]];
    expect(addAreaAt(m, dart, "Slab1").id).toBe("A1");
    const one = setAreaMesh(addAreaAt(m, dart, "Slab1").model, ["A1"], "none");
    expect(checkModel(one).find((i) => i.level === "error" && i.areas?.includes("A1"))?.message).toMatch(/convex/);
  });

  it("adds triangles and divides them into n x n triangles", () => {
    const a = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [0, 6, 4], [6, 0, 4]], "Slab1");
    expect(a.id).toBe("A1");
    // Given clockwise from above, stored counterclockwise (local 3 up).
    const pts = a.model.areas[0].nodes.map((n) => a.model.nodes.find((x) => x.id === n)!).map((n) => [n.x, n.y, n.z] as Vec3);
    expect(areaAxes(pts)[2][2]).toBeCloseTo(1);
    let m = setAreaLoads(a.model, "Dead", ["A1"], { dir: "Z", q: -2 }, "add");
    const d = divideAreas(m, ["A1"], 3, 1);
    m = d.model;
    expect(d.ids).toHaveLength(9);
    expect(m.nodes).toHaveLength(10);
    expect(m.areas.every((ar) => ar.nodes.length === 3)).toBe(true);
    expect(m.load_patterns[0].area_loads).toHaveLength(9);
    const total = m.areas.reduce((s, ar) => s + areaOf(ar.nodes.map((n) => m.nodes.find((x) => x.id === n)!).map((n) => [n.x, n.y, n.z] as Vec3)), 0);
    expect(total).toBeCloseTo(18);
    for (const ar of m.areas) {
      const p = ar.nodes.map((n) => m.nodes.find((x) => x.id === n)!).map((n) => [n.x, n.y, n.z] as Vec3);
      expect(areaAxes(p)[2][2]).toBeCloseTo(1);
    }
    expect(checkModel(m).some((i) => i.message.includes("edges of areas"))).toBe(false);
  });

  it("divides areas with shared joints and keeps loads", () => {
    const a = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1");
    let m = setAreaLoads(a.model, "Dead", ["A1"], { dir: "Z", q: -5 }, "add");
    const d = divideAreas(m, ["A1"], 2, 3);
    m = d.model;
    expect(d.ids).toHaveLength(6);
    expect(m.areas).toHaveLength(6);
    expect(m.nodes).toHaveLength(12);
    expect(m.load_patterns[0].area_loads).toHaveLength(6);
    expect(m.load_patterns[0].area_loads.every((l) => l.q === -5)).toBe(true);
    expect(() => divideAreas(m, ["A2"], 0, 2)).toThrow(EditError);
    // Every piece is a floor with local 3 up.
    for (const ar of m.areas) {
      const p = ar.nodes.map((n) => m.nodes.find((x) => x.id === n)!).map((n) => [n.x, n.y, n.z] as Vec3);
      expect(areaAxes(p)[2][2]).toBeCloseTo(1);
    }
  });

  it("assigns sections, angles and reverses local 3", () => {
    const a = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1");
    let m = assignArea(a.model, ["A1"], { section: "Wall1", angle: 30, reverse: true });
    expect(m.areas[0]).toMatchObject({ section: "Wall1", angle: 30 });
    expect(m.areas[0].nodes).toEqual([a.model.areas[0].nodes[0], ...a.model.areas[0].nodes.slice(1).reverse()]);
    m = assignArea(m, ["A1"], { angle: 0 });
    expect(m.areas[0].angle).toBeUndefined();
    expect(() => assignArea(m, ["A1"], { section: "Nope" })).toThrow(EditError);
  });

  it("area loads: add sums, replace and delete", () => {
    const a = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1");
    let m = setAreaLoads(a.model, "Dead", ["A1"], { dir: "Z", q: -2 }, "add");
    m = setAreaLoads(m, "Dead", ["A1"], { dir: "Z", q: -3 }, "add");
    m = setAreaLoads(m, "Dead", ["A1"], { dir: "X", q: 1 }, "add");
    expect(m.load_patterns[0].area_loads).toEqual([
      { area: "A1", dir: "Z", q: -5 },
      { area: "A1", dir: "X", q: 1 },
    ]);
    m = setAreaLoads(m, "Dead", ["A1"], { dir: "3", q: -1 }, "replace");
    expect(m.load_patterns[0].area_loads).toEqual([{ area: "A1", dir: "3", q: -1 }]);
    m = setAreaLoads(m, "Dead", ["A1"], null, "delete");
    expect(m.load_patterns[0].area_loads).toEqual([]);
  });

  it("area sections: add, rename, copy, in-use checks and material links", () => {
    let m = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    m = upsertAreaSection(m, "Slab1", { id: "S250", type: "shell-thin", material: "Concrete", thickness: 0.25 });
    expect(m.areas[0].section).toBe("S250");
    expect(() => upsertAreaSection(m, null, { id: "X", type: "membrane", material: "Nope", thickness: 0.2 })).toThrow(EditError);
    expect(() => upsertAreaSection(m, null, { id: "X", type: "membrane", material: "Concrete", thickness: 0 })).toThrow(EditError);
    expect(() => upsertAreaSection(m, null, { id: "Wall1", type: "membrane", material: "Concrete", thickness: 0.2 })).toThrow(EditError);
    expect(() => deleteAreaSection(m, "S250")).toThrow(EditError);
    expect(deleteAreaSection(m, "Wall1").area_sections.map((s) => s.id)).toEqual(["S250", "Deck1"]);
    expect(copyDefinition(m, "areaSection", "S250").id).toBe("S250-copy");
    expect(areasUsing(m, "areaSection", "S250")).toEqual(["A1"]);
    expect(areasUsing(m, "material", "Concrete")).toEqual(["A1"]);
    expect(() => deleteMaterial(m, "Concrete")).toThrow(EditError);
    m = upsertMaterial(m, "Concrete", { id: "C30", E: 30e6, nu: 0.2 });
    expect(m.area_sections.every((s) => s.material === "C30")).toBe(true);
  });

  it("one-way load distribution: kept on Membrane sections only, also through open", () => {
    let m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
    m = upsertAreaSection(m, null, { id: "P", type: "membrane", material: "Concrete", thickness: 0.2, one_way: true });
    m = upsertAreaSection(m, null, { id: "T", type: "shell-thin", material: "Concrete", thickness: 0.2, one_way: true });
    expect(m.area_sections.find((s) => s.id === "P")!.one_way).toBe(true);
    expect(m.area_sections.find((s) => s.id === "T")!.one_way).toBeUndefined();
    const again = normalizeModel(JSON.parse(JSON.stringify(m)));
    expect(again.area_sections.find((s) => s.id === "P")!.one_way).toBe(true);
    m = upsertAreaSection(m, "P", { id: "P", type: "membrane", material: "Concrete", thickness: 0.2 });
    expect(m.area_sections.find((s) => s.id === "P")!.one_way).toBeUndefined();
  });

  it("deleting a joint deletes its areas and their loads; moving a joint keeps areas flat", () => {
    let m = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    m = setAreaLoads(m, "Dead", ["A1"], { dir: "Z", q: -5 }, "add");
    expect(() => moveNode(m, m.areas[0].nodes[0], [0, 0, 5])).toThrow(/plane/);
    const d = deleteObjects(m, [m.areas[0].nodes[0]], []);
    expect(d.areas).toEqual([]);
    expect(d.load_patterns[0].area_loads).toEqual([]);
    expect(deleteObjects(m, [], [], [], ["A1"]).areas).toEqual([]);
  });

  it("reads area data from files", () => {
    const m = normalizeModel({
      nodes: [{ id: 1 }],
      area_sections: [{ id: "S", type: "membrane", material: "C", thickness: "0.2" }],
      areas: [{ id: 1, nodes: [1, 2, 3, 4], section: "S", angle: 0 }],
      load_cases: [{ id: "D", area_loads: [{ area: 1, dir: "Z", q: -1 }] }],
    });
    expect(m.area_sections).toEqual([{ id: "S", type: "membrane", material: "C", thickness: 0.2, property_type: "slab" }]);
    expect(m.areas).toEqual([{ id: "1", nodes: ["1", "2", "3", "4"], section: "S" }]);
    expect(m.load_patterns[0].area_loads).toEqual([{ area: "1", dir: "Z", q: -1 }]);
    expect(normalizeModel({}).areas).toEqual([]);
  });

  it("gives older files' area sections a property type: wall if only walls use it, else slab", () => {
    const nodes = [
      [1, 0, 0, 0], [2, 4, 0, 0], [3, 4, 4, 0], [4, 0, 4, 0], [5, 4, 0, 3], [6, 0, 0, 3],
    ].map(([id, x, y, z]) => ({ id, x, y, z }));
    const sec = (id: string) => ({ id, type: "shell-thin", material: "C", thickness: 0.2 });
    const m = normalizeModel({
      nodes,
      area_sections: [sec("F"), sec("W"), sec("B"), sec("U"), { ...sec("K"), property_type: "slab", slab_type: "drop" }, { ...sec("L"), property_type: "wall", slab_type: "stiff" }],
      areas: [
        { id: "f", nodes: [1, 2, 3, 4], section: "F" },
        { id: "w", nodes: [1, 2, 5, 6], section: "W" },
        { id: "b1", nodes: [1, 2, 3, 4], section: "B" },
        { id: "b2", nodes: [1, 2, 5, 6], section: "B" },
        { id: "k", nodes: [1, 2, 5, 6], section: "K" },
      ],
    });
    expect(Object.fromEntries(m.area_sections.map((s) => [s.id, s.property_type]))).toEqual({ F: "slab", W: "wall", B: "slab", U: "slab", K: "slab", L: "wall" });
    // A stated type is kept; a wall section has no slab type.
    expect(m.area_sections.find((s) => s.id === "K")?.slab_type).toBe("drop");
    expect(m.area_sections.find((s) => s.id === "L")?.slab_type).toBeUndefined();
    // Saving and reading again keeps every type.
    expect(normalizeModel(JSON.parse(JSON.stringify(m))).area_sections).toEqual(m.area_sections);
  });

  it("a wall section keeps no slab type; new models have Slab1 (slab), Deck1 (deck) and Wall1 (wall)", () => {
    const m0 = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
    expect(m0.area_sections.map((s) => [s.id, s.property_type])).toEqual([["Slab1", "slab"], ["Deck1", "deck"], ["Wall1", "wall"]]);
    const m = upsertAreaSection(m0, null, { id: "W2", type: "shell-thin", material: "Concrete", thickness: 0.3, property_type: "wall", slab_type: "stiff" });
    expect(m.area_sections.find((s) => s.id === "W2")).toEqual({ id: "W2", type: "shell-thin", material: "Concrete", thickness: 0.3, property_type: "wall" });
  });
});

describe("area checks", () => {
  const slab = () => addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 0], [6, 0, 0], [6, 6, 0], [0, 6, 0]], "Slab1").model;

  it("Check Model notes a wall section on a floor and a slab section on a wall (any type is allowed)", () => {
    const m0 = slab();
    const m1 = assignArea(m0, [m0.areas[0].id], { section: "Wall1" });
    const issues = checkModel(m1);
    const note = issues.find((i) => i.message.startsWith("Floors with a wall section"));
    expect(note?.level).toBe("note");
    expect(note?.areas).toEqual([m0.areas[0].id]);
    expect(issues.some((i) => i.message.startsWith("Walls with a slab section"))).toBe(false);
    expect(checkModel(m0).some((i) => i.message.includes("wall section"))).toBe(false);
  });

  it("an area counts as a member and its joints as used", () => {
    const issues = checkModel(slab()).map((i) => i.message);
    expect(issues.some((s) => s.includes("no members"))).toBe(false);
    expect(issues.some((s) => s.includes("not connected"))).toBe(false);
  });

  it("warns about poor shapes and loads out of a plane frame; joints on area edges are tied, so no message", () => {
    let m = addNode(slab(), [3, 0, 0]).model;
    expect(checkModel(m).some((i) => i.nodes?.includes(m.nodes.at(-1)!.id) && !i.message.includes("not connected to any"))).toBe(false);
    m = addAreaAt(m, [[10, 0, 0], [40, 0, 0], [40, 1, 0], [10, 1, 0]], "Slab1").model;
    // Only an area analysed as one element can be poorly shaped; a meshed one gets good elements.
    expect(checkModel(m).some((i) => i.message.includes("poor shape"))).toBe(false);
    expect(checkModel(setAreaMesh(m, ["A2"], "none")).some((i) => i.message.includes("poor shape") && i.areas?.includes("A2"))).toBe(true);
    m = { ...slab(), dimension: "2D" };
    expect(checkModel(m).some((i) => i.message.includes("X-Z plane: A1"))).toBe(true);
  });
});

// An L-shaped floor 6 x 6 m with a 3 x 3 m corner cut away (six corners, counterclockwise from above).
const L: Vec3[] = [[0, 0, 4], [6, 0, 4], [6, 3, 4], [3, 3, 4], [3, 6, 4], [0, 6, 4]];
const meshed = (m: Model): Model => setMeshSettings(m, {});
const blank = () => newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });

describe("polygon areas (any number of corners when meshed)", () => {
  it("shape rules: meshed areas may be any flat polygon whose edges do not cross", () => {
    expect(shapeProblem(L, true)).toBeNull();
    expect(shapeProblem(L)).toMatch(/6 corners/);
    expect(shapeProblem([[0, 0, 0], [4, 0, 0], [1, 1, 0], [0, 4, 0]], true)).toBeNull(); // not convex, fine when meshed
    expect(shapeProblem([[0, 0, 0], [6, 0, 0], [6, 6, 0], [0, 6, 0], [3, -2, 0]], true)).toMatch(/cross/);
    expect(shapeProblem([...L.slice(0, 5), [0, 6, 4.5]], true)).toMatch(/plane/);
    expect(shapeProblem([[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0], [4, 0, 0]], true)).toMatch(/one line/);
  });

  it("axes, area, drawing triangles, label and load points of an L-shape", () => {
    const ax = areaAxes(L);
    expect(ax[2][2]).toBeCloseTo(1);
    expect(ax[0][0]).toBeCloseTo(1);
    expect(areaOf(L)).toBeCloseTo(27);
    const tris = triangulate(L);
    expect(tris).toHaveLength(4);
    expect(tris.reduce((s, t) => s + areaOf(t.map((k) => L[k])), 0)).toBeCloseTo(27);
    // The label sits on the floor, not in the cut-away corner.
    const at = labelPoint(L);
    expect(at[0] < 3 || at[1] < 3).toBe(true);
    expect(insidePoints(L).every((p) => p[0] < 3 || p[1] < 3)).toBe(true);
    // A rectangle given with six corners has the axes of the four-corner one.
    expect(areaAxes([[0, 0, 0], [3, 0, 0], [6, 0, 0], [6, 4, 0], [3, 4, 0], [0, 4, 0]])).toEqual(areaAxes([[0, 0, 0], [6, 0, 0], [6, 4, 0], [0, 4, 0]]));
    expect(isFloor(L)).toBe(true);
  });

  it("adds polygons; an older file with meshing off sets every area to no mesh", () => {
    expect(addAreaAt(blank(), L, "Slab1").model.areas[0].nodes).toHaveLength(6); // meshed by default
    let m = addAreaAt(meshed(blank()), [...L].reverse(), "Slab1").model;
    expect(m.areas[0].nodes).toHaveLength(6);
    // Floors are stored counterclockwise from above.
    expect(areaAxes(areaPoints(m, m.areas[0]))[2][2]).toBeCloseTo(1);
    expect(() => divideAreas(m, ["A1"], 2, 2)).toThrow(/more than four corners/);
    // An older file with meshing off for the whole model: every area is set to no mesh.
    const old = normalizeModel({ ...JSON.parse(JSON.stringify(m)), mesh: "off" });
    expect(old.mesh).toBeUndefined();
    expect(old.areas.map((a) => a.mesh)).toEqual(["none"]);
    // A polygon set to "no mesh" cannot be one element: the mesh step meshes it with the model's settings,
    // and Check Model warns.
    m = setAreaMesh(setMeshSettings(m, {}), ["A1"], "none");
    const issues = checkModel(m).filter((i) => i.areas?.includes("A1"));
    expect(issues.some((i) => i.level === "error")).toBe(false);
    expect(issues.find((i) => i.level === "warning")?.message).toMatch(/no auto mesh with more than 4 corners: A1/);
    // An opening may cut it, as it is meshed.
    const o = addAreaAt(m, [[1, 1, 4], [2, 1, 4], [2, 2, 4], [1, 2, 4]], "Slab1", { opening: true }).model;
    expect(checkModel(o).some((i) => i.level === "error" && i.areas?.length)).toBe(false);
  });

  it("mesh options per area and for the model, kept in the file", () => {
    let m = addAreaAt(meshed(blank()), L, "Slab1").model;
    m = setAreaMesh(m, ["A1"], { size: 0.5 }, true);
    expect(m.areas[0]).toMatchObject({ mesh: { size: 0.5 }, edge_restraints: true });
    m = setMeshSettings(m, { floor: "rectangular", floor_size: 1 });
    const back = normalizeModel(JSON.parse(JSON.stringify(m)));
    expect(back.mesh).toEqual({ floor: "rectangular", floor_size: 1 });
    expect(back.areas[0]).toMatchObject({ mesh: { size: 0.5 }, edge_restraints: true });
    expect(setAreaMesh(m, ["A1"], "default").areas[0]).not.toHaveProperty("mesh");
    expect(normalizeModel({ mesh: "off" }).mesh).toBeUndefined();
    // Grading: missing = on, only "false" is kept.
    expect(normalizeModel({ mesh: { refine: false } }).mesh).toEqual({ refine: false });
    expect(normalizeModel({ mesh: { refine: true } }).mesh).toEqual({});
    expect(normalizeModel({}).mesh).toBeUndefined();
    expect(() => setAreaMesh(m, ["A1"], { size: 0 })).toThrow(EditError);
    expect(() => setMeshSettings(m, { wall_size: -1 })).toThrow(EditError);
  });
});

describe("openings, null areas and overlaps", () => {
  // A floor with its corners fixed, so that only the area checks speak.
  const floor = (m: Model) => {
    const r = addAreaAt(m, [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1");
    return setRestraints(r.model, r.model.areas.find((a) => a.id === r.id)!.nodes, [true, true, true, true, true, true]);
  };
  const hole: Vec3[] = [[2, 2, 4], [3, 2, 4], [3, 3, 4], [2, 3, 4]];

  it("openings have no section and no loads and need a meshed floor", () => {
    let m = addAreaAt(floor(blank()), hole, "Slab1", { opening: true }).model;
    expect(m.areas[1]).toEqual({ id: "A2", nodes: m.areas[1].nodes, section: "", opening: true });
    m = setAreaLoads(m, "Dead", ["A1", "A2"], { dir: "Z", q: -5 }, "add");
    expect(m.load_patterns[0].area_loads.map((l) => l.area)).toEqual(["A1"]);
    expect(assignArea(m, ["A2"], { section: "Slab1" }).areas[1].section).toBe("");
    m = meshed(m);
    expect(checkModel(m).some((i) => i.level === "error")).toBe(false);
    // Outside every floor, or in an unmeshed one: errors (the mesh step's rules).
    const out = addAreaAt(meshed(blank()), hole, "", { opening: true }).model;
    expect(checkModel(out).some((i) => i.level === "error" && /A1 does not lie in a floor/.test(i.message))).toBe(true);
    expect(checkModel(setAreaMesh(m, ["A1"], "none")).find((i) => i.level === "error")?.message).toMatch(/not meshed/);
    // Loads on an opening from a file.
    const loaded = { ...m, load_patterns: [{ ...m.load_patterns[0], area_loads: [{ area: "A2", dir: "Z" as const, q: -1 }] }] };
    expect(checkModel(loaded).find((i) => i.level === "error")?.message).toMatch(/no loads/);
    // Switched off, an opening is a null area; switched on, its loads go.
    const nullArea = setOpening(m, ["A2"], false).areas[1];
    expect(nullArea).toEqual({ id: "A2", nodes: nullArea.nodes, section: "" });
    expect(setOpening(m, ["A1"], true).load_patterns[0].area_loads).toEqual([]);
  });

  it("null areas keep their loads, do not count as overlaps; files keep openings and null objects", () => {
    let m = addAreaAt(floor(meshed(blank())), hole, "").model;
    m = setAreaLoads(m, "Dead", ["A2"], { dir: "Z", q: -2 }, "add");
    expect(m.load_patterns[0].area_loads).toEqual([{ area: "A2", dir: "Z", q: -2 }]);
    expect(checkModel(m).some((i) => i.level === "note")).toBe(false);
    const raw = { areas: [{ id: "O1", nodes: [1, 2, 3], opening: true, section: "X" }, { id: "N1", nodes: [1, 2, 3] }] };
    expect(normalizeModel(raw).areas).toEqual([
      { id: "O1", nodes: ["1", "2", "3"], section: "", opening: true },
      { id: "N1", nodes: ["1", "2", "3"], section: "" },
    ]);
  });

  it("overlaps: the one inside wins, then Stiff over Drop over Slab, then the smaller (a note); errors for unmeshed areas", () => {
    let m = upsertAreaSection(meshed(blank()), null, { id: "DROP", type: "shell-thin", material: "Concrete", thickness: 0.3, slab_type: "drop" });
    m = floor(m);
    m = addAreaAt(m, hole, "Slab1").model; // inside A1
    m = addAreaAt(m, [[5, 5, 4], [8, 5, 4], [8, 8, 4], [5, 8, 4]], "DROP").model; // partly over A1
    const note = checkModel(m).find((i) => i.level === "note")!;
    expect(note.message).toMatch(/A2 over A1 \(inside it\)/);
    expect(note.message).toMatch(/A3 over A1 \(slab type/);
    expect(checkModel(m).some((i) => i.level === "error")).toBe(false);
    expect(checkModel(setAreaMesh(m, ["A2"], "none")).find((i) => i.level === "error")?.message).toMatch(/A2 is not meshed, so it cannot overlap area A1/);
    // Both not meshed: both are analysed in full there (a note); A3 (meshed) over unmeshed A1 stays an error.
    const both = checkModel(setAreaMesh(m, ["A1", "A2"], "none"));
    expect(both.find((i) => i.level === "note" && /analysed in full/.test(i.message))?.message).toMatch(/A1\/A2/);
    expect(both.filter((i) => i.level === "error").map((i) => i.message)).toEqual([expect.stringMatching(/A1 is not meshed, so it cannot overlap area A3/)]);
    // Walls in other planes do not overlap the floor.
    const w = addAreaAt(m, [[0, 0, 0], [6, 0, 0], [6, 0, 4], [0, 0, 4]], "Wall1").model;
    expect(checkModel(w).find((i) => i.level === "note")!.message).not.toMatch(/A4/);
  });

  it("a joint with only a joint load on a floor is not loose (its load goes into the floor)", () => {
    let m = floor(blank());
    const r = addNode(m, [2, 2, 4]);
    m = r.model;
    expect(checkModel(m).some((i) => i.message.includes("not connected to any") && i.nodes?.includes(r.id))).toBe(true);
    m = setNodeLoads(m, "Dead", [r.id], { fz: -10 }, "add");
    expect(checkModel(m).some((i) => i.message.includes("not connected to any") && i.nodes?.includes(r.id))).toBe(false);
  });
});

describe("slab types", () => {
  it("keeps Drop and Stiff through edits and file loads, and drops the default Slab", () => {
    let m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
    m = upsertAreaSection(m, null, { id: "RZ", type: "shell-thin", material: "Concrete", thickness: 0.3, slab_type: "stiff" });
    m = upsertAreaSection(m, null, { id: "DP", type: "shell-thin", material: "Concrete", thickness: 0.3, slab_type: "drop" });
    m = upsertAreaSection(m, null, { id: "SL", type: "shell-thin", material: "Concrete", thickness: 0.3, slab_type: "slab" });
    const back = normalizeModel(JSON.parse(JSON.stringify(m)));
    const of = (id: string) => back.area_sections.find((s) => s.id === id)!;
    expect(of("RZ").slab_type).toBe("stiff");
    expect(of("DP").slab_type).toBe("drop");
    expect("slab_type" in of("SL")).toBe(false);
  });
});
