import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { frameSolid } from "../scene/areas";
import { addAreaAt, setOpening } from "./areaOps";
import { areaMidPlaneOffset, frameCentroidOffsets, insertionText, setAreaInsertion, setFrameInsertion } from "./insertion";
import { localAxes } from "./geometry";
import { addFrame, addNode, newModel, normalizeModel } from "./ops";
import { rectangle } from "./sections";
import type { Model, Vec3 } from "./types";

/** A new model with a 0.6 deep x 0.3 wide section "B" and joints at these points. */
function model(points: Vec3[]): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
  m = { ...m, sections: [...m.sections, { id: "B", ...rectangle(0.6, 0.3), depth: 0.6, width: 0.3 }] };
  for (const p of points) m = addNode(m, p).model;
  return m;
}

const props = { type: "frame" as const, material: "Steel", section: "B" };
const close = (a: Vec3, b: Vec3) => a.forEach((v, k) => expect(v).toBeCloseTo(b[k], 12));

describe("insertion points", () => {
  it("drawn objects get the usual ones: columns 5, beams 8, braces and null frames none, floors at their top", () => {
    let m = model([[0, 0, 0], [0, 0, 4], [6, 0, 4], [6, 0, 0]]);
    const [a, b, c, d] = m.nodes.slice(-4).map((n) => n.id);
    m = addFrame(m, a, b, props).model;
    m = addFrame(m, b, c, props).model;
    m = addFrame(m, d, b, props).model;
    m = addFrame(m, a, c, { ...props, section: "" }).model;
    expect(m.frames.map((f) => f.insertion)).toEqual([{ point: 5 }, { point: 8 }, undefined, undefined]);
    m = addAreaAt(m, [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    m = addAreaAt(m, [[0, 6, 0], [6, 6, 0], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    expect(m.areas.map((x) => x.insertion)).toEqual([{ point: "top" }, undefined]);
  });

  it("are assigned and cleared; openings get none", () => {
    let m = model([[0, 0, 4], [6, 0, 4]]);
    m = addFrame(m, m.nodes.at(-2)!.id, m.nodes.at(-1)!.id, props).model;
    const id = m.frames[0].id;
    m = setFrameInsertion(m, [id], { point: 2, mirror2: false, offset_i: [0, 0, 0], offset_j: [0, 0.1, 0], global: true, transform: true });
    expect(m.frames[0].insertion).toEqual({ point: 2, offset_j: [0, 0.1, 0], global: true, transform: true });
    expect(setFrameInsertion(m, [id], { point: 10, offset_i: [0, 0, 0], global: true }).frames[0]).not.toHaveProperty("insertion");
    m = addAreaAt(m, [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    m = addAreaAt(m, [[1, 1, 4], [2, 1, 4], [2, 2, 4], [1, 2, 4]], "", { opening: true }).model;
    m = setAreaInsertion(m, m.areas.map((a) => a.id), { point: "bottom", transform: true });
    expect(m.areas.map((a) => a.insertion)).toEqual([{ point: "bottom", transform: true }, undefined]);
    expect(setAreaInsertion(m, [m.areas[0].id], { point: "middle" }).areas[0]).not.toHaveProperty("insertion");
    expect(setOpening(m, [m.areas[1].id], true).areas[1].insertion).toBeUndefined();
  });

  it("put the centroid where the solver's rigid arms do", () => {
    // A beam along +X: local 2 = +Z, local 3 = -Y. The probe's element offsets for the 0.6 x 0.3 section.
    let m = model([[0, 0, 3], [4, 0, 3]]);
    m = addFrame(m, m.nodes.at(-2)!.id, m.nodes.at(-1)!.id, props).model;
    const f = (insertion: Model["frames"][number]["insertion"]) => frameCentroidOffsets(m, { ...m.frames[0], insertion });
    close(f({ point: 1 })[0], [0, -0.15, 0.3]);
    close(f({ point: 8 })[0], [0, 0, -0.3]);
    close(f({ point: 9 })[1], [0, 0.15, -0.3]);
    close(f({ point: 8, mirror3: true })[0], [0, 0, 0.3]);
    close(f({ point: 4, mirror2: true })[0], [0, 0.15, 0]);
    close(f({ point: 10, offset_i: [0, -0.2, 0.1] })[0], [0, -0.1, -0.2]);
    close(f({ point: 10, offset_j: [0, 0.1, -0.2], global: true })[1], [0, 0.1, -0.2]);
    close(f(undefined)[0], [0, 0, 0]);
    // The extruded beam at 8 hangs below its joints.
    const zs = frameSolid(m, m.frames[0].id).filter((_, i) => i % 3 === 2);
    expect(Math.min(...zs)).toBeCloseTo(2.4, 12);
    expect(Math.max(...zs)).toBeCloseTo(3.0, 12);
    // A section given only by its properties is drawn as the rectangle with the same A and I33 (0.6 deep here),
    // and point 8 still puts the beam's top at its joints.
    const bare = { ...m, sections: m.sections.map((s) => ({ ...s, depth: undefined, width: undefined })) };
    close(frameCentroidOffsets(bare, { ...bare.frames[0], insertion: { point: 8 } })[0], [0, 0, -0.3]);
    const top = frameSolid({ ...bare, frames: [{ ...bare.frames[0], insertion: { point: 8 } }] }, m.frames[0].id).filter((_, i) => i % 3 === 2);
    expect(Math.max(...top)).toBeCloseTo(3.0, 12);
    // An I-shape drawn from its dimensions, no depth stored: point 8 puts its top flange at the joints; point 2
    // (bottom centre) its bottom.
    const iShape = { shape: "i" as const, depth: 0.5, flangeWidth: 0.25, flangeThickness: 0.02, webThickness: 0.012 };
    const withI = { ...bare, gui: { ...(bare.gui ?? {}), sectionShapes: { [bare.frames[0].section]: iShape } } } as Model;
    const zsOf = (point: number) => frameSolid({ ...withI, frames: [{ ...withI.frames[0], insertion: { point } }] }, m.frames[0].id).filter((_, i) => i % 3 === 2);
    expect(Math.max(...zsOf(8))).toBeCloseTo(3.0, 12);
    expect(Math.min(...zsOf(8))).toBeCloseTo(2.5, 12);
    expect(Math.min(...zsOf(2))).toBeCloseTo(3.0, 12);
  });

  it("place a column at its point too (2 shifts it half its depth, 5 is the middle)", () => {
    let m = model([[0, 0, 0], [0, 0, 3]]);
    m = addFrame(m, m.nodes.at(-2)!.id, m.nodes.at(-1)!.id, props).model;
    const col = m.frames[0];
    // Point 5 is the middle: no shift; point 2 shifts the centroid half the depth along local 2.
    close(frameCentroidOffsets(m, { ...col, insertion: { point: 5 } })[0], [0, 0, 0]);
    const shifted = frameCentroidOffsets(m, { ...col, insertion: { point: 2 } })[0];
    expect(Math.hypot(...shifted)).toBeCloseTo(0.3, 12);
  });

  it("move a floor's mid-plane half its thickness; short texts for the view and tables", () => {
    let m = model([]);
    m = addAreaAt(m, [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    const a = m.areas[0];
    const p = a.nodes.map((id) => m.nodes.find((n) => n.id === id)!).map((n) => [n.x, n.y, n.z] as Vec3);
    const t = m.area_sections.find((s) => s.id === a.section)!.thickness;
    close(areaMidPlaneOffset(m, a, p), [0, 0, -t / 2]);
    close(areaMidPlaneOffset(m, { ...a, insertion: { point: "bottom" } }, p), [0, 0, t / 2]);
    expect(insertionText({ point: 8, mirror2: true, offset_j: [0, 0, 1], transform: true }, false)).toBe("8 M2 + T");
    expect(insertionText({ point: "top", transform: true }, true)).toBe("Top T");
    expect(insertionText(undefined, false)).toBe("10");
  });

  // B65 has every outline (steel, concrete and composite shapes); the AISC list's sections (B66) take these
  // outlines too once the app has read their dimensions from the steel table.
  it.each(["b65_section_shapes"])("put every section shape of %s at its cardinal point in the extruded view", (bench) => {
    const raw = JSON.parse(readFileSync(new URL(`../../../tests/benchmarks/${bench}/model.json`, import.meta.url), "utf8"));
    const m = normalizeModel(raw);
    expect(m.frames.length).toBeGreaterThanOrEqual(10);
    for (const f of m.frames) {
      const at = (id: string) => {
        const n = m.nodes.find((x) => x.id === id)!;
        return [n.x, n.y, n.z] as Vec3;
      };
      const a = at(f.nodes[0]);
      const [, ax2, ax3] = localAxes(a, at(f.nodes[1]), f.angle ?? 0);
      // The solid's extent along local 2 and 3, from the joint line, at a cardinal point.
      const extent = (point: number, axis: Vec3) => {
        const xyz = frameSolid({ ...m, frames: m.frames.map((x) => (x.id === f.id ? { ...x, insertion: { point } } : x)) }, f.id);
        const along: number[] = [];
        for (let k = 0; k < xyz.length; k += 3) along.push((xyz[k] - a[0]) * axis[0] + (xyz[k + 1] - a[1]) * axis[1] + (xyz[k + 2] - a[2]) * axis[2]);
        return [Math.min(...along), Math.max(...along)];
      };
      expect(extent(8, ax2)[1], `${f.id} top centre`).toBeCloseTo(0, 9);
      expect(extent(2, ax2)[0], `${f.id} bottom centre`).toBeCloseTo(0, 9);
      expect(extent(4, ax3)[0], `${f.id} middle left`).toBeCloseTo(0, 9);
      expect(extent(6, ax3)[1], `${f.id} middle right`).toBeCloseTo(0, 9);
    }
  });
});
