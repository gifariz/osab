import { describe, expect, it } from "vitest";

import { buildModelScene } from "../scene/buildModelScene";
import { COLORS } from "../scene/palette";
import { defaultViewOptions, withItems } from "../scene/viewOptions";
import { EditError, addFrame, addNode, assignFrame, cleanReleases, newModel, normalizeModel, releaseProblem, setFrameReleases } from "./ops";
import type { Model, Vec3 } from "./types";

/** A beam B1 between two joints of a new model. */
function beam(): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [4] });
  const ids: string[] = [];
  for (const p of [
    [0, 0, 4],
    [6, 0, 4],
  ] as Vec3[]) {
    const r = addNode(m, p);
    m = r.model;
    ids.push(r.id);
  }
  return addFrame(m, ids[0], ids[1], { type: "frame", material: "Steel", section: "FSEC1" }).model;
}

describe("end releases", () => {
  it("are kept in the standard order when a file is opened, without repeats", () => {
    expect(cleanReleases({ i: ["M3", "M2", "M3"], j: [] })).toEqual({ i: ["M2", "M3"] });
    expect(cleanReleases({ i: [] })).toBeUndefined();
    const m = beam();
    m.frames[0].releases = { j: ["M3", "T"] };
    expect(normalizeModel(JSON.parse(JSON.stringify(m))).frames[0].releases).toEqual({ j: ["T", "M3"] });
  });

  it("refuse unstable sets, like the solver", () => {
    expect(releaseProblem({ i: ["T"], j: ["T"] })).toMatch(/T released at both ends/);
    expect(releaseProblem({ i: ["M3"], j: ["M3", "V2"] })).toMatch(/M3 released at both ends together with V2/);
    expect(releaseProblem({ i: ["M2", "M3"], j: ["M2", "M3"] })).toBeNull();
    expect(releaseProblem({ i: ["T", "M2", "M3"], j: ["M2", "M3"] })).toBeNull();
  });

  it("are assigned, replaced and cleared; trusses take none", () => {
    let m = beam();
    const id = m.frames[0].id;
    m = setFrameReleases(m, [id], { i: ["M2", "M3"], j: ["M2", "M3"] });
    expect(m.frames[0].releases).toEqual({ i: ["M2", "M3"], j: ["M2", "M3"] });
    expect(() => setFrameReleases(m, [id], { i: ["P"], j: ["P"] })).toThrow(EditError);
    m = setFrameReleases(m, [id], {});
    expect(m.frames[0].releases).toBeUndefined();
    m = setFrameReleases(m, [id], { j: ["M3"] });
    const truss = assignFrame(m, [id], { type: "truss" });
    expect(truss.frames[0].releases).toBeUndefined();
    expect(() => setFrameReleases(truss, [id], { j: ["M3"] })).toThrow(/truss/);
  });

  it("are drawn as a dot near each released end", () => {
    const m = setFrameReleases(beam(), [beam().frames[0].id], { i: ["M3"] });
    const none = { nodes: [], frames: [] };
    const s = buildModelScene(m, { view: { kind: "3d", at: 0 }, selection: none, highlight: none, loadsPattern: null, dimmed: false, options: withItems(defaultViewOptions(), "frame", { releases: true }) });
    const dots = s.scene.points.filter((p) => p.color === COLORS.release);
    expect(dots).toHaveLength(1);
    expect(dots[0].position[0]).toBeCloseTo(0.48);
  });
});
