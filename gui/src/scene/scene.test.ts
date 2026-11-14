import { describe, expect, it } from "vitest";

import { localAxes } from "../model/geometry";
import { addAreaAt, setAreaLoads } from "../model/areaOps";
import { addFrame, addLink, addNode, newModel, setFrameLoads, setRestraints, setSprings } from "../model/ops";
import type { MeshInfo, Model, Results, Station, Vec3 } from "../model/types";
import { contourMesh, shellCornerValues, shellValue } from "./areas";
import { filteredOut, jointPositions, meshEdges } from "./mesh";
import { COLORS } from "./palette";
import { buildModelScene, modelStyles, spanSymbol } from "./buildModelScene";
import { pickKey, type PickRef, type PickStyle, type SceneData } from "./types";
import { defaultViewOptions, setAll, withItems } from "./viewOptions";
import { buildResultsScene, diagramAxis, diagramLabelStations, diagramSign } from "./buildResultsScene";
import { fmt } from "./format";
import { inView } from "./view";

function portal(): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [], storyHeights: [4] });
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
  m = setRestraints(m, ["1", "4"], [true, true, true, true, true, true]);
  return setFrameLoads(m, "Dead", ["M2"], "uniform", { type: "uniform", dir: "Z", w: -15 }, "add");
}

const st = (x: number, M3: number, uz = 0): Station => ({ x, P: 0, V2: 0, V3: 0, T: 0, M2: 0, M3, ux: 0, uy: 0, uz });

/** Results for a beam B1 (2 -> 3, along +X at z = 4) with a sagging moment of 10 at midspan. */
function beamResults(): Results {
  return {
    title: "",
    units: { force: "kN", length: "m" },
    notes: [],
    cases: [
      {
        id: "Dead",
        displacements: [],
        reactions: [{ node: "1", fx: 0, fy: 0, fz: 45, mx: 0, my: 14.3, mz: 0 }],
        frames: [
          {
            element: "M2",
            nodes: ["2", "3"],
            length: 6,
            axes: localAxes([0, 0, 4], [6, 0, 4]),
            stations: [st(0, -5), st(3, 10, -0.01), st(6, -5)],
          },
        ],
        equilibrium: { applied: [], reactions: [], relative_error: 0, residual: 0 },
      },
    ],
  };
}

describe("local axes", () => {
  it("follow the usual rule (beam: 2 up; column: 2 = +X)", () => {
    const beam = localAxes([0, 0, 0], [6, 0, 0]);
    expect(beam[1]).toEqual([0, 0, 1]);
    expect(beam[2].map((v) => v + 0)).toEqual([0, -1, 0]);
    const col = localAxes([0, 0, 0], [0, 0, 4]);
    expect(col[1]).toEqual([1, 0, 0]);
  });
});

describe("model scene", () => {
  it("shows only objects in the plane of a plan or elevation view", () => {
    expect(inView([0, 0, 4], { kind: "plan", at: 4 })).toBe(true);
    expect(inView([0, 0, 0], { kind: "plan", at: 4 })).toBe(false);
    const m = portal();
    const sel = { nodes: [], frames: [] };
    const opts = { selection: sel, highlight: sel, loadsPattern: null, dimmed: false };
    const plan = buildModelScene(m, { ...opts, view: { kind: "plan", at: 4 } });
    expect(plan.scene.lines.filter((l) => l.pick).map((l) => l.pick!.id)).toEqual(["M2"]);
    const elev = buildModelScene(m, { ...opts, view: { kind: "elev-xz", at: 0 } });
    expect(elev.scene.lines.filter((l) => l.pick)).toHaveLength(3);
    expect(elev.scene.glyphs.map((g) => g.kind)).toEqual(["fixed", "fixed"]);
  });

  it("does not draw hidden objects, their supports or loads", () => {
    const sel = { nodes: [], frames: [] };
    const s = buildModelScene(portal(), {
      view: { kind: "3d", at: 0 },
      selection: sel,
      highlight: sel,
      loadsPattern: "Dead",
      dimmed: false,
      hidden: { nodes: ["1"], frames: ["M2"] },
    });
    expect(s.scene.lines.filter((l) => l.pick).map((l) => l.pick!.id)).toEqual(["M1", "M3"]);
    expect(s.scene.points.map((p) => p.pick!.id)).toEqual(["2", "3", "4"]);
    expect(s.scene.glyphs).toHaveLength(1); // the support at joint 1 is hidden with it
    expect(s.scene.arrows).toEqual([]); // B1's load is hidden with B1
  });

  it("draws gravity loads pointing down onto the member", () => {
    const sel = { nodes: [], frames: [] };
    const s = buildModelScene(portal(), { view: { kind: "3d", at: 0 }, selection: sel, highlight: sel, loadsPattern: "Dead", dimmed: false });
    expect(s.scene.arrows.length).toBeGreaterThan(2);
    for (const a of s.scene.arrows) {
      expect(a.head[2]).toBeCloseTo(4, 9);
      expect(a.tail[2]).toBeGreaterThan(4);
    }
    expect(s.scene.labels.find((l) => l.className === "lbl-load")!.text).toBe("15");
  });

  it("draws trapezoidal loads with arrows growing from w1 to w2", () => {
    const m = portal();
    m.load_patterns[0].frame_loads = [{ type: "trapezoidal", frame: "M2", dir: "Z", w1: -4, w2: -10, from: 1, to: 5 }];
    const sel = { nodes: [], frames: [] };
    const s = buildModelScene(m, { view: { kind: "3d", at: 0 }, selection: sel, highlight: sel, loadsPattern: "Dead", dimmed: false });
    const arrows = s.scene.arrows;
    expect(arrows.length).toBeGreaterThan(2);
    expect(arrows[0].head[0]).toBeCloseTo(1, 9);
    expect(arrows[arrows.length - 1].head[0]).toBeCloseTo(5, 9);
    const length = (a: { head: Vec3; tail: Vec3 }) => a.tail[2] - a.head[2];
    expect(length(arrows[arrows.length - 1]) / length(arrows[0])).toBeCloseTo(10 / 4, 9);
    expect(s.scene.labels.filter((l) => l.className === "lbl-load").map((l) => l.text)).toEqual(["4", "10"]);
  });
});

describe("selection looks", () => {
  /** A scene with these objects drawn in their styles (what the viewport does with them). */
  const styled = (scene: SceneData, styles: Map<string, PickStyle>): SceneData => {
    const st = (p?: PickRef) => (p ? styles.get(pickKey(p)) : undefined);
    return {
      ...scene,
      lines: scene.lines.map((l) => {
        const s = st(l.pick);
        return s ? { ...l, color: s.color, dashed: s.dashed ?? l.dashed } : l;
      }),
      meshes: scene.meshes.map((m) => {
        const s = st(m.pick);
        return s ? { ...m, color: s.color, opacity: s.opacity ?? m.opacity } : m;
      }),
      points: scene.points.map((p) => {
        const s = st(p.pick);
        return s ? { ...p, color: s.color, size: s.size ?? p.size } : p;
      }),
    };
  };
  // Dashes: the scene says false or leaves it out for "not dashed".
  const plain = (d: SceneData) => ({ ...d, lines: d.lines.map((l) => ({ ...l, dashed: !!l.dashed })) });

  it("the model drawn without a selection, with the selected objects' styles, is the model drawn with it", () => {
    let m = addAreaAt(portal(), [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    m = addAreaAt(m, [[0, 0, 0], [6, 0, 0], [3, 0, 2]], "Slab1", { opening: true }).model;
    m = addLink(m, "2", "3").model;
    const selection = { nodes: ["2"], frames: ["M1"], links: [m.links[0].id], areas: m.areas.map((a) => a.id) };
    const highlight = { nodes: ["3"], frames: ["M2", "M1"], links: [], areas: [m.areas[0].id] };
    for (const options of [defaultViewOptions(), { ...defaultViewOptions(), fill: false }, { ...defaultViewOptions(), extrudeFrames: true, extrudeAreas: true }, { ...defaultViewOptions(), diaphragms: true }]) {
      for (const dimmed of [false, true]) {
        const o = { view: { kind: "3d" as const, at: 0 }, loadsPattern: null, dimmed, options };
        const want = buildModelScene(m, { ...o, selection, highlight }).scene;
        const base = buildModelScene(m, { ...o, selection: {}, highlight: {} }).scene;
        expect(plain(styled(base, modelStyles(m, { ...o, selection, highlight })))).toEqual(plain(want));
      }
    }
  });
});

describe("springs and links", () => {
  it("draws links as their own pickable lines and springs as a glyph", () => {
    let m = setSprings(portal(), ["1"], [0, 0, 0, 0, 5000, 0], "replace");
    m = addNode(m, [3, 0, 5]).model;
    m = addLink(m, "2", "5").model;
    const none = {};
    const opts = { view: { kind: "3d" as const, at: 0 }, selection: none, highlight: none, options: setAll(defaultViewOptions(), "labels", true), loadsPattern: null, dimmed: false };
    const s = buildModelScene(m, opts);
    const link = s.scene.lines.find((l) => l.pick?.kind === "link")!;
    expect(link.pick!.id).toBe("K1");
    expect([link.a, link.b]).toEqual([
      [0, 0, 4],
      [3, 0, 5],
    ]);
    expect(s.scene.labels.some((l) => l.text === "K1")).toBe(true);
    expect(s.scene.glyphs.map((g) => g.kind).sort()).toEqual(["fixed", "fixed", "spring"]);
    const hidden = buildModelScene(m, { ...opts, hidden: { links: ["K1"], nodes: ["1"] } });
    expect(hidden.scene.lines.some((l) => l.pick?.kind === "link")).toBe(false);
    expect(hidden.scene.glyphs.map((g) => g.kind)).toEqual(["fixed"]);
  });

  it("results include links in the deformed shape and force diagrams", () => {
    let m = addNode(portal(), [3, 0, 5]).model;
    m = addLink(m, "2", "5").model;
    const r = beamResults();
    r.cases[0].links = [
      { element: "K1", nodes: ["2", "5"], length: Math.hypot(3, 1), axes: localAxes([0, 0, 4], [3, 0, 5]), stations: [st(0, -6, -0.002), st(Math.hypot(3, 1), 0, -0.004)] },
    ];
    const opts = { view: { kind: "3d" as const, at: 0 }, caseId: "Dead", component: "M3" as const, scale: 1, showValues: true, tensionSide: true, size: 7 };
    const d = buildResultsScene(m, r, { ...opts, mode: "deformed" });
    expect(d.scene.lines.filter((l) => l.pick?.kind === "link")).toHaveLength(1);
    const f = buildResultsScene(m, r, { ...opts, mode: "forces" });
    expect(f.scene.lines.some((l) => l.pick?.kind === "link")).toBe(true);
    expect(f.scene.labels.some((l) => l.text === "-6")).toBe(true);
    expect(buildResultsScene(m, r, { ...opts, mode: "forces", hidden: { links: ["K1"] } }).scene.lines.some((l) => l.pick?.kind === "link")).toBe(false);
  });
});

describe("results scene", () => {
  const opts = { view: { kind: "3d" as const, at: 0 }, caseId: "Dead", component: "M3" as const, scale: 1, showValues: true, tensionSide: true, size: 7 };

  it("draws moments on the tension side: sagging below a beam", () => {
    expect(diagramAxis("M3")).toBe(1);
    expect(diagramAxis("M2")).toBe(2);
    expect(diagramSign("M3", true)).toBe(-1);
    expect(diagramSign("M3", false)).toBe(1);
    expect(diagramSign("V2", true)).toBe(1);
    const r = buildResultsScene(portal(), beamResults(), { ...opts, mode: "forces" });
    const mid = r.scene.labels.find((l) => l.text === "10")!;
    expect(mid.position[2]).toBeLessThan(4); // positive (sagging) M3 hangs below the beam
    const end = r.scene.labels.find((l) => l.text === "-5")!;
    expect(end.position[2]).toBeGreaterThan(4);
    expect(end.className).toContain("neg");
    expect(r.scene.meshes.map((x) => x.color)).toHaveLength(2); // both signs filled
    expect(r.maxValue).toBe(10);
  });

  it("labels the ends and an interior peak", () => {
    const er = beamResults().cases[0].frames[0];
    expect(diagramLabelStations(er, "M3").map((s) => s.x)).toEqual([0, 6, 3]);
  });

  it("deformed shape follows the station displacements", () => {
    const r = buildResultsScene(portal(), beamResults(), { ...opts, mode: "deformed" });
    const zs = r.scene.lines.flatMap((l) => [l.a[2], l.b[2]]);
    expect(Math.min(...zs)).toBeLessThan(4);
    expect(r.maxValue).toBeCloseTo(0.01, 12);
  });

  it("reactions get arrows and values", () => {
    const r = buildResultsScene(portal(), beamResults(), { ...opts, mode: "reactions" });
    expect(r.scene.arrows.length).toBe(2); // FZ force + MY moment
    expect(r.scene.labels[0].text).toBe("FZ 45\nMY 14.3");
  });
});

describe("number format", () => {
  it("rounds to 4 digits and hides round-off", () => {
    expect(fmt(33.6123)).toBe("33.61");
    expect(fmt(1e-14, 10)).toBe("0");
    expect(fmt(-1.464e-4)).toBe("-1.464e-4");
    expect(fmt(200e6)).toBe("2e+8");
  });
});

describe("areas in the drawing", () => {
  const opts = { view: { kind: "3d" as const, at: 0 }, caseId: "Dead", component: "M3" as const, scale: 1, showValues: true, tensionSide: true, size: 7 };
  const slab = () => {
    let m = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
    m = setAreaLoads(m, "Dead", ["A1"], { dir: "Z", q: -5 }, "add");
    return m;
  };
  const base = { view: { kind: "3d" as const, at: 0 }, selection: {}, highlight: {}, options: setAll(defaultViewOptions(), "labels", true), loadsPattern: "Dead", dimmed: false };

  it("draws a pickable face with edges, its label and its load", () => {
    const s = buildModelScene(slab(), base).scene;
    const face = s.meshes.find((x) => x.pick?.kind === "area");
    expect(face?.positions).toHaveLength(18);
    expect(s.lines.filter((l) => l.pick?.kind === "area")).toHaveLength(4);
    expect(s.labels.some((l) => l.text === "A1")).toBe(true);
    // The load: its value with unit and direction, no arrows (arrows are point and frame loads).
    expect(s.labels.some((l) => l.text === "-5 kN/m² Z")).toBe(true);
    expect(s.arrows).toHaveLength(0);
    // A plan at another story and hidden areas leave it out.
    expect(buildModelScene(slab(), { ...base, view: { kind: "plan", at: 0 } }).scene.meshes).toHaveLength(0);
    expect(buildModelScene(slab(), { ...base, hidden: { areas: ["A1"] } }).scene.meshes).toHaveLength(0);
  });

  it("extrudes members with their section and areas with their thickness", () => {
    let m = slab();
    m = addNode(m, [0, 0, 0]).model;
    const bottom = m.nodes.find((n) => n.z === 0)!.id;
    const top = m.nodes.find((n) => n.x === 0 && n.y === 0 && n.z === 4)!.id;
    m = addFrame(m, bottom, top, { type: "frame", material: "Steel", section: "FSEC1" }).model;
    const s = buildModelScene(m, { ...base, options: { ...base.options, extrudeFrames: true, extrudeAreas: true } }).scene;
    const solids = s.meshes.filter((x) => x.shaded);
    expect(solids).toHaveLength(2);
    const zs = (mesh: { positions: number[] }) => mesh.positions.filter((_, i) => i % 3 === 2);
    const slabSolid = solids.find((x) => x.pick?.kind === "area")!;
    // A drawn floor sits at its top (insertion point Top): from 3.8 up to the joints at 4.
    expect(Math.min(...zs(slabSolid))).toBeCloseTo(3.8);
    expect(Math.max(...zs(slabSolid))).toBeCloseTo(4.0);
    const col = solids.find((x) => x.pick?.kind === "frame")!;
    const xs = col.positions.filter((_, i) => i % 3 === 0);
    expect(Math.max(...xs) - Math.min(...xs)).toBeCloseTo(0.3);
  });

  it("draws triangles: face, extruded prism and a linear contour", () => {
    const m = addAreaAt(newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] }), [[0, 0, 4], [6, 0, 4], [0, 6, 4]], "Slab1").model;
    const s = buildModelScene(m, { ...base, loadsPattern: null }).scene;
    expect(s.meshes.find((x) => x.pick?.kind === "area")?.positions).toHaveLength(9);
    expect(s.lines.filter((l) => l.pick?.kind === "area")).toHaveLength(3);
    const solid = buildModelScene(m, { ...base, loadsPattern: null, options: { ...base.options, extrudeFrames: true, extrudeAreas: true } }).scene.meshes.find((x) => x.shaded)!;
    expect(solid.positions).toHaveLength((1 + 1 + 3 * 2) * 9); // two caps, three side quads
    const ids = m.areas[0].nodes;
    const mesh = contourMesh(ids.map((n) => m.nodes.find((x) => x.id === n)!).map((n) => [n.x, n.y, n.z] as Vec3), [0, 1, 2], 0, 2, { kind: "area", id: "A1" });
    expect(mesh.colors).toHaveLength(mesh.positions.length);
  });

  it("shows local axes of areas: 1 along +X, 3 up for a floor", () => {
    const s = buildModelScene(slab(), { ...base, loadsPattern: null, options: withItems(base.options, "area", { localAxes: true }) }).scene;
    expect(s.arrows).toHaveLength(3);
    const [one, , three] = s.arrows.map((x) => x.head.map((v, k) => v - x.tail[k]));
    expect(one[0]).toBeGreaterThan(0);
    expect(three[2]).toBeGreaterThan(0);
  });

  it("shell values: principal and resultant, averaged at joints", () => {
    const c = { node: "1", F11: 3, F22: -1, F12: 0, M11: 0, M22: 0, M12: 2, V13: 3, V23: 4 };
    expect(shellValue(c, "FMAX")).toBeCloseTo(3);
    expect(shellValue(c, "FMIN")).toBeCloseTo(-1);
    expect(shellValue(c, "MMAX")).toBeCloseTo(2);
    expect(shellValue(c, "VMAX")).toBeCloseTo(5);
    const corner = (node: string, M11: number) => ({ ...c, node, M11 });
    const a = { element: "A", area: "A", nodes: ["1", "2", "3", "4"], axes: [] as unknown as [Vec3, Vec3, Vec3], corners: [corner("1", 1), corner("2", 2), corner("3", 3), corner("4", 4)] };
    const b = { ...a, element: "B", area: "B", corners: [corner("2", 6), corner("5", 0), corner("6", 0), corner("3", 5)] };
    const avg = shellCornerValues([a, b], "M11", true);
    expect(avg.get(a)).toEqual([1, 4, 4, 4]);
    expect(shellCornerValues([a, b], "M11", false).get(b)).toEqual([6, 0, 0, 5]);
  });

  it("shell contours get a colour per vertex and a legend range", () => {
    const m = slab();
    const ids = m.areas[0].nodes;
    const corners = ids.map((node, k) => ({ node, F11: 0, F22: 0, F12: 0, M11: k, M22: 0, M12: 0, V13: 0, V23: 0 }));
    const results: Results = {
      title: "",
      units: m.units,
      notes: [],
      cases: [
        {
          id: "Dead",
          displacements: [],
          reactions: [],
          frames: [],
          shells: [{ element: "A1", area: "A1", nodes: ids, axes: [[1, 0, 0], [0, 1, 0], [0, 0, 1]], corners }],
          equilibrium: { applied: [], reactions: [], relative_error: 0, residual: 0 },
        },
      ],
    };
    const r = buildResultsScene(m, results, { ...opts, mode: "shells", shellComponent: "M11" });
    expect(r.legend).toEqual({ min: 0, max: 3 });
    const mesh = r.scene.meshes[0];
    expect(mesh.colors).toHaveLength(mesh.positions.length);
    expect(r.scene.labels.map((l) => l.text).sort()).toEqual(["0", "3"]);
  });

  it("the analysis mesh: drawn on the areas, results on its elements, types filtered out", () => {
    const m = slab();
    const [a, b, c, d] = m.areas[0].nodes;
    // F1 split into two elements at mesh joints ~1 (3, 0) and ~2 (3, 6).
    const mesh: MeshInfo = {
      joints: [
        { id: "~1", x: 3, y: 0, z: 4 },
        { id: "~2", x: 3, y: 6, z: 4 },
      ],
      shells: [
        { id: "A1-1", parent: "A1", nodes: [a, "~1", "~2", d] },
        { id: "A1-2", parent: "A1", nodes: ["~1", b, c, "~2"] },
      ],
      frames: [],
      edge_constraints: [],
    };
    // Seven distinct edges, the shared one once.
    expect(buildModelScene(m, { ...base, mesh }).scene.lines.filter((l) => l.color === COLORS.mesh)).toHaveLength(7);
    expect(meshEdges(m, mesh, () => true)).toHaveLength(7);
    expect(meshEdges(m, mesh, () => false)).toHaveLength(0);
    expect(jointPositions(m, mesh).get("~2")).toEqual([3, 6, 4]);
    // Shell contours at the mesh joints; the deformed shape per element.
    const corner = (node: string) => ({ node, F11: 0, F22: 0, F12: 0, M11: 1, M22: 0, M12: 0, V13: 0, V23: 0 });
    const shells = mesh.shells.map((el) => ({ element: el.id, area: "A1", nodes: el.nodes, axes: [[1, 0, 0], [0, 1, 0], [0, 0, 1]] as [Vec3, Vec3, Vec3], corners: el.nodes.map(corner) }));
    const disp = (node: string) => ({ node, ux: 0, uy: 0, uz: -0.01, rx: 0, ry: 0, rz: 0 });
    const results: Results = {
      title: "",
      units: m.units,
      notes: [],
      mesh,
      cases: [{ id: "Dead", displacements: [a, b, c, d, "~1", "~2"].map(disp), reactions: [], frames: [], shells }],
    };
    expect(buildResultsScene(m, results, { ...opts, mode: "shells" }).scene.meshes).toHaveLength(2);
    expect(buildResultsScene(m, results, { ...opts, mode: "deformed" }).scene.meshes).toHaveLength(2);
    // View > Show Object Types without floors: nothing of F1 is drawn.
    expect(buildResultsScene(m, results, { ...opts, mode: "shells", hiddenTypes: ["area:floor"] }).scene.meshes).toHaveLength(0);
    expect(buildModelScene(m, { ...base, options: { ...defaultViewOptions(), hiddenTypes: ["area:floor"] } }).scene.meshes).toHaveLength(0);
    expect(filteredOut(m, ["area:floor"]).areas).toEqual(new Set(["A1"]));
  });
});

describe("span symbol of one-way floors", () => {
  it("lies along local axis 1, 0.45 of the floor's length through its centre (at most 0.9 of its width), with opposite end slashes", () => {
    const p: Vec3[] = [[0, 0, 3], [8, 0, 3], [8, 4, 3], [0, 4, 3]];
    const [main, s1, s2] = spanSymbol(p, 0);
    expect(main[0].map((v) => +v.toFixed(9))).toEqual([5.8, 2, 3]);
    expect(main[1].map((v) => +v.toFixed(9))).toEqual([2.2, 2, 3]);
    // Point-symmetric slashes about the centre.
    const c = [4, 2, 3];
    s1[1].forEach((v, k) => expect(v - c[k]).toBeCloseTo(-(s2[1][k] - c[k]), 12));
    const [along] = spanSymbol(p, 90);
    expect(Math.hypot(...along[0].map((v, k) => v - along[1][k]))).toBeCloseTo(1.8, 12);
    expect(along[0][0]).toBeCloseTo(4, 12);
    // Skewed: the length through the centre along local 1 (8 m at 30 degrees across a 4 m wide floor).
    const [skew] = spanSymbol([[0, 0, 3], [16, 0, 3], [16, 4, 3], [0, 4, 3]], 30);
    expect(Math.hypot(...skew[0].map((v, k) => v - skew[1][k]))).toBeCloseTo(0.45 * 8, 9);
  });

  it("is drawn in plan views only, for one-way sections only", () => {
    let m = newModel({ xSpacings: [8], ySpacings: [4], storyHeights: [3] });
    m = addAreaAt(m, [[0, 0, 3], [8, 0, 3], [8, 4, 3], [0, 4, 3]], "Deck1").model;
    m = addAreaAt(m, [[0, 0, 3], [8, 0, 3], [8, -4, 3], [0, -4, 3]], "Slab1").model;
    const sel = { nodes: [], frames: [] };
    const opts = { selection: sel, highlight: sel, loadsPattern: null, dimmed: false };
    const spans = (view: { kind: "plan" | "3d"; at: number }) => buildModelScene(m, { ...opts, view }).scene.lines.filter((l) => l.color === COLORS.span);
    expect(spans({ kind: "plan", at: 3 })).toHaveLength(3);
    expect(spans({ kind: "3d", at: 0 })).toHaveLength(0);
  });
});
