import { describe, expect, it } from "vitest";

import { addFrame, addNode, assignFrame, newModel, normalizeModel, upsertMaterial, upsertSection } from "./ops";
import { SHAPES, defaultShape, outlineFaces, polygonArea, sectionShape, shapeOutline, type ShapeKind, type ShapeSpec } from "./sectionShapes";
import type { Model, Section } from "./types";

/** The area of an outline (holes subtract). */
const area = (polys: [number, number][][]) => polys.reduce((s, p) => s + polygonArea(p), 0);

describe("section shape outlines", () => {
  it("every shape's default outline has the area of its closed form (circles as 48-sided polygons)", () => {
    const n48 = (48 / (2 * Math.PI)) * Math.sin((2 * Math.PI) / 48);
    const exact: Partial<Record<ShapeKind, number>> = {
      rectangle: 0.5 * 0.3,
      i: 2 * 0.2 * 0.016 + (0.4 - 0.032) * 0.01,
      channel: 2 * 0.1 * 0.012 + (0.3 - 0.024) * 0.008,
      tee: 0.1 * 0.012 + (0.3 - 0.012) * 0.008,
      angle: 0.1 * 0.012 + (0.15 - 0.012) * 0.01,
      box: 0.3 * 0.2 - (0.3 - 0.024) * (0.2 - 0.02),
      pipe: n48 * Math.PI * (0.15 ** 2 - 0.14 ** 2),
      circle: n48 * Math.PI * 0.3 ** 2,
      cross: 0.6 * 0.2 + (0.8 - 0.2) * 0.25,
      trapezoid: ((0.4 + 0.3) / 2) * 0.6,
    };
    for (const kind of Object.keys(SHAPES) as Exclude<ShapeKind, "aisc">[]) {
      const o = shapeOutline(defaultShape(kind));
      expect(area(o.steel), kind).toBeGreaterThan(0);
      if (exact[kind] !== undefined) expect(area(o.steel), kind).toBeCloseTo(exact[kind]!, 10);
      // Each hole lies in an outline.
      const faces = outlineFaces([...o.steel, ...o.fill]);
      expect(faces.reduce((s, f) => s + f.holes.length, 0), kind).toBe([...o.steel, ...o.fill].filter((p) => polygonArea(p) < 0).length);
    }
  });

  it("mirrors keep the outline counterclockwise and move the legs", () => {
    const a: ShapeSpec = { type: "angle", depth: 0.15, width: 0.1, flange: 0.012, web: 0.01 };
    const [plain] = shapeOutline(a).steel;
    const [m2] = shapeOutline({ ...a, mirror2: true }).steel;
    expect(polygonArea(m2)).toBeCloseTo(polygonArea(plain), 12);
    // The vertical leg on the +3 side, then on the -3 side.
    expect(Math.max(...plain.filter((p) => p[1] > 0).map((p) => p[0]))).toBeCloseTo(0.05);
    expect(Math.min(...m2.filter((p) => p[1] > 0).map((p) => p[0]))).toBeCloseTo(-0.05);
  });
});

describe("sections with a shape", () => {
  const base = (): Model => {
    let m = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [3] });
    m = upsertMaterial(m, null, { id: "C30", E: 25e6, nu: 0.2, weight: 23.5616 });
    return m;
  };
  const numbers: Omit<Section, "id"> = { A: 0.0165, I22: 8.3e-5, I33: 1.7e-4, J: 1.9e-4, As2: 0.0118, As3: 0.011, depth: 0.3, width: 0.2, centroid_offset: [0, 0], self_weight: 1.96 };

  it("keep the shape and the solver's numbers; a composite section gives its frames the steel", () => {
    let m = base();
    const steel = m.materials.find((x) => x.id !== "C30")!.id;
    const ft: ShapeSpec = { type: "filled_tube", depth: 0.3, width: 0.2, flange: 0.012, web: 0.01, material: steel, fill: "C30" };
    m = upsertSection(m, null, { id: "FT", ...numbers }, ft);
    const sec = m.sections.find((s) => s.id === "FT")!;
    expect(sec.shape).toEqual(ft);
    expect(sec.self_weight).toBe(1.96);
    expect(sectionShape(m, sec)).toEqual(ft);
    // A frame drawn or assigned with it takes the steel.
    m = addNode(m, [0, 0, 0]).model;
    m = addNode(m, [0, 0, 3]).model;
    const ids = m.nodes.slice(-2).map((n) => n.id);
    const r = addFrame(m, ids[0], ids[1], { type: "frame", material: "C30", section: "FT" });
    expect(r.model.frames.find((f) => f.id === r.id)!.material).toBe(steel);
    const m2 = assignFrame(r.model, [r.id], { material: "C30" });
    expect(m2.frames.find((f) => f.id === r.id)!.material).toBe(steel);
    expect(() => upsertSection(m, null, { id: "X", ...numbers }, { ...ft, fill: "NOPE" })).toThrow(/NOPE/);
  });

  it("an older file's rectangle record reads as a shape; files keep shapes and the new numbers", () => {
    const m = base();
    expect(sectionShape(m, m.sections[0])).toEqual({ type: "rectangle", depth: 0.3, width: 0.3 });
    const read = normalizeModel({
      sections: [{ id: "L", A: 0.0026, I22: 2.3e-6, I33: 5.8e-6, J: 1e-7, I23: -2.2e-6, centroid_offset: [-0.029, -0.024], shape: { type: "angle", depth: 0.15, width: 0.1, flange: 0.012, web: 0.01, mirror2: true } }],
    });
    expect(read.sections[0]).toMatchObject({ I23: -2.2e-6, centroid_offset: [-0.029, -0.024], shape: { type: "angle", mirror2: true } });
  });
});
