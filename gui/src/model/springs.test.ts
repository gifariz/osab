import { describe, expect, it } from "vitest";

import { addAreaAt } from "./areaOps";
import { EditError, addFrame, addNode, newModel } from "./ops";
import { assignAreaSpring, assignLineSpring, deleteSpringProp, springUsers, upsertSpringProp } from "./springs";
import type { Model } from "./types";

/** A beam, a null frame, a floor and an opening. */
function model(): Model {
  let m = newModel({ xSpacings: [6], ySpacings: [6], storyHeights: [4] });
  for (const p of [[0, 0, 4], [6, 0, 4], [6, 6, 4]] as const) m = addNode(m, [...p]).model;
  const [a, b, c] = m.nodes.slice(-3).map((n) => n.id);
  m = addFrame(m, a, b, { type: "frame", material: "Steel", section: "FSEC1" }).model;
  m = addFrame(m, b, c, { type: "frame", material: "", section: "" }).model;
  m = addAreaAt(m, [[0, 0, 4], [6, 0, 4], [6, 6, 4], [0, 6, 4]], "Slab1").model;
  return addAreaAt(m, [[1, 1, 4], [2, 1, 4], [2, 2, 4], [1, 2, 4]], "", { opening: true }).model;
}

describe("line and area springs", () => {
  it("properties are added, renamed with their users, and deleted with them", () => {
    let m = upsertSpringProp(model(), "line", null, { id: " LS1 ", u2: 2000, u1: 0 });
    expect(m.line_spring_props).toEqual([{ id: "LS1", u2: 2000 }]);
    m = assignLineSpring(m, m.frames.map((f) => f.id), "LS1");
    // The null frame gets none.
    expect(m.frames.map((f) => f.line_spring)).toEqual(["LS1", undefined]);
    m = upsertSpringProp(m, "line", "LS1", { id: "SOIL", u2: 3000, r1: 10 });
    expect(m.line_spring_props).toEqual([{ id: "SOIL", u2: 3000, r1: 10 }]);
    expect(springUsers(m, "line", "SOIL")).toEqual([m.frames[0].id]);
    m = deleteSpringProp(m, "line", "SOIL");
    expect(m).not.toHaveProperty("line_spring_props");
    expect(m.frames[0]).not.toHaveProperty("line_spring");
  });

  it("area springs go on floors, not openings; bad values and names are refused", () => {
    let m = upsertSpringProp(model(), "area", null, { id: "AS1", u3: 20000 });
    m = assignAreaSpring(m, m.areas.map((a) => a.id), "AS1");
    expect(m.areas.map((a) => a.area_spring)).toEqual(["AS1", undefined]);
    expect(assignAreaSpring(m, [m.areas[0].id], null).areas[0]).not.toHaveProperty("area_spring");
    expect(() => upsertSpringProp(m, "area", null, { id: "AS1", u3: 1 })).toThrow(EditError);
    expect(() => upsertSpringProp(m, "area", null, { id: "AS2", u3: -1 })).toThrow(/0 or more/);
    expect(() => assignAreaSpring(m, [m.areas[0].id], "NONE")).toThrow(EditError);
  });
});
