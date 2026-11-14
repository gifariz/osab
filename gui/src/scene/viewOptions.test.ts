import { describe, expect, it } from "vitest";

import { withViewOptions } from "../actions";
import { addAreaAt } from "../model/areaOps";
import { addFrame, addLink, addNode, newModel, setFrameModifiers, setJointMasses, setRestraints } from "../model/ops";
import type { Model, Vec3 } from "../model/types";
import { MAX_TEXTS, buildModelScene, planHull, type ModelSceneOptions } from "./buildModelScene";
import { COLORS } from "./palette";
import { allOn, defaultViewOptions, setAll, viewOptionsOf, withItems, type ViewOptions } from "./viewOptions";

/** Portal: columns M1 (1-2) and M3 (4-3), beam M2 (2-3) at z = 4, fixed bases. */
function portal(): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
  for (const p of [
    [0, 0, 0],
    [0, 0, 4],
    [6, 0, 4],
    [6, 0, 0],
  ] as Vec3[])
    m = addNode(m, p).model;
  const props = { type: "frame" as const, material: "Steel", section: "FSEC1" };
  m = addFrame(m, "1", "2", props).model;
  m = addFrame(m, "2", "3", props).model;
  m = addFrame(m, "4", "3", props).model;
  return setRestraints(m, ["1", "4"], [true, true, true, true, true, true]);
}

const scene = (m: Model, options: ViewOptions, extra: Partial<ModelSceneOptions> = {}) =>
  buildModelScene(m, { view: { kind: "3d", at: 0 }, selection: {}, highlight: {}, loadsPattern: null, dimmed: false, options, ...extra });
const D = defaultViewOptions;

describe("view options", () => {
  it("read from a file: missing or wrong entries are the defaults", () => {
    expect(viewOptionsOf(undefined)).toEqual(D());
    const o = viewOptionsOf({ colorBy: "materials", grid: true, frame: { sections: true, ids: "yes" }, hiddenTypes: ["joint", 3], extra: 1 });
    expect(o.colorBy).toBe("materials");
    expect(o.grid).toBe(true);
    expect(o.frame).toEqual({ ...D().frame, sections: true });
    expect(o.hiddenTypes).toEqual(["joint"]);
    expect(viewOptionsOf({ colorBy: "rainbow" }).colorBy).toBe("sections");
  });

  it("are saved in gui.viewOptions only when they differ from the defaults", () => {
    const m = portal();
    expect(withViewOptions(m, D())).toBe(m);
    const o = withItems(D(), "frame", { sections: true });
    const saved = withViewOptions(m, o);
    expect(saved.gui?.viewOptions).toEqual(o);
    expect(saved.gui?.grid).toEqual(m.gui?.grid);
    expect(viewOptionsOf(JSON.parse(JSON.stringify(saved)).gui.viewOptions)).toEqual(o);
    expect(withViewOptions(saved, D()).gui).not.toHaveProperty("viewOptions");
  });

  it("Show Labels and Show Local Axes switch every kind at once", () => {
    const on = setAll(D(), "labels", true);
    expect([on.joint.labels, on.frame.labels, on.area.labels, on.link.labels]).toEqual([true, true, true, true]);
    expect(allOn(on, "labels")).toBe(true);
    expect(allOn(withItems(on, "link", { labels: false }), "labels")).toBe(false);
    const axes = setAll(D(), "localAxes", true);
    expect([axes.frame.localAxes, axes.area.localAxes, axes.link.localAxes]).toEqual([true, true, true]);
  });
});

describe("model view with view options", () => {
  it("draws no object texts or assignment symbols by default", () => {
    let m = setFrameModifiers(portal(), ["M2"], { I33: 0.5 });
    m = setJointMasses(m, ["2"], [5, 5, 0, 0, 0, 0], "replace");
    const s = scene(m, D()).scene;
    expect(s.labels.filter((l) => l.className !== "lbl-grid")).toEqual([]);
    expect(s.arrows).toEqual([]);
    expect(s.glyphs).toHaveLength(2); // supports are on
    expect(scene(m, { ...D(), supports: false }).scene.glyphs).toEqual([]);
  });

  it("stacks a member's texts in one label: label, unique name, section, modified", () => {
    let m = setFrameModifiers(portal(), ["M2"], { I33: 0.5 });
    m.frames[1].label = "B1";
    const o = withItems(D(), "frame", { labels: true, ids: true, sections: true, modifiers: true });
    const texts = scene(m, o).scene.labels.filter((l) => l.className === "lbl-elem").map((l) => l.text);
    expect(texts).toEqual(["M1\nM1\nFSEC1", "B1\nM2\nFSEC1\nModified", "M3\nM3\nFSEC1"]);
  });

  it("writes area sections and link properties", () => {
    let m = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    m = addNode(m, [0, 0, 0]).model;
    m = addLink(m, m.nodes[0].id, m.nodes.at(-1)!.id).model;
    const o = withItems(withItems(D(), "area", { sections: true }), "link", { properties: true });
    const texts = scene(m, o).scene.labels.map((l) => l.text);
    expect(texts).toContain("Slab1");
    expect(texts).toContain("Rigid");
  });

  it("draws joint masses as arrows along X and Y with their values", () => {
    const m = setJointMasses(portal(), ["2"], [5, 2, 0, 0, 0, 0], "replace");
    const s = scene(m, withItems(D(), "joint", { masses: true })).scene;
    expect(s.arrows.map((a) => a.head.map((v, k) => Math.sign(v - a.tail[k])))).toEqual([
      [1, 0, 0],
      [0, 1, 0],
    ]);
    expect(s.arrows.every((a) => a.color === COLORS.mass)).toBe(true);
    // 5 t and 2 t in SI display units (kg).
    expect(s.labels.filter((l) => l.className === "lbl-mass").map((l) => l.text)).toEqual(["UX 5000", "UY 2000"]);
  });

  it("leaves out joints and links by type, shrinks members and draws areas without fill", () => {
    let m = portal();
    m = addLink(m, "2", "4").model;
    const s = scene(m, { ...D(), hiddenTypes: ["joint", "link"], shrink: true }).scene;
    expect(s.points.filter((p) => p.pick)).toEqual([]);
    expect(s.lines.some((l) => l.pick?.kind === "link")).toBe(false);
    const beam = s.lines.find((l) => l.pick?.id === "M2")!;
    expect(beam.a[0]).toBeCloseTo(0.45);
    expect(beam.b[0]).toBeCloseTo(5.55);
    const slab = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    const face = (o: ViewOptions) => scene(slab, o).scene.meshes.find((x) => x.pick?.kind === "area")!.opacity;
    expect(face(D())).toBe(0.4);
    expect(face({ ...D(), fill: false })).toBeLessThan(0.05);
  });

  it("draws invisible joints without their dots, still picked, with their symbols and masses", () => {
    const m = setJointMasses(portal(), ["2"], [5, 0, 0, 0, 0, 0], "replace");
    const o = withItems({ ...D(), jointsInvisible: true }, "joint", { masses: true });
    const s = scene(m, o).scene;
    const dots = s.points.filter((p) => p.pick?.kind === "node");
    expect(dots.map((p) => p.size)).toEqual([0, 0, 0, 0]);
    expect(s.glyphs).toHaveLength(2);
    expect(s.arrows).toHaveLength(1);
    // A selected joint shows its dot.
    const sel = scene(m, o, { selection: { nodes: ["2"] } }).scene.points.filter((p) => p.pick?.kind === "node");
    expect(sel.map((p) => p.size)).toEqual([0, 9, 0, 0]);
  });

  it("colours members by section, object type or material", () => {
    const m = portal();
    const colour = (o: ViewOptions, id: string) => scene(m, o).scene.lines.find((l) => l.pick?.id === id)!.color;
    expect(colour(D(), "M1")).toBe(colour(D(), "M2"));
    expect(colour({ ...D(), colorBy: "types" }, "M1")).not.toBe(colour({ ...D(), colorBy: "types" }, "M2"));
    expect(colour({ ...D(), colorBy: "materials" }, "M1")).toBe(colour({ ...D(), colorBy: "materials" }, "M3"));
  });

  it("stops writing object texts at the cap and counts the ones left out", () => {
    let m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
    for (let i = 0; i < MAX_TEXTS + 5; i++) m.nodes.push({ id: `N${i}`, x: i, y: 0, z: 0, special: true });
    const r = scene(m, withItems(D(), "joint", { ids: true }));
    expect(r.scene.labels.filter((l) => l.className === "lbl-node")).toHaveLength(MAX_TEXTS);
    expect(r.textsLeftOut).toBe(5);
  });

  it("draws no texts over results", () => {
    const r = scene(portal(), setAll(D(), "labels", true), { dimmed: true });
    expect(r.scene.labels.filter((l) => l.className !== "lbl-grid")).toEqual([]);
  });
});

describe("plan hull", () => {
  it("keeps the outer corners, counter-clockwise", () => {
    const hull = planHull([
      [0, 0, 4],
      [6, 0, 4],
      [3, 2, 4],
      [6, 4, 4],
      [0, 4, 4],
    ]);
    expect(hull).toEqual([
      [0, 0, 4],
      [6, 0, 4],
      [6, 4, 4],
      [0, 4, 4],
    ]);
    expect(planHull([[0, 0, 0], [1, 0, 0], [2, 0, 0]])).toEqual([[0, 0, 0], [2, 0, 0]]);
  });
});
