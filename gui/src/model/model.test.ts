import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { addAreaAt } from "./areaOps";
import { checkModel } from "./check";
import { withLabels } from "./labels";
import { lineType } from "./objectTypes";
import { gridPoints, letters, makeGrid, parseSpacings } from "./grid";
import {
  EditError,
  MODEL_FORMAT,
  addFrame,
  addLink,
  addNode,
  assignFrame,
  copyDefinition,
  deleteCombination,
  deleteLoadCase,
  deleteObjects,
  membersUsing,
  fixFromFlags,
  moveNode,
  newModel,
  normalizeModel,
  restraintsOf,
  setAngleLimits,
  setDimension,
  setFrameLoads,
  setInMesh,
  setNodeLoads,
  setPlaneFrame,
  setRestraints,
  setSpecial,
  setSprings,
  springsOf,
  upsertCombination,
  upsertLoadPattern,
  setLoadCase,
  deleteLoadPattern,
  setWindLoads,
  setMassSource,
  deleteDiaphragm,
  addDiaphragm,
  setDiaphragmSemiRigid,
  upsertMaterial,
  upsertSection,
  deleteSection,
} from "./ops";
import { iSection, rectangle } from "./sections";
import type { Model, Vec3 } from "./types";

const repo = fileURLToPath(new URL("../../../", import.meta.url));

function portal(): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [4] });
  const pts: Vec3[] = [
    [0, 0, 0],
    [0, 0, 4],
    [6, 0, 4],
    [6, 0, 0],
  ];
  const ids: string[] = [];
  for (const p of pts) {
    const r = addNode(m, p);
    m = r.model;
    ids.push(r.id);
  }
  const props = { type: "frame" as const, material: "Steel", section: "FSEC1" };
  m = addFrame(m, ids[0], ids[1], props).model;
  m = addFrame(m, ids[1], ids[2], props).model;
  m = addFrame(m, ids[2], ids[3], props).model; // right column drawn downwards
  return m;
}

describe("frame end-order rule", () => {

  it("stores new members with end i by the rule, whatever the click order", () => {
    const m = portal();
    expect(m.frames.map((e) => [e.id, ...e.nodes])).toEqual([
      ["M1", "1", "2"],
      ["M2", "2", "3"],
      ["M3", "4", "3"],
    ]);
    // A beam drawn right-to-left still gets end i on the left.
    let r = deleteObjects(m, [], ["M2"]);
    r = addFrame(r, "3", "2", { type: "frame", material: "Steel", section: "FSEC1" }).model;
    expect(r.frames.find((e) => e.id === "M4")!.nodes).toEqual(["2", "3"]);
  });

  it("labels members by type: columns C, beams B, braces D, by the model's angle limits", () => {
    expect(lineType([0, 0, 0], [0, 0, 3])).toBe("column");
    expect(lineType([0, 0, 3], [5, 2, 3])).toBe("beam");
    expect(lineType([0, 0, 0], [4, 0, 3])).toBe("brace");
    // A line 3 degrees off vertical: a brace with the default 1 degree, a column with 5.
    let m = portal();
    const top: Vec3 = [8 + 4 * Math.tan((3 * Math.PI) / 180), 0, 4];
    const j = addNode(addNode(m, [8, 0, 0]).model, top);
    m = j.model;
    const base = m.nodes.find((n) => n.x === 8)!.id;
    const props = { type: "frame" as const, material: "Steel", section: "FSEC1" };
    // Names are M and a number; labels follow the type.
    const added = addFrame(m, base, j.id, props);
    expect(added.id).toBe("M4");
    expect(withLabels(added.model).frames.at(-1)!.label).toBe("D1");
    m = setAngleLimits(m, { column: 5, beam_floor: 20 });
    expect(withLabels(addFrame(m, base, j.id, props).model).frames.at(-1)!.label).toBe("C3");
    expect(() => setAngleLimits(m, { column: 60, beam_floor: 40 })).toThrow(EditError);
    expect(setAngleLimits(m, null).angle_limits).toBeUndefined();
  });
});

describe("trapezoidal frame loads", () => {
  it("adds w1 to w2 over part of a member (fractions or lengths), replaced by type", () => {
    let m = portal();
    m = setFrameLoads(m, "Dead", ["M2"], "trapezoidal", { type: "trapezoidal", dir: "Z", w1: -10, w2: -20, from: 0.25, to: 0.75 }, "add", true);
    expect(m.load_patterns[0].frame_loads).toEqual([{ type: "trapezoidal", frame: "M2", dir: "Z", w1: -10, w2: -20, from: 1.5, to: 4.5 }]);
    m = setFrameLoads(m, "Dead", ["M2"], "uniform", { type: "uniform", dir: "Z", w: -5 }, "add");
    m = setFrameLoads(m, "Dead", ["M2"], "trapezoidal", { type: "trapezoidal", dir: "Z", w1: 0, w2: -8 }, "replace");
    expect(m.load_patterns[0].frame_loads.map((l) => l.type).sort()).toEqual(["trapezoidal", "uniform"]);
    expect(m.load_patterns[0].frame_loads.find((l) => l.type === "trapezoidal")).toEqual({ type: "trapezoidal", frame: "M2", dir: "Z", w1: 0, w2: -8 });
    expect(() => setFrameLoads(m, "Dead", ["M2"], "trapezoidal", { type: "trapezoidal", dir: "Z", w1: 1, w2: 2, from: 5, to: 9 }, "add")).toThrow(EditError);
  });
});

describe("null frames and Include in mesh", () => {
  it("draws null frames (no section, no material), turns frames null and back", () => {
    let m = portal();
    const r = addFrame(m, "2", "4", { type: "truss", material: "Steel", section: "" });
    expect(r.model.frames.at(-1)).toEqual({ id: "M4", type: "frame", nodes: ["4", "2"], material: "", section: "" });
    m = assignFrame(r.model, ["M2"], { section: "" });
    expect(m.frames.find((f) => f.id === "M2")).toMatchObject({ section: "", material: "" });
    expect(() => assignFrame(m, ["M2"], { section: "FSEC1" })).toThrow(/needs a material/);
    expect(assignFrame(m, ["M2"], { section: "FSEC1", material: "Steel" }).frames.find((f) => f.id === "M2")).toMatchObject({ section: "FSEC1", material: "Steel" });
    // Null frames are not checked as members.
    expect(checkModel(m).some((i) => i.frames?.includes("M2"))).toBe(false);
  });

  it("Include in mesh on joints (default no) and frames (default yes), kept in the file", () => {
    let m = setInMesh(portal(), ["3"], ["M2"], true);
    expect(m.nodes.find((n) => n.id === "3")!.in_mesh).toBe(true);
    expect(m.frames.find((f) => f.id === "M2")!.in_mesh).toBeUndefined();
    m = setInMesh(m, [], ["M2"], false);
    expect(m.frames.find((f) => f.id === "M2")!.in_mesh).toBe(false);
    const back = normalizeModel(JSON.parse(JSON.stringify({ ...m, angle_limits: { column: 2 } })));
    expect(back.nodes.find((n) => n.id === "3")!.in_mesh).toBe(true);
    expect(back.frames.find((f) => f.id === "M2")!.in_mesh).toBe(false);
    expect(back.angle_limits).toEqual({ column: 2, beam_floor: 20 });
    expect(normalizeModel({ frames: [{ id: "N", type: "frame", nodes: [1, 2] }] }).frames[0]).toEqual({ id: "N", type: "frame", nodes: ["1", "2"], material: "", section: "" });
    expect(setInMesh(m, ["3"], ["M2"], undefined).nodes.find((n) => n.id === "3")).not.toHaveProperty("in_mesh");
  });
});

describe("edit operations", () => {
  it("reuses a joint at the same point and refuses bad members", () => {
    let m = portal();
    const r = addNode(m, [6, 0, 4]);
    expect(r.created).toBe(false);
    expect(r.id).toBe("3");
    const props = { type: "frame" as const, material: "Steel", section: "FSEC1" };
    expect(() => addFrame(m, "2", "3", props)).toThrow(EditError);
    expect(() => addFrame(m, "2", "2", props)).toThrow(/two different/);
    expect(() => addFrame(m, "1", "3", { ...props, section: "NOPE" })).toThrow(/section/);
    m = addFrame(m, "1", "3", props).model;
    expect(m.frames.at(-1)!.id).toBe("M4");
  });

  it("deleting a joint takes its members, support and loads with it; joints left alone go too", () => {
    let m = setRestraints(portal(), ["1", "4"], [true, true, true, true, true, true]);
    m = setNodeLoads(m, "Dead", ["2"], { fx: 10 }, "add");
    m = setFrameLoads(m, "Dead", ["M2"], "uniform", { type: "uniform", dir: "Z", w: -15 }, "add");
    const d = deleteObjects(m, ["2"], []);
    // Joint 1 is left with nothing attached (its column went with joint 2): deleted with its support.
    expect(d.nodes.map((n) => n.id)).toEqual(["3", "4"]);
    expect(d.frames.map((e) => e.id)).toEqual(["M3"]);
    expect(d.load_patterns[0].node_loads).toEqual([]);
    expect(d.load_patterns[0].frame_loads).toEqual([]);
    expect(d.supports.map((s) => s.node)).toEqual(["4"]);
    expect(m.nodes).toHaveLength(4); // the input is unchanged
  });

  it("special joints stay when their objects are deleted; others left alone go, whatever is assigned", () => {
    // As measured: plain, supported, sprung, loaded and diaphragm joints go; special ones stay.
    let m = setRestraints(portal(), ["1"], [true, true, true, true, true, true]);
    m = setNodeLoads(m, "Dead", ["4"], { fz: -10 }, "add");
    m = setSpecial(m, ["2"], true);
    expect(m.nodes.find((n) => n.id === "2")!.special).toBe(true);
    const d = deleteObjects(m, [], ["M1", "M2", "M3"]);
    expect(d.nodes.map((n) => n.id)).toEqual(["2"]);
    expect(d.supports).toEqual([]);
    expect(d.load_patterns[0].node_loads).toEqual([]);
    // Deleting one member keeps the joints the others still use.
    expect(deleteObjects(m, [], ["M2"]).nodes.map((n) => n.id)).toEqual(["1", "2", "3", "4"]);
    // Joints drawn alone are special; the flag is kept in the file and can be turned off.
    const lone = addNode(newModel({ xSpacings: [], ySpacings: [], storyHeights: [] }), [1, 2, 0], { free: true });
    expect(lone.model.nodes[0].special).toBe(true);
    expect(normalizeModel(JSON.parse(JSON.stringify(m))).nodes.find((n) => n.id === "2")!.special).toBe(true);
    expect(setSpecial(m, ["2"], false).nodes.find((n) => n.id === "2")!.special).toBeUndefined();
    // As measured: turned off, a joint with nothing attached is deleted at once, with its assignments.
    let free = setRestraints(d, ["2"], [true, true, true, false, false, false]);
    free = setSpecial(free, ["2"], false);
    expect(free.nodes).toEqual([]);
    expect(free.supports).toEqual([]);
  });

  it("stores restraints as fixed, pinned or a list", () => {
    expect(fixFromFlags([true, true, true, true, true, true])).toBe("fixed");
    expect(fixFromFlags([true, true, true, false, false, false])).toBe("pinned");
    expect(fixFromFlags([false, false, true, false, false, false])).toEqual(["uz"]);
    expect(fixFromFlags([false, false, false, false, false, false])).toBeNull();
    let m = setRestraints(portal(), ["1"], [true, false, true, false, true, false]);
    expect(restraintsOf(m, "1")).toEqual([true, false, true, false, true, false]);
    m = setRestraints(m, ["1"], [false, false, false, false, false, false]);
    expect(m.supports).toEqual([]);
  });

  it("adds, replaces and deletes loads", () => {
    let m = portal();
    m = setNodeLoads(m, "Dead", ["2"], { fx: 10 }, "add");
    m = setNodeLoads(m, "Dead", ["2"], { fx: 5, fz: -1 }, "add");
    expect(m.load_patterns[0].node_loads).toEqual([{ node: "2", fx: 15, fz: -1 }]);
    m = setNodeLoads(m, "Dead", ["2"], { fx: 2 }, "replace");
    expect(m.load_patterns[0].node_loads).toEqual([{ node: "2", fx: 2 }]);
    m = setNodeLoads(m, "Dead", ["2"], {}, "delete");
    expect(m.load_patterns[0].node_loads).toEqual([]);

    m = setFrameLoads(m, "Dead", ["M2"], "uniform", { type: "uniform", dir: "Z", w: -15 }, "add");
    m = setFrameLoads(m, "Dead", ["M2"], "point", { type: "point", dir: "Z", p: -5, at: 0.5 }, "add", true);
    expect(m.load_patterns[0].frame_loads).toEqual([
      { type: "uniform", frame: "M2", dir: "Z", w: -15 },
      { type: "point", frame: "M2", dir: "Z", p: -5, at: 3 },
    ]);
    m = setFrameLoads(m, "Dead", ["M2"], "uniform", { type: "uniform", dir: "2", w: -2, from: 0.25, to: 0.75 }, "replace", true);
    expect(m.load_patterns[0].frame_loads).toEqual([
      { type: "point", frame: "M2", dir: "Z", p: -5, at: 3 },
      { type: "uniform", frame: "M2", dir: "2", w: -2, from: 1.5, to: 4.5 },
    ]);
    expect(() => setFrameLoads(m, "Dead", ["M2"], "point", { type: "point", dir: "Z", p: 1, at: 7 }, "add")).toThrow(/within/);
    m = setFrameLoads(m, "Dead", ["M2"], "point", null, "delete");
    expect(m.load_patterns[0].frame_loads).toHaveLength(1);
  });

  it("renames materials, sections and load cases with their references", () => {
    let m = upsertMaterial(portal(), "Steel", { id: "S355", E: 210e6, nu: 0.3 });
    expect(m.frames.every((e) => e.material === "S355")).toBe(true);
    m = upsertSection(m, null, { id: "COL", ...iSection(0.3, 0.3, 0.02, 0.01) });
    m = upsertSection(m, "FSEC1", { ...m.sections[0], id: "BEAM" });
    expect(m.frames.every((e) => e.section === "BEAM")).toBe(true);
    expect(() => deleteSection(m, "BEAM")).toThrow(/used by 3/);
    expect(() => upsertSection(m, null, { id: "COL", ...rectangle(0.3, 0.3) })).toThrow(/already exists/);
    m = upsertLoadPattern(m, null, { id: "WIND", type: "wind" });
    expect(m.load_cases.map((c) => c.id)).toEqual(["Dead", "Live", "WIND"]);
    expect(() => upsertMaterial(m, null, { id: "X", E: 0, nu: 0.3 })).toThrow(/E must/);
  });

  it("moves joints anywhere in 3D, but not onto another joint", () => {
    const m = portal();
    expect(m.dimension).toBe("3D");
    expect(moveNode(m, "3", [6, 0, 5]).nodes.find((n) => n.id === "3")!.z).toBe(5);
    expect(moveNode(m, "3", [6, 1, 4]).nodes.find((n) => n.id === "3")!.y).toBe(1);
    expect(() => moveNode(m, "3", [0, 0, 4])).toThrow(/already at/);
  });

  it("plane frame analysis is an option, checked before running", () => {
    let m = setRestraints(portal(), ["1", "4"], [true, true, false, false, false, false]);
    m = setPlaneFrame(m, true);
    expect(m.dimension).toBe("2D");
    expect(checkModel(m).filter((i) => i.level === "error")).toEqual([]);
    const off = checkModel(moveNode(m, "3", [6, 1, 4])).find((i) => i.message.includes("one X-Z plane"))!;
    expect(off.level).toBe("error");
    expect(off.nodes).toEqual(["3"]);
    m = setNodeLoads(m, "Dead", ["2"], { fy: 1 }, "add");
    m = setFrameLoads(m, "Dead", ["M2"], "uniform", { type: "uniform", dir: "3", w: 1 }, "add");
    const loads = checkModel(m).find((i) => i.message.includes("only loads in the X-Z plane"))!;
    expect(loads.nodes).toEqual(["2"]);
    expect(loads.frames).toEqual(["M2"]);
    expect(setPlaneFrame(m, false).dimension).toBe("3D");
  });

  it("a plane grid (X-Y) keeps UZ, RX, RY: joints at one Z, horizontal areas, vertical loads", () => {
    // B1 alone, supported at both ends, as a grid at z = 4.
    let m = deleteObjects(portal(), ["1", "4"], []);
    m = setRestraints(m, ["2", "3"], [true, true, true, false, false, false]);
    m = setDimension(m, "grid");
    expect(normalizeModel(JSON.parse(JSON.stringify(m))).dimension).toBe("grid");
    m = setNodeLoads(m, "Dead", ["2"], { mx: 2, my: -1 }, "add");
    m = setFrameLoads(m, "Dead", ["M2"], "uniform", { type: "uniform", dir: "Z", w: -5 }, "add");
    expect(checkModel(m).filter((i) => i.level === "error")).toEqual([]);
    const off = checkModel(moveNode(m, "3", [6, 0, 5])).find((i) => i.message.includes("one X-Y plane"))!;
    expect(off.nodes).toEqual(["3"]);
    m = setNodeLoads(m, "Dead", ["3"], { fx: 1 }, "add");
    m = setFrameLoads(m, "Dead", ["M2"], "uniform", { type: "uniform", dir: "1", w: 1 }, "add");
    const loads = checkModel(m).find((i) => i.message.includes("Plane grid analysis takes only vertical loads"))!;
    expect(loads.nodes).toEqual(["3"]);
    expect(loads.frames).toEqual(["M2"]);
    // Walls do not belong in a plane grid.
    const w = addAreaAt(m, [[0, 0, 4], [6, 0, 4], [6, 0, 8], [0, 0, 8]], "Wall1").model;
    expect(checkModel(w).some((i) => i.message.includes("only horizontal areas: A1"))).toBe(true);
  });
});

describe("grid", () => {
  it("reads spacings", () => {
    expect(parseSpacings("6, 4")).toEqual([6, 4]);
    expect(parseSpacings("3*4 2.5")).toEqual([4, 4, 4, 2.5]);
    expect(parseSpacings("")).toEqual([]);
    expect(parseSpacings("4, -1")).toBeNull();
    expect(parseSpacings("abc")).toBeNull();
  });

  it("labels X lines with letters, Y lines with numbers, stories from Base", () => {
    const g = makeGrid([6, 6], [5], [4, 3.5]);
    expect(g.x.map((l) => `${l.label}=${l.coord}`)).toEqual(["A=0", "B=6", "C=12"]);
    expect(g.y.map((l) => l.label)).toEqual(["1", "2"]);
    expect(g.stories.map((s) => `${s.label}=${s.z}`)).toEqual(["Base=0", "Story1=4", "Story2=7.5"]);
    expect(gridPoints(g)).toHaveLength(18);
    expect(letters(25)).toBe("Z");
    expect(letters(26)).toBe("AA");
  });
});

describe("file format version", () => {
  it("an older file (no version) opens; the model carries the current version, so a save writes it", () => {
    const m = normalizeModel({ nodes: [{ id: 1 }] });
    expect(m.version).toBe(MODEL_FORMAT);
    expect(JSON.parse(JSON.stringify(m)).version).toBe(MODEL_FORMAT);
    expect(normalizeModel({ version: MODEL_FORMAT }).version).toBe(MODEL_FORMAT);
  });

  it("a file from a newer OSAB is refused with a message that says so", () => {
    expect(() => normalizeModel({ version: MODEL_FORMAT + 1 })).toThrow(EditError);
    expect(() => normalizeModel({ version: MODEL_FORMAT + 1 })).toThrow(/newer OSAB \(file format 2\)/);
  });
});

describe("files", () => {
  const files = [
    ...readdirSync(join(repo, "examples")).map((f) => join(repo, "examples", f)),
    ...readdirSync(join(repo, "tests/benchmarks")).map((d) => join(repo, "tests/benchmarks", d, "model.json")),
  ];

  it.each(files)("opens and saves %s without losing anything", (file) => {
    const raw = JSON.parse(readFileSync(file, "utf8"));
    const m = normalizeModel(raw);
    expect(normalizeModel(JSON.parse(JSON.stringify(m)))).toEqual(m);
    expect(m.nodes).toHaveLength(raw.nodes.length);
    expect(m.frames.map((e) => e.nodes)).toEqual(raw.frames.map((e: { nodes: unknown[] }) => e.nodes.map(String)));
    // Loads per pattern: a file without patterns has them on its cases (one pattern per case).
    const lists = (x: { frame_loads?: unknown[]; node_loads?: unknown[]; area_loads?: unknown[] }) =>
      [(x.frame_loads ?? []).length, (x.node_loads ?? []).length, (x.area_loads ?? []).length];
    expect(m.load_patterns.map(lists)).toEqual((raw.load_patterns ?? raw.load_cases ?? []).map(lists));
    expect(m.load_cases.map((c) => c.id)).toEqual((raw.load_cases ?? []).map((c: { id: unknown }) => String(c.id)));
    expect(m.springs).toHaveLength((raw.springs ?? []).length);
    expect(m.links.map((l) => [l.id, ...l.nodes])).toEqual((raw.links ?? []).map((l: { id: string; nodes: string[] }) => [l.id, ...l.nodes]));
  });

  it("opens files written before Phase 3a (elements, member_loads)", () => {
    const m = normalizeModel({
      nodes: [{ id: 1 }, { id: 2, x: 4 }],
      elements: [{ id: "B1", type: "frame", nodes: [1, 2], material: "m", section: "s" }],
      load_cases: [{ id: "Dead", member_loads: [{ type: "uniform", element: "B1", dir: "Z", w: -5 }] }],
    });
    expect(m.frames.map((f) => [f.id, ...f.nodes])).toEqual([["B1", "1", "2"]]);
    expect(m.load_patterns[0].frame_loads).toEqual([{ type: "uniform", frame: "B1", dir: "Z", w: -5 }]);
    expect(Object.keys(m)).not.toContain("elements");
  });
});

describe("springs and rigid links", () => {
  it("assigns springs: add, replace, delete", () => {
    let m = setSprings(portal(), ["1", "4"], [30000, 0, 0, 0, 25000, 0], "replace");
    expect(m.springs).toEqual([
      { node: "1", ux: 30000, ry: 25000 },
      { node: "4", ux: 30000, ry: 25000 },
    ]);
    m = setSprings(m, ["1"], [0, 0, 5000, 0, 0, 0], "add");
    expect(springsOf(m, "1")).toEqual([30000, 0, 5000, 0, 25000, 0]);
    m = setSprings(m, ["4"], [0, 0, 0, 0, 0, 0], "delete");
    expect(m.springs.map((s) => s.node)).toEqual(["1"]);
    expect(() => setSprings(m, ["1"], [-1, 0, 0, 0, 0, 0], "replace")).toThrow(EditError);
    // Deleting a joint deletes its springs.
    expect(deleteObjects(m, ["1"], []).springs).toEqual([]);
  });

  it("draws rigid links in the frame end order and refuses loops and two restrained joints", () => {
    let m = portal();
    let r = addLink(m, "3", "2"); // drawn right to left: end i is the lower X
    expect(r.model.links).toEqual([{ id: "K1", type: "rigid", nodes: ["2", "3"] }]);
    m = addNode(r.model, [3, 0, 5]).model;
    r = addLink(m, "3", "5");
    expect(r.id).toBe("K2");
    expect(() => addLink(r.model, "5", "2")).toThrow(/already joined rigidly/);
    expect(() => addLink(r.model, "2", "2")).toThrow(EditError);
    const fixed = [true, true, true, true, true, true];
    expect(() => addLink(setRestraints(r.model, ["1", "2"], fixed), "1", "5")).toThrow(/restraints at one joint only/);
    m = addLink(setRestraints(r.model, ["1"], fixed), "1", "4").model;
    expect(() => setRestraints(m, ["4"], [false, false, true, false, false, false])).toThrow(/restraints at one joint only/);
    // Deleting a joint deletes its links.
    expect(deleteObjects(m, ["3"], []).links.map((l) => l.id)).toEqual(["K3"]);
  });

  it("pre-run checks accept springs as supports and links as connections", () => {
    let m = setSprings(portal(), ["1", "4"], [1e4, 1e4, 1e4, 1e4, 1e4, 1e4], "replace");
    m = addNode(m, [7, 0, 4]).model;
    m = addLink(m, "3", "5").model;
    const issues = checkModel(m);
    expect(issues.filter((i) => i.level === "error")).toEqual([]);
    expect(issues.some((i) => i.message.includes("not connected"))).toBe(false);
    const bad = normalizeModel({ ...m, supports: [{ node: "3", fix: "fixed" }, { node: "5", fix: ["uz"] }] });
    expect(checkModel(bad).find((i) => i.level === "error")?.message).toMatch(/restraints at one joint only/);
  });
});

describe("pre-run checks", () => {
  it("lists what is missing; frames are split at joints along them", () => {
    let m = portal();
    let issues = checkModel(m);
    expect(issues.filter((i) => i.level === "error").map((i) => i.message)).toEqual([
      expect.stringMatching(/no supports/),
    ]);
    m = setRestraints(m, ["1", "4"], [true, true, true, true, true, true]);
    m = addNode(m, [3, 0, 4]).model; // on the beam, not connected
    // Meshed (the default), frames are split at the joints along them: only the free joint is reported.
    issues = checkModel(m);
    expect(issues.some((i) => i.level === "error")).toBe(false);
    expect(issues.some((i) => i.message.includes("lie on members"))).toBe(false);
    expect(issues.find((i) => i.message.includes("not connected to any"))?.nodes).toEqual(["5"]);
  });
});

describe("definitions in the explorer", () => {
  it("copies materials, sections and load cases with new names", () => {
    let m = setNodeLoads(portal(), "Dead", ["2"], { fx: 3 }, "add");
    let r = copyDefinition(m, "section", "FSEC1");
    expect(r.id).toBe("FSEC1-copy");
    r = copyDefinition(r.model, "section", "FSEC1");
    expect(r.id).toBe("FSEC1-copy2");
    expect(r.model.sections.map((s) => s.id)).toEqual(["FSEC1", "FSEC1-copy2", "FSEC1-copy"]);
    m = copyDefinition(m, "pattern", "Dead").model;
    expect(m.load_patterns[1]).toEqual({ id: "Dead-copy", type: "dead", node_loads: [{ node: "2", fx: 3 }], frame_loads: [], area_loads: [], self_weight: 1 });
    expect(copyDefinition(m, "case", "Dead").model.load_cases[1]).toEqual({ id: "Dead-copy", loads: [{ pattern: "Dead", scale: 1 }] });
    expect(copyDefinition(m, "material", "Steel").model.materials.map((x) => x.id)).toEqual(["Steel", "Steel-copy", "Concrete"]);
    expect(() => copyDefinition(m, "material", "Nope")).toThrow(EditError);
  });

  it("finds the members using a section or material", () => {
    const m = assignFrame(portal(), ["M2"], { section: "X" });
    expect(membersUsing(m, "section", "FSEC1")).toEqual(["M1", "M3"]);
    expect(membersUsing(m, "material", "Steel")).toEqual(["M1", "M2", "M3"]);
  });
});

describe("load combinations", () => {
  const base = () => {
    let m = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [4] });
    return upsertCombination(m, null, { id: "ULS", type: "linear-add", items: [{ case: "Dead", scale: 1.2 }, { case: "Live", scale: 1.6 }] });
  };

  it("adds, renames with cascades and refuses bad parts", () => {
    let m = base();
    m = upsertCombination(m, null, { id: "ENV", type: "envelope", items: [{ combo: "ULS", scale: 1 }, { case: "Dead", scale: 1 }] });
    m = setLoadCase(m, "Dead", { id: "D", loads: [{ pattern: "Dead", scale: 1 }] });
    expect(m.load_combinations[0].items[0]).toEqual({ case: "D", scale: 1.2 });
    m = upsertCombination(m, "ULS", { ...m.load_combinations[0], id: "ULS1" });
    expect(m.load_combinations[1].items[0]).toEqual({ combo: "ULS1", scale: 1 });
    expect(() => upsertCombination(m, null, { id: "Live", type: "envelope", items: [{ case: "D", scale: 1 }] })).toThrow(/already exists/);
    expect(() => setLoadCase(m, null, { id: "ENV", loads: [] })).toThrow(/already exists/);
    expect(() => upsertCombination(m, null, { id: "X", type: "envelope", items: [] })).toThrow(EditError);
    expect(() => upsertCombination(m, null, { id: "X", type: "envelope", items: [{ case: "NOPE", scale: 1 }] })).toThrow(/NOPE/);
    expect(() => upsertCombination(m, null, { id: "X", type: "envelope", items: [{ case: "D", scale: NaN }] })).toThrow(EditError);
    // ULS1 inside ENV, so ULS1 may not contain ENV.
    expect(() => upsertCombination(m, "ULS1", { id: "ULS1", type: "linear-add", items: [{ combo: "ENV", scale: 1 }] })).toThrow(/contain itself/);
  });

  it("refuses to delete what a combination uses; copies with a free name", () => {
    let m = base();
    expect(() => deleteLoadCase(m, "Live")).toThrow(/ULS/);
    m = upsertCombination(m, null, { id: "ENV", type: "envelope", items: [{ combo: "ULS", scale: 1 }] });
    expect(() => deleteCombination(m, "ULS")).toThrow(/ENV/);
    expect(deleteCombination(m, "ENV").load_combinations.map((c) => c.id)).toEqual(["ULS"]);
    const r = copyDefinition(m, "combo", "ULS");
    expect(r.id).toBe("ULS-copy");
    expect(r.model.load_combinations.map((c) => c.id)).toEqual(["ULS", "ULS-copy", "ENV"]);
  });

  it("reads combinations from files", () => {
    const m = normalizeModel({ load_combinations: [{ id: 1, type: "envelope", items: [{ case: "A", scale: "2" }, { combo: 3, scale: 1 }] }] });
    expect(m.load_combinations).toEqual([{ id: "1", type: "envelope", items: [{ case: "A", scale: 2 }, { combo: "3", scale: 1 }] }]);
    expect(normalizeModel({}).load_combinations).toEqual([]);
  });
});

describe("load patterns and cases", () => {
  it("a new model has the patterns Dead and Live, each with its own case", () => {
    const m = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [4] });
    expect(m.load_patterns.map((p) => [p.id, p.type, p.self_weight ?? 0])).toEqual([["Dead", "dead", 1], ["Live", "live", 0]]);
    expect(m.load_cases).toEqual([{ id: "Dead", loads: [{ pattern: "Dead", scale: 1 }] }, { id: "Live", loads: [{ pattern: "Live", scale: 1 }] }]);
  });

  it("adding a pattern adds its case; renaming follows into the cases, its own case, combinations and the mass source", () => {
    let m = upsertLoadPattern(portal(), null, { id: "SDL", type: "super-dead" });
    expect(m.load_cases.at(-1)).toEqual({ id: "SDL", loads: [{ pattern: "SDL", scale: 1 }] });
    m = setLoadCase(m, null, { id: "C1", loads: [{ pattern: "Dead", scale: 1.2 }, { pattern: "SDL", scale: 1.2 }, { pattern: "SDL", scale: 1 }] });
    m = upsertCombination(m, null, { id: "K", type: "linear-add", items: [{ case: "SDL", scale: 1 }] });
    m = setMassSource(m, { loads: [{ pattern: "SDL", scale: 1 }] }, m.modal ?? null);
    m = upsertLoadPattern(m, "SDL", { id: "SD", type: "super-dead" });
    expect(m.load_cases.find((c) => c.id === "C1")!.loads.map((l) => l.pattern)).toEqual(["Dead", "SD", "SD"]);
    expect(m.load_cases.some((c) => c.id === "SD")).toBe(true);
    expect(m.load_combinations[0].items[0]).toEqual({ case: "SD", scale: 1 });
    expect(m.mass_source?.loads).toEqual([{ pattern: "SD", scale: 1 }]);
    // A pattern name may equal a case name, not another pattern's.
    expect(() => upsertLoadPattern(m, null, { id: "Dead", type: "dead" })).toThrow(/already exists/);
    expect(upsertLoadPattern(m, null, { id: "C1", type: "other" }).load_cases.filter((c) => c.id === "C1")).toHaveLength(1);
  });

  it("refuses to delete a pattern still in use; deletes its own case with it", () => {
    let m = upsertLoadPattern(portal(), null, { id: "SDL", type: "super-dead" });
    m = setLoadCase(m, null, { id: "C1", loads: [{ pattern: "SDL", scale: 2 }] });
    expect(() => deleteLoadPattern(m, "SDL")).toThrow(/C1/);
    m = deleteLoadCase(m, "C1");
    m = deleteLoadPattern(m, "SDL");
    expect(m.load_patterns.map((p) => p.id)).toEqual(["Dead", "Live"]);
    expect(m.load_cases.map((c) => c.id)).toEqual(["Dead", "Live"]);
    expect(() => setLoadCase(m, null, { id: "X", loads: [{ pattern: "NOPE", scale: 1 }] })).toThrow(/NOPE/);
    expect(() => setLoadCase(m, null, { id: "X", loads: [{ pattern: "Dead", scale: NaN }] })).toThrow(EditError);
    expect(setLoadCase(m, null, { id: "EMPTY", loads: [] }).load_cases.at(-1)).toEqual({ id: "EMPTY", loads: [] });
  });

  it("opens files written before load patterns: one pattern per case, the mass source on patterns", () => {
    const m = normalizeModel({
      nodes: [{ id: 1 }],
      load_cases: [
        { id: "DEAD", self_weight: 1 },
        { id: "LIVE", node_loads: [{ node: 1, fz: -2 }] },
        { id: "W1", node_loads: [{ node: 1, fx: 3 }] },
        { id: "Q" },
      ],
      mass_source: { loads: [{ case: "LIVE", scale: 0.3 }] },
    });
    expect(m.load_patterns.map((p) => [p.id, p.type, p.self_weight ?? 0, p.node_loads.length])).toEqual([
      ["DEAD", "dead", 1, 0],
      ["LIVE", "live", 0, 1],
      ["W1", "other", 0, 1],
      ["Q", "other", 0, 0],
    ]);
    expect(m.load_cases.map((c) => [c.id, c.loads])).toEqual(["DEAD", "LIVE", "W1", "Q"].map((id) => [id, [{ pattern: id, scale: 1 }]]));
    expect(m.mass_source).toEqual({ loads: [{ pattern: "LIVE", scale: 0.3 }] });
    // A case with patterns and loads of its own: the own loads become a pattern.
    const mixed = normalizeModel({ nodes: [{ id: 1 }], load_patterns: [{ id: "P", type: "dead" }], load_cases: [{ id: "P", loads: [{ pattern: "P" }], node_loads: [{ node: 1, fz: -1 }] }] });
    expect(mixed.load_patterns.map((p) => p.id)).toEqual(["P", "P-loads"]);
    expect(mixed.load_cases[0].loads).toEqual([{ pattern: "P", scale: 1 }, { pattern: "P-loads", scale: 1 }]);
  });

  it("user wind loads on Wind patterns, semi-rigid diaphragms", () => {
    let m = upsertLoadPattern(portal(), null, { id: "WX", type: "wind" });
    const row = { story: "Story1", diaphragm: "D1", fx: 10, mz: 2, x: 1, y: 2 };
    m = setWindLoads(m, "WX", [row]);
    expect(m.load_patterns.find((p) => p.id === "WX")).toMatchObject({ auto: "user-wind", wind_loads: [row] });
    expect(normalizeModel(JSON.parse(JSON.stringify(m)))).toEqual(m);
    expect(() => setWindLoads(m, "Dead", [row])).toThrow(/not of type Wind/);
    expect(() => setWindLoads(m, "WX", [{ ...row, story: "Roof" }])).toThrow(/Roof/);
    expect(() => setWindLoads(m, "WX", [{ ...row, diaphragm: "D9" }])).toThrow(/D9/);
    expect(() => deleteDiaphragm(m, "D1")).toThrow(/WX/);
    m = setDiaphragmSemiRigid(m, "D1", true);
    expect(m.diaphragms).toEqual([{ id: "D1", semi_rigid: true }]);
    expect(addDiaphragm(m, "D2", true).diaphragms?.[1]).toEqual({ id: "D2", semi_rigid: true });
    // Not Wind any more: the user wind loads go.
    expect(upsertLoadPattern(m, "WX", { id: "WX", type: "other" }).load_patterns.find((p) => p.id === "WX")!.wind_loads).toBeUndefined();
    expect(setWindLoads(m, "WX", []).load_patterns.find((p) => p.id === "WX")!.auto).toBeUndefined();
  });
});

describe("insertion points in the file", () => {
  it("are read and kept; defaults are left out", () => {
    const raw = {
      nodes: [{ id: 1, x: 0, y: 0, z: 0 }, { id: 2, x: 4, y: 0, z: 0 }],
      frames: [
        { id: "M1", type: "frame", nodes: [1, 2], material: "C", section: "S", insertion: { point: 8, offset_j: [0, 0, -0.2], global: true } },
        { id: "M2", type: "frame", nodes: [1, 2], material: "C", section: "S", insertion: { point: 10 } },
        { id: "M3", type: "frame", nodes: [1, 2], material: "C", section: "S", insertion: { point: 99, transform: true } },
      ],
      areas: [
        { id: "F1", nodes: [1, 2, 2], section: "SL", insertion: { point: "top" } },
        { id: "F2", nodes: [1, 2, 2], section: "SL", insertion: { point: "middle" } },
      ],
    };
    const m = normalizeModel(raw);
    expect(m.frames.map((f) => f.insertion)).toEqual([{ point: 8, offset_j: [0, 0, -0.2], global: true }, undefined, { point: 10, transform: true }]);
    expect(m.areas.map((a) => a.insertion)).toEqual([{ point: "top" }, undefined]);
  });
});

describe("line and area springs in the file", () => {
  it("are read and kept; zero stiffness values are left out", () => {
    const raw = {
      nodes: [{ id: 1, x: 0, y: 0, z: 0 }, { id: 2, x: 4, y: 0, z: 0 }, { id: 3, x: 4, y: 4, z: 0 }],
      frames: [{ id: "M1", type: "frame", nodes: [1, 2], material: "C", section: "S", line_spring: "LS1" }],
      areas: [{ id: "F1", nodes: [1, 2, 3], section: "SL", area_spring: "AS1" }],
      line_spring_props: [{ id: "LS1", u1: 0, u2: 2000, r1: 50 }],
      area_spring_props: [{ id: "AS1", u3: 30000, u1: -5 }],
    };
    const m = normalizeModel(raw);
    expect(m.frames[0].line_spring).toBe("LS1");
    expect(m.areas[0].area_spring).toBe("AS1");
    expect(m.line_spring_props).toEqual([{ id: "LS1", u2: 2000, r1: 50 }]);
    expect(m.area_spring_props).toEqual([{ id: "AS1", u3: 30000 }]);
  });
});
