import { describe, expect, it } from "vitest";

import { setLang } from "../i18n";
import { normalizeModel, setNodeLoads, setSprings } from "../model/ops";
import type { Model } from "../model/types";
import { useStore } from "../state/store";
import { objectProps, type Prop } from "./ObjectProperties";
import b05 from "../../../tests/benchmarks/b05_portal_frame/model.json";

setLang("en");
const row = (p: Prop) =>
  p.rows
    ? {
        value: p.value,
        rows: Object.fromEntries(p.rows.map((r) => [r.label, r.value])),
      }
    : p.value;
/** Each group's rows by label (loads: pattern -> text). */
const groups = (m: Model, kind: "node" | "frame" | "link" | "area", id: string) =>
  Object.fromEntries(objectProps(m, kind, id).map((g) => [g.label, Object.fromEntries(g.rows.map((p) => [p.label, row(p)]))]));
const find = (m: Model, kind: "node" | "frame" | "area", id: string, label: string) =>
  objectProps(m, kind, id)
    .flatMap((g) => g.rows)
    .find((p) => p.label === label)!;

describe("properties of one object", () => {
  it("a frame: Geometry, Assignments (multi-value ones as a summary with their values) and Loads of every pattern", () => {
    let m = normalizeModel(b05);
    m.frames[1] = {
      ...m.frames[1],
      modifiers: { I33: 0.35 },
      releases: { j: ["M2", "M3"] },
      end_offsets: { i: 0.2, j: 0.3 },
      rigid_zone: 0.5,
    };
    m = {
      ...m,
      load_patterns: m.load_patterns.map((p) =>
        p.id === "WIND"
          ? {
              ...p,
              frame_loads: [
                {
                  type: "point" as const,
                  frame: "2",
                  dir: "Z" as const,
                  p: -4,
                  at: 3,
                },
              ],
            }
          : p,
      ),
    };
    const p = groups(m, "frame", "2");
    expect(Object.keys(p)).toEqual(["Geometry", "Assignments", "Loads"]);
    expect(p.Geometry).toMatchObject({ Beam: "2", Length: "6 m" });
    expect(p.Assignments).toMatchObject({
      Section: "BEAM",
      Material: "steel",
      "Axis angle": "0°",
      "In mesh": "Yes",
    });
    expect(p.Assignments.Modifiers).toEqual({
      value: "Specified",
      rows: {
        A: "1",
        As2: "1",
        As3: "1",
        J: "1",
        I22: "1",
        I33: "0.35",
        mass: "1",
        weight: "1",
      },
    });
    expect(p.Assignments.Releases).toEqual({
      value: "Specified",
      rows: { "End i": "None", "End j": "M2, M3" },
    });
    expect(p.Assignments["End offsets"]).toEqual({
      value: "Specified",
      rows: { "End i": "0.2 m", "End j": "0.3 m", "Rigid-zone factor": "0.5" },
    });
    expect(p.Loads).toEqual({
      DEAD: expect.stringContaining("15"),
      WIND: expect.stringContaining("4"),
    });
    // A plain column: defaults, no loads.
    expect(groups(m, "frame", "1")).toMatchObject({
      Assignments: {
        Modifiers: { value: "Default" },
        Releases: { value: "None" },
        "End offsets": { value: "None" },
      },
      Loads: { None: "" },
    });
  });

  it("a joint: restraints, springs, masses, diaphragm and loads", () => {
    let m = normalizeModel(b05);
    m = setSprings(m, ["2"], [1000, 0, 0, 0, 0, 0], "replace");
    m = setNodeLoads(m, "DEAD", ["2"], { fz: -3 }, "add");
    m.masses = [{ node: "2", ux: 2 }];
    const p = groups(m, "node", "2");
    expect(p.Geometry).toMatchObject({ Joint: "2", X: "0 m", Z: "4 m" });
    expect(p.Assignments).toMatchObject({
      Restraints: "Free",
      Diaphragm: "None",
    });
    expect(p.Assignments.Springs).toMatchObject({
      value: "Specified",
      rows: { UX: "1000 kN/m", UY: "0 kN/m" },
    });
    expect(p.Assignments.Masses).toMatchObject({ value: "Specified" });
    expect(Object.keys(p.Loads)).toEqual(["DEAD", "WIND"]);
    expect(groups(m, "node", "1").Assignments).toMatchObject({
      Restraints: "Fixed",
      Springs: { value: "None" },
      Masses: { value: "None" },
    });
  });

  it("values edited in place change the model (one undo step each); long names are tooltips", () => {
    useStore.getState().load(normalizeModel(b05), "b05.json", null);
    const m = () => useStore.getState().model;
    const section = find(m(), "frame", "2", "Section").edit!;
    expect(section.kind === "select" && section.options.map((o) => o.value)).toEqual(["", "COL", "BEAM"]);
    if (section.kind === "select") section.apply("COL");
    expect(m().frames.find((f) => f.id === "2")!.section).toBe("COL");

    const angle = find(m(), "frame", "2", "Axis angle");
    expect(angle.title).toBe("Local axis angle");
    if (angle.edit?.kind === "number") angle.edit.apply(30);
    expect(m().frames.find((f) => f.id === "2")!.angle).toBe(30);

    const restraints = find(m(), "node", "1", "Restraints").edit!;
    if (restraints.kind === "select") restraints.apply("pinned");
    expect(m().supports.find((s) => s.node === "1")!.fix).toBe("pinned");

    const z = find(m(), "node", "2", "Z").edit!;
    expect(z.kind === "number" && z.read("4.5")).toBe(4.5);
    if (z.kind === "number") z.apply(4.5);
    expect(m().nodes.find((n) => n.id === "2")!.z).toBe(4.5);
    expect(useStore.getState().past.length).toBe(4);
  });
});
