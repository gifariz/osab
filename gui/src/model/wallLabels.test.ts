import { describe, expect, it } from "vitest";

import { normalizeModel as readModel } from "./ops";
import type { Model } from "./types";
import { assignWallLabel, deleteWallLabel, labelUsers, labelsOf, setWallLabel } from "./wallLabels";

/** A wall with a door (an opening) and a column at its edge. */
const model = (): Model =>
  readModel({
    materials: [{ id: "C", E: 25e6, nu: 0.2 }],
    sections: [{ id: "R", A: 0.12, I22: 1e-3, I33: 1e-3, J: 2e-3 }],
    area_sections: [{ id: "W", type: "shell-thin", material: "C", thickness: 0.2 }],
    nodes: [
      { id: "1", x: 0, y: 0, z: 0 },
      { id: "2", x: 6, y: 0, z: 0 },
      { id: "3", x: 6, y: 0, z: 3 },
      { id: "4", x: 0, y: 0, z: 3 },
      { id: "5", x: 2, y: 0, z: 0 },
      { id: "6", x: 4, y: 0, z: 0 },
      { id: "7", x: 4, y: 0, z: 2 },
      { id: "8", x: 2, y: 0, z: 2 },
    ],
    frames: [{ id: "F", type: "frame", nodes: ["1", "4"], material: "C", section: "R", pier: "P1" }],
    areas: [
      { id: "A", nodes: ["1", "2", "3", "4"], section: "W", pier: "P1", spandrel: "S1" },
      { id: "O", nodes: ["5", "6", "7", "8"], opening: true, pier: "P1" },
    ],
    pier_labels: [{ id: "P1", color: "#ff0000" }],
    spandrel_labels: [{ id: "S1", multi_story: true }],
    load_cases: [],
  });

describe("pier and spandrel labels", () => {
  it("are read from a file (an opening takes none)", () => {
    const m = model();
    expect(m.pier_labels).toEqual([{ id: "P1", color: "#ff0000" }]);
    expect(m.spandrel_labels).toEqual([{ id: "S1", multi_story: true }]);
    expect(m.frames[0].pier).toBe("P1");
    expect(m.areas[0]).toMatchObject({ pier: "P1", spandrel: "S1" });
    expect(m.areas[1].pier).toBeUndefined();
    expect(labelUsers(m, "pier", "P1")).toBe(2);
  });

  it("are added, renamed (the objects follow) and deleted (the objects lose them)", () => {
    let m = setWallLabel(model(), "pier", null, { id: "P2" });
    expect(labelsOf(m, "pier").map((l) => l.id)).toEqual(["P1", "P2"]);
    expect(labelsOf(m, "pier")[1].color).toMatch(/^#/);
    expect(() => setWallLabel(m, "pier", null, { id: "P1" })).toThrow(/already/);
    expect(() => setWallLabel(m, "pier", null, { id: " " })).toThrow();
    m = setWallLabel(m, "pier", "P1", { id: "PA" });
    expect(m.frames[0].pier).toBe("PA");
    expect(m.areas[0].pier).toBe("PA");
    expect(labelsOf(m, "pier")[0].color).toBe("#ff0000");
    m = setWallLabel(m, "spandrel", "S1", { id: "S1", multi_story: false });
    expect(m.spandrel_labels).toEqual([{ id: "S1", color: expect.any(String) }]);
    m = deleteWallLabel(m, "pier", "PA");
    expect(m.frames[0].pier).toBeUndefined();
    expect(m.areas[0].pier).toBeUndefined();
    m = deleteWallLabel(m, "pier", "P2");
    expect(m.pier_labels).toBeUndefined();
  });

  it("are given to the selected frames and areas and taken away", () => {
    let m = setWallLabel(model(), "spandrel", null, { id: "S2" });
    m = assignWallLabel(m, "spandrel", "S2", { frames: ["F"], areas: ["A", "O"] });
    expect(m.frames[0].spandrel).toBe("S2");
    expect(m.areas[0].spandrel).toBe("S2");
    expect(m.areas[1].spandrel).toBeUndefined();
    m = assignWallLabel(m, "pier", null, { areas: ["A"] });
    expect(m.areas[0].pier).toBeUndefined();
    expect(m.frames[0].pier).toBe("P1");
    expect(() => assignWallLabel(m, "pier", "NOPE", { areas: ["A"] })).toThrow(/NOPE/);
  });
});
