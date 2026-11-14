import { describe, expect, it } from "vitest";

import { addAreaAt, setAreaModifiers, setOpening, upsertAreaSection } from "./areaOps";
import { STIFF_MODIFIERS, modifierProblem, modifiersText, readModifiers, sectionAreaModifiers, withModifiers } from "./modifiers";
import { EditError, addFrame, addNode, copyDefinition, newModel, normalizeModel, setFrameModifiers, upsertSection } from "./ops";
import { AREA_MODIFIERS, FRAME_MODIFIERS, type Model, type Vec3 } from "./types";

/** Two members and a floor area on a new model's grid. */
function model(): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
  const ids: string[] = [];
  for (const p of [
    [0, 0, 0],
    [0, 0, 4],
    [6, 0, 4],
  ] as Vec3[]) {
    const r = addNode(m, p);
    m = r.model;
    ids.push(r.id);
  }
  const props = { type: "frame" as const, material: "Steel", section: "FSEC1" };
  m = addFrame(m, ids[0], ids[1], props).model;
  m = addFrame(m, ids[1], ids[2], props).model;
  const floor: Vec3[] = [
    [0, 0, 4],
    [6, 0, 4],
    [6, 6, 4],
    [0, 6, 4],
  ];
  return addAreaAt(m, floor, m.area_sections[0].id).model;
}

describe("property modifiers", () => {
  it("reads only numbers other than 1 and keeps them when a file is opened", () => {
    expect(readModifiers(FRAME_MODIFIERS, { A: 1, I33: 0.35, J: "0.1", bogus: 3 })).toEqual({ I33: 0.35, J: 0.1 });
    expect(readModifiers(FRAME_MODIFIERS, { A: 1 })).toBeUndefined();
    const m = model();
    m.sections[0].modifiers = { I33: 0.7 };
    m.frames[0].modifiers = { I22: 0.5, mass: 1 };
    m.area_sections[0].modifiers = { m11: 0.25, m22: 0.25, m12: 0.25 };
    m.areas[0].modifiers = { f12: 0.5 };
    const back = normalizeModel(JSON.parse(JSON.stringify(m)));
    expect(back.sections[0].modifiers).toEqual({ I33: 0.7 });
    expect(back.frames[0].modifiers).toEqual({ I22: 0.5 });
    expect(back.frames[1].modifiers).toBeUndefined();
    expect(back.area_sections[0].modifiers).toEqual({ m11: 0.25, m22: 0.25, m12: 0.25 });
    expect(back.areas[0].modifiers).toEqual({ f12: 0.5 });
  });

  it("assigns frame modifiers: given values replace, blank ones are kept, all 1 removes them", () => {
    let m = model();
    const [a, b] = m.frames.map((e) => e.id);
    m = setFrameModifiers(m, [a, b], { I33: 0.35, I22: 0.35 });
    m = setFrameModifiers(m, [a], { J: 0.1, I33: undefined });
    expect(m.frames[0].modifiers).toEqual({ I33: 0.35, I22: 0.35, J: 0.1 });
    expect(m.frames[1].modifiers).toEqual({ I33: 0.35, I22: 0.35 });
    m = setFrameModifiers(m, [b], { I33: 1, I22: 1 });
    expect(m.frames[1].modifiers).toBeUndefined();
    expect(() => setFrameModifiers(m, [a], { I33: 0 })).toThrow(EditError);
    expect(() => setFrameModifiers(m, [a], { mass: -1 })).toThrow(/mass must be a number of 0 or more/);
    expect(setFrameModifiers(m, [a], { mass: 0, weight: 0 }).frames[0].modifiers).toMatchObject({ mass: 0, weight: 0 });
  });

  it("assigns area modifiers, but not to openings", () => {
    let m = model();
    const id = m.areas[0].id;
    m = setAreaModifiers(m, [id], { m11: 0.25, m22: 0.25, m12: 0.25 });
    expect(m.areas[0].modifiers).toEqual({ m11: 0.25, m22: 0.25, m12: 0.25 });
    expect(() => setAreaModifiers(m, [id], { v13: -1 })).toThrow(/v13 must be a number greater than 0/);
    const opening = setAreaModifiers(setOpening(m, [id], true), [id], { f11: 0.5 });
    expect(opening.areas[0].modifiers).toEqual({ m11: 0.25, m22: 0.25, m12: 0.25 });
  });

  it("keeps section modifiers through edits and copies", () => {
    let m = model();
    const sec = m.sections[0];
    m = upsertSection(m, sec.id, { ...sec, modifiers: { I33: 0.7, A: 1 } });
    expect(m.sections[0].modifiers).toEqual({ I33: 0.7 });
    m = copyDefinition(m, "section", sec.id).model;
    expect(m.sections[1].modifiers).toEqual({ I33: 0.7 });
    expect(() => upsertSection(m, sec.id, { ...sec, modifiers: { J: Number.NaN } })).toThrow(EditError);
    const as = m.area_sections[0];
    m = upsertAreaSection(m, as.id, { ...as, modifiers: { f11: 0.7, f22: 0.7, mass: 1 } });
    expect(m.area_sections[0].modifiers).toEqual({ f11: 0.7, f22: 0.7 });
  });

  it("uses fixed values for a Stiff section", () => {
    expect(sectionAreaModifiers({ id: "S", type: "shell-thin", material: "C", thickness: 0.2, slab_type: "stiff", modifiers: { m11: 0.5 } })).toEqual(STIFF_MODIFIERS);
    expect(sectionAreaModifiers({ id: "S", type: "shell-thin", material: "C", thickness: 0.2, modifiers: { m11: 0.5 } })).toEqual({ m11: 0.5 });
  });

  it("checks values like the solver and writes them as text", () => {
    expect(modifierProblem("area", { mass: 0, weight: 0, m11: 100 })).toBeNull();
    expect(modifierProblem("area", { v23: 0 })).toEqual({ name: "v23", zeroAllowed: false });
    expect(modifierProblem("frame", { weight: -0.1 })).toEqual({ name: "weight", zeroAllowed: true });
    expect(withModifiers(AREA_MODIFIERS, { f11: 0.5 }, { f11: 1, m11: 0.25 })).toEqual({ m11: 0.25 });
    expect(modifiersText(FRAME_MODIFIERS, { J: 0.1, I33: 0.35 })).toBe("J 0.1, I33 0.35");
  });
});
