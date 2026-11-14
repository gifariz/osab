import { describe, expect, it } from "vitest";

import { buildResultsScene } from "../scene/buildResultsScene";
import { EditError, addNode, massesOf, newModel, normalizeModel, setJointMasses, setMassSource, upsertLoadPattern, deleteLoadPattern } from "./ops";
import type { Model, Results, Vec3 } from "./types";

function model(): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [3] });
  for (const p of [
    [0, 0, 3],
    [6, 0, 3],
  ] as Vec3[]) m = addNode(m, p).model;
  return m;
}

describe("modal analysis in the app", () => {
  it("a new model has the default modal case (Eigen, 12 modes) and default mass source", () => {
    const m = model();
    expect(m.modal).toEqual({ id: "Modal", modes: 12 });
    expect(m.mass_source).toBeUndefined();
  });

  it("joint masses are added, replaced, deleted and read back from a file", () => {
    let m = model();
    const [a, b] = m.nodes.map((n) => n.id);
    m = setJointMasses(m, [a, b], [2, 2, 0, 0, 0, 1], "replace");
    m = setJointMasses(m, [a], [1, 1, 0, 0, 0, 0], "add");
    expect(massesOf(m, a)).toEqual([3, 3, 0, 0, 0, 1]);
    expect(normalizeModel(JSON.parse(JSON.stringify(m))).masses).toEqual(m.masses);
    m = setJointMasses(m, [a, b], [], "delete");
    expect(m.masses).toBeUndefined();
    expect(() => setJointMasses(m, [a], [-1, 0, 0, 0, 0, 0], "replace")).toThrow(EditError);
  });

  it("the mass source keeps only what differs from the defaults and follows its load patterns", () => {
    let m = upsertLoadPattern(model(), null, { id: "SDL", type: "super-dead" });
    m = setMassSource(m, { elements: true, joint_masses: false, lateral_only: true, lump_at_stories: true, loads: [{ pattern: "SDL", scale: 0.5 }] }, { id: "Modal", modes: 6 });
    expect(m.mass_source).toEqual({ joint_masses: false, loads: [{ pattern: "SDL", scale: 0.5 }] });
    expect(m.modal).toEqual({ id: "Modal", modes: 6 });
    m = upsertLoadPattern(m, "SDL", { id: "SDL2", type: "super-dead" });
    expect(m.mass_source?.loads).toEqual([{ pattern: "SDL2", scale: 0.5 }]);
    // A pattern the mass source takes cannot be deleted.
    expect(() => deleteLoadPattern(m, "SDL2")).toThrow(EditError);
    expect(() => setMassSource(m, {}, { id: "Modal", modes: 0 })).toThrow(EditError);
    expect(() => setMassSource(m, { loads: [{ pattern: "X", scale: 1 }] }, null)).toThrow(EditError);
    expect(setMassSource(m, {}, null).modal).toBeUndefined();
  });

  it("a Ritz case keeps its starting loads in order and follows its load patterns", () => {
    let m = upsertLoadPattern(model(), null, { id: "SDL", type: "super-dead" });
    const ritz = { id: "Modal", modes: 5, type: "ritz" as const, starts: [{ accel: "UX" as const }, { pattern: "SDL", cycles: 2 }, { accel: "UY" as const, cycles: 0 }] };
    m = setMassSource(m, {}, ritz);
    expect(m.modal).toEqual({ id: "Modal", modes: 5, type: "ritz", starts: [{ accel: "UX" }, { pattern: "SDL", cycles: 2 }, { accel: "UY" }] });
    // Kept in the file.
    expect(normalizeModel(JSON.parse(JSON.stringify(m))).modal).toEqual(m.modal);
    // Renaming the pattern renames the start; a start's pattern cannot be deleted.
    m = upsertLoadPattern(m, "SDL", { id: "SDL2", type: "super-dead" });
    expect(m.modal?.starts?.[1]).toEqual({ pattern: "SDL2", cycles: 2 });
    expect(() => deleteLoadPattern(m, "SDL2")).toThrow(/starting load/);
    // At least one start, known patterns, whole cycles.
    expect(() => setMassSource(m, {}, { ...ritz, starts: [] })).toThrow(EditError);
    expect(() => setMassSource(m, {}, { ...ritz, starts: [{ pattern: "X" }] })).toThrow(EditError);
    expect(() => setMassSource(m, {}, { ...ritz, starts: [{ accel: "UX", cycles: 1.5 }] })).toThrow(EditError);
    // Ground rotations too (they turn about the centre of mass).
    const rot = setMassSource(m, {}, { ...ritz, starts: [{ accel: "RZ" }, { accel: "RX", cycles: 1 }] });
    expect(normalizeModel(JSON.parse(JSON.stringify(rot))).modal?.starts).toEqual([{ accel: "RZ" }, { accel: "RX", cycles: 1 }]);
    // Back to Eigen: no starts.
    expect(setMassSource(m, {}, { id: "Modal", modes: 12 }).modal).toEqual({ id: "Modal", modes: 12 });
  });

  it("draws the chosen mode shape", () => {
    const m = model();
    const [a, b] = m.nodes.map((n) => n.id);
    m.frames.push({ id: "B1", type: "frame", nodes: [a, b], material: "Steel", section: "FSEC1" });
    const shape = (k: number) => [
      { node: a, ux: k, uy: 0, uz: 0, rx: 0, ry: 0, rz: 0 },
      { node: b, ux: 2 * k, uy: 0, uz: 0, rx: 0, ry: 0, rz: 0 },
    ];
    const mode = (n: number) => ({ mode: n, period: 1 / n, frequency: n, circular_frequency: 0, eigenvalue: 0, participation: [], mass_ratio: [], sum_mass_ratio: [], shape: shape(n) });
    const r = { title: "", units: m.units, notes: [], cases: [], modal: { id: "Modal", modes: [mode(1), mode(2)], masses: [], free_mass: [], centre_of_mass: [0, 0, 0] } } as unknown as Results;
    const s = buildResultsScene(m, r, { view: { kind: "3d", at: 0 }, mode: "modes", caseId: null, component: "M3", scale: 1, size: 10, modeNumber: 2, showValues: false, tensionSide: true } as never);
    expect(s.maxValue).toBe(4);
    expect(s.scene.lines).toHaveLength(1);
    expect(s.scene.lines[0].b[0] - s.scene.lines[0].a[0]).toBeCloseTo(6 + 0.5, 9);
  });
});
