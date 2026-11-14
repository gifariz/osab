import { describe, expect, it } from "vitest";

import { addAreaAt, divideAreas, upsertAreaSection } from "./areaOps";
import { checkModel } from "./check";
import { setAreaMesh } from "./areaOps";
import { moveNode, newModel, normalizeModel } from "./ops";
import { areaMeshText, canStoreMesh, removeStoredMeshes, storeMeshes, storedMeshProblem, userMeshOf } from "./storedMesh";
import type { MeshInfo, Model } from "./types";

/** A 4 x 4 m floor A1 and the mesh step's 2 x 2 mesh of it (mesh joints ~1..~5). */
function floor(): { m: Model; mesh: MeshInfo } {
  const m = addAreaAt(newModel({ xSpacings: [4], ySpacings: [4], storyHeights: [3] }), [[0, 0, 3], [4, 0, 3], [4, 4, 3], [0, 4, 3]], "Slab1").model;
  const [a, b, c, d] = m.areas[0].nodes;
  const mesh: MeshInfo = {
    joints: [
      { id: "~1", x: 2, y: 0, z: 3 },
      { id: "~2", x: 4, y: 2, z: 3 },
      { id: "~3", x: 2, y: 4, z: 3 },
      { id: "~4", x: 0, y: 2, z: 3 },
      { id: "~5", x: 2, y: 2, z: 3 },
    ],
    shells: [
      { id: "A1-1", parent: "A1", nodes: [a, "~1", "~5", "~4"] },
      { id: "A1-2", parent: "A1", nodes: ["~1", b, "~2", "~5"] },
      { id: "A1-3", parent: "A1", nodes: ["~4", "~5", "~3", d] },
      { id: "A1-4", parent: "A1", nodes: ["~5", "~2", c, "~3"] },
    ],
    frames: [],
    edge_constraints: [],
  };
  return { m, mesh };
}

describe("stored meshes (Edit > Store Mesh)", () => {
  it("keeps the mesh step's elements of an area: joints by coordinates, elements by index", () => {
    const { m, mesh } = floor();
    const um = userMeshOf(m, mesh, "A1")!;
    expect(um.joints).toHaveLength(9);
    expect(um.elements).toEqual([
      [0, 1, 2, 3],
      [1, 4, 5, 2],
      [3, 2, 6, 7],
      [2, 5, 8, 6],
    ]);
    expect(um.joints[2]).toEqual([2, 2, 3]);
    const s = storeMeshes(m, new Map([["A1", um]]));
    expect(areaMeshText(s.areas[0])).toMatch(/stored \(4 elements\)/);
    // Kept in the file; removed again.
    expect(normalizeModel(JSON.parse(JSON.stringify(s))).areas[0].user_mesh).toEqual(um);
    expect(removeStoredMeshes(s, ["A1"]).areas[0]).not.toHaveProperty("user_mesh");
    // An area that is one element stores itself.
    expect(userMeshOf(m, { ...mesh, shells: [] }, "A1")!.elements).toEqual([[0, 1, 2, 3]]);
  });

  it("openings, null areas and Membrane floors have no mesh to store", () => {
    let { m, mesh } = floor();
    m = upsertAreaSection(m, "Slab1", { id: "Slab1", type: "membrane", material: "Concrete", thickness: 0.2 });
    expect(canStoreMesh(m, m.areas[0])).toBe(false);
    expect(userMeshOf(m, mesh, "A1")).toBeNull();
    m = addAreaAt(floor().m, [[1, 1, 3], [2, 1, 3], [2, 2, 3]], "", { opening: true }).model;
    expect(canStoreMesh(m, m.areas[1])).toBe(false);
  });

  it("a stored mesh that no longer fits is an error; divided pieces drop it; a stored polygon needs no auto mesh", () => {
    const { m, mesh } = floor();
    let s = storeMeshes(m, new Map([["A1", userMeshOf(m, mesh, "A1")!]]));
    expect(storedMeshProblem(s, s.areas[0])).toBeNull();
    const moved = moveNode(s, s.areas[0].nodes[2], [5, 4, 3]);
    expect(storedMeshProblem(moved, moved.areas[0])).toMatch(/joint lies outside|corner moved/);
    expect(checkModel(moved).some((i) => i.level === "error" && /stored mesh/.test(i.message))).toBe(true);
    expect(divideAreas(s, ["A1"], 2, 2).model.areas.every((a) => !a.user_mesh)).toBe(true);
    // A corner without a stored joint is fine where an opening covers it (the mesh goes round it).
    const um = {
      joints: [[0, 0, 3], [2, 0, 3], [2, 2, 3], [0, 2, 3], [4, 0, 3], [4, 2, 3], [2, 4, 3], [0, 4, 3]] as [number, number, number][],
      elements: [[0, 1, 2, 3], [1, 4, 5, 2], [3, 2, 6, 7]],
    };
    const cut = storeMeshes(m, new Map([["A1", um]]));
    expect(storedMeshProblem(cut, cut.areas[0])).toMatch(/corner moved/);
    const opened = addAreaAt(cut, [[2, 2, 3], [4.5, 2, 3], [4.5, 4.5, 3], [2, 4.5, 3]], "", { opening: true }).model;
    expect(storedMeshProblem(opened, opened.areas[0])).toBeNull();
    // Set to no mesh, an area keeps its stored elements, so it is not one element.
    s = setAreaMesh(s, ["A1"], "none");
    expect(checkModel(s).some((i) => i.areas?.includes("A1") && i.level === "error")).toBe(false);
  });

  it("an opening over stored meshes only (across two floors) is their hole already", () => {
    let m = addAreaAt(newModel({ xSpacings: [4, 4], ySpacings: [4], storyHeights: [3] }), [[0, 0, 3], [4, 0, 3], [4, 4, 3], [0, 4, 3]], "Slab1").model;
    m = addAreaAt(m, [[4, 0, 3], [8, 0, 3], [8, 4, 3], [4, 4, 3]], "Slab1").model;
    m = addAreaAt(m, [[3, 1, 3], [5, 1, 3], [5, 3, 3], [3, 3, 3]], "", { opening: true }).model;
    m = setAreaMesh(m, ["A1", "A2"], "none");
    const unmeshed = (x: Model) => checkModel(x).some((i) => i.level === "error" && /cannot have an opening/.test(i.message));
    expect(unmeshed(m)).toBe(true);
    const own = (x0: number) => ({ joints: [[x0, 0, 3], [x0 + 4, 0, 3], [x0 + 4, 4, 3], [x0, 4, 3]] as [number, number, number][], elements: [[0, 1, 2, 3]] });
    expect(unmeshed(storeMeshes(m, new Map([["A1", own(0)], ["A2", own(4)]])))).toBe(false);
  });
});
