import { describe, expect, it } from "vitest";

import { buildModelScene } from "../scene/buildModelScene";
import { COLORS } from "../scene/palette";
import { defaultViewOptions } from "../scene/viewOptions";
import { addAreaAt } from "./areaOps";
import { EditError, addDiaphragm, addNode, deleteDiaphragm, newModel, normalizeModel, setDiaphragm, withStories } from "./ops";
import type { Model, Vec3 } from "./types";

/** A new model (one 4 m story) with two joints at the story level and a floor area over a 6 x 4 bay. */
function story(): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [4], storyHeights: [4] });
  for (const p of [
    [0, 0, 4],
    [6, 0, 4],
    [6, 4, 4],
    [0, 4, 4],
  ] as Vec3[]) {
    m = addNode(m, p).model;
  }
  m.area_sections = [{ id: "Slab1", type: "shell-thin", material: "Concrete", thickness: 0.2 }];
  m.areas = [{ id: "F1", nodes: m.nodes.map((n) => n.id), section: "Slab1" }];
  return m;
}

describe("rigid diaphragms", () => {
  it("a new model has a semi-rigid D1 that new story joints and floors get", () => {
    let m = story();
    expect(m.diaphragms).toEqual([{ id: "D1", semi_rigid: true }]);
    expect(m.nodes.every((n) => n.diaphragm === "D1")).toBe(true);
    // Not at the base, not between levels; a joint drawn on its own is in the mesh.
    m = addNode(m, [0, 0, 0]).model;
    m = addNode(m, [0, 0, 2]).model;
    m = addNode(m, [3, 2, 4], { free: true }).model;
    expect(m.nodes.slice(4).map((n) => [n.diaphragm, n.in_mesh])).toEqual([[undefined, undefined], [undefined, undefined], ["D1", true]]);
    // A floor drawn gets D1, a wall and an opening do not.
    const f = addAreaAt(m, [[0, 0, 4], [6, 0, 4], [6, 4, 4]], "Slab1").model;
    expect(f.areas.at(-1)!.diaphragm).toBe("D1");
    const w = addAreaAt(m, [[0, 0, 0], [6, 0, 0], [6, 0, 4], [0, 0, 4]], "Slab1").model;
    expect(w.areas.at(-1)!.diaphragm).toBeUndefined();
    // Without diaphragms nothing is assigned.
    expect(addNode({ ...m, diaphragms: [] }, [6, 4, 8]).model.nodes.at(-1)!.diaphragm).toBeUndefined();
  });

  it("are assigned to joints and areas, removed with their diaphragm and read back from a file", () => {
    let m = story();
    m = setDiaphragm(m, [m.nodes[0].id], ["F1"], "D1");
    expect(m.nodes[0].diaphragm).toBe("D1");
    expect(m.areas[0].diaphragm).toBe("D1");
    expect(normalizeModel(JSON.parse(JSON.stringify(m)))).toMatchObject({ diaphragms: [{ id: "D1", semi_rigid: true }], areas: [{ diaphragm: "D1" }] });
    m = addDiaphragm(m, "D2");
    expect(() => addDiaphragm(m, "D2")).toThrow(EditError);
    expect(() => setDiaphragm(m, [], ["F1"], "D9")).toThrow(EditError);
    m = setDiaphragm(m, [], ["F1"], null);
    expect(m.areas[0].diaphragm).toBeUndefined();
    m = deleteDiaphragm(m, "D1");
    expect(m.diaphragms).toEqual([{ id: "D2" }]);
    // Its joints had D1 as the new model's default: deleting it removes that too.
    expect(m.nodes[0].diaphragm).toBeUndefined();
  });

  it("the solver gets the grid's stories", () => {
    const m = withStories(story());
    expect(m.stories).toEqual([
      { id: "Base", z: 0 },
      { id: "Story1", z: 4 },
    ]);
  });

  it("are drawn with Diaphragm extent: joints in the diaphragm colour, lines from the corners to the centre, the name there", () => {
    const m = setDiaphragm(story(), [story().nodes[1].id], ["F1"], "D1");
    const none = { nodes: [], frames: [] };
    const s = buildModelScene(m, { view: { kind: "3d", at: 0 }, selection: none, highlight: none, loadsPattern: null, dimmed: false, options: { ...defaultViewOptions(), diaphragms: true } });
    // The story joints have D1 already (a new model's default); with the floor's corners, one group.
    expect(s.scene.points.filter((p) => p.color === COLORS.diaphragm && p.pick)).toHaveLength(4);
    const centre = s.scene.points.filter((p) => p.color === COLORS.diaphragm && !p.pick);
    expect(centre.map((p) => p.position)).toEqual([[3, 2, 4]]);
    expect(s.scene.lines.filter((l) => l.color === COLORS.diaphragm && l.dashed)).toHaveLength(4);
    expect(s.scene.labels.filter((l) => l.className === "lbl-dia").map((l) => l.text)).toEqual(["D1"]);
    // Off (the default): nothing of it is drawn.
    const off = buildModelScene(m, { view: { kind: "3d", at: 0 }, selection: none, highlight: none, loadsPattern: null, dimmed: false });
    expect(off.scene.points.some((p) => p.color === COLORS.diaphragm)).toBe(false);
    expect(off.scene.labels.some((l) => l.className === "lbl-dia")).toBe(false);
  });
});
