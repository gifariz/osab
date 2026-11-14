import { describe, expect, it } from "vitest";

import { ALL, GROUP_COLORS, assignGroup, deleteGroup, groupMembers, memberCount, readGroup, setGroup } from "./groups";
import { EditError, addFrame, addNode, deleteObjects, newModel, normalizeModel } from "./ops";
import type { Model } from "./types";

/** A model with three joints and two frames between them. */
const base = (): Model => {
  let m = newModel({ xSpacings: [6, 6], ySpacings: [6], storyHeights: [3] });
  for (const p of [
    [0, 0, 3],
    [6, 0, 3],
    [12, 0, 3],
  ] as [number, number, number][])
    m = addNode(m, p).model;
  const [a, b, c] = m.nodes.map((n) => n.id);
  const props = { type: "frame" as const, material: m.materials[0].id, section: m.sections[0].id };
  m = addFrame(m, a, b, props).model;
  return addFrame(m, b, c, props).model;
};

describe("groups", () => {
  it("are named and coloured; All is every object and cannot be made or changed", () => {
    let m = setGroup(base(), null, { id: "G1" });
    m = setGroup(m, null, { id: "G2", color: "#123456" });
    expect(m.groups).toEqual([
      { id: "G1", color: GROUP_COLORS[0] },
      { id: "G2", color: "#123456" },
    ]);
    expect(() => setGroup(m, null, { id: "all" })).toThrow(EditError);
    expect(() => setGroup(m, null, { id: "G1" })).toThrow(/already exists/);
    expect(() => assignGroup(m, ALL, { nodes: [m.nodes[0].id] })).toThrow(EditError);
    expect(memberCount(groupMembers(m, ALL))).toBe(m.nodes.length + m.frames.length + m.links.length + m.areas.length);
  });

  it("hold exactly the objects put in (an object in several), taken out on request and when deleted", () => {
    let m = setGroup(setGroup(base(), null, { id: "G1" }), null, { id: "G2" });
    const [f1, f2] = m.frames.map((f) => f.id);
    m = assignGroup(m, "G1", { frames: [f2, f1], nodes: [m.nodes[0].id] });
    m = assignGroup(m, "G2", { frames: [f2] });
    // In the model's order; a frame's joints are not added with it.
    expect(groupMembers(m, "G1")).toMatchObject({ frames: [f1, f2], nodes: [m.nodes[0].id] });
    expect(groupMembers(m, "G2").frames).toEqual([f2]);
    m = assignGroup(m, "G1", { nodes: [m.nodes[0].id] }, true);
    expect(m.groups![0].nodes).toBeUndefined();
    // Deleting an object takes it out of every group; deleting a group keeps its objects.
    m = deleteObjects(m, [], [f2]);
    expect(groupMembers(m, "G1").frames).toEqual([f1]);
    expect(m.groups![1].frames).toBeUndefined();
    const n = m.frames.length;
    m = deleteGroup(m, "G1");
    expect(m.groups!.map((g) => g.id)).toEqual(["G2"]);
    expect(m.frames.length).toBe(n);
    // Renaming keeps the objects and the colour.
    m = assignGroup(m, "G2", { frames: [f1] });
    const color = m.groups![0].color;
    m = setGroup(m, "G2", { id: "BEAMS" });
    expect(m.groups![0]).toMatchObject({ id: "BEAMS", color, frames: [f1] });
  });

  it("are read from files", () => {
    expect(readGroup({ id: 7, color: "#ff0000", frames: ["B1", "B1", 2], areas: [] })).toEqual({ id: "7", color: "#ff0000", frames: ["B1", "2"] });
    expect(normalizeModel({ groups: [{ id: "G", nodes: ["1"] }] }).groups).toEqual([{ id: "G", nodes: ["1"] }]);
  });
});
