import { describe, expect, it } from "vitest";

import { decodeCase, decodeShape, fromSummary, plain, type Geometry, type ResultsSummary } from "./resultCases";

/** Bytes as the desktop app sends them (gui/src-tauri/src/case_codec.rs): a magic word, then floats. */
function binary(values: number[]): ArrayBuffer {
  const buf = new ArrayBuffer(8 + 8 * values.length);
  new Uint8Array(buf).set(new TextEncoder().encode("OWB1"));
  new Float64Array(buf, 8).set(values);
  return buf;
}

const axes: Geometry["frames"][number]["axes"] = [
  [1, 0, 0],
  [0, 0, 1],
  [0, -1, 0],
];

describe("binary case values", () => {
  it("are put back into the objects of the geometry, in its order", () => {
    const g: Geometry = {
      displacements: ["1", "2"],
      reactions: ["1"],
      frames: [{ element: "M1", nodes: ["1", "2"], length: 4, offsets: [0.2, 0], axes }],
      links: [],
      shells: [{ element: "F1-1", area: "F1", nodes: ["1", "2", "3"], axes, corners: ["1", "2", "3"] }],
    };
    const v = [
      ...[0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6], // displacements
      ...[7, 8, 9, 10, 11, 12], // reaction
      2, ...[0, 1, 2, 3, 4, 5, 6, 7, 8, 9], ...[4, 11, 12, 13, 14, 15, 16, 17, 18, 19], // two stations
      ...Array.from({ length: 24 }, (_, i) => 100 + i), // three corners
    ];
    const head = { id: "DEAD", equilibrium: { applied: [], reactions: [], relative_error: 0, residual: 0 }, has_shells: true };
    const read = decodeCase(binary(v), head, g);
    const c = plain(read);
    expect(c.id).toBe("DEAD");
    expect(c).not.toHaveProperty("has_shells");
    expect(c.displacements[1]).toEqual({ node: "2", ux: 1, uy: 2, uz: 3, rx: 4, ry: 5, rz: 6 });
    expect(c.reactions).toEqual([{ node: "1", fx: 7, fy: 8, fz: 9, mx: 10, my: 11, mz: 12 }]);
    expect(c.frames[0]).toMatchObject({ element: "M1", nodes: ["1", "2"], length: 4, offsets: [0.2, 0], axes });
    expect(c.frames[0].stations[1]).toEqual({ x: 4, P: 11, V2: 12, V3: 13, T: 14, M2: 15, M3: 16, ux: 17, uy: 18, uz: 19 });
    expect(c.shells![0].corners[2]).toEqual({ node: "3", F11: 116, F22: 117, F12: 118, M11: 119, M22: 120, M12: 121, V13: 122, V23: 123 });
    expect(c.links).toEqual([]);
    // Rows read their numbers in place: the same values through their fields, the same object each time, and
    // the same JSON as the plain objects.
    expect(read.shells![0].corners[2].M11).toBe(119);
    expect(read.frames[0].stations[1].uz).toBe(19);
    expect(read.displacements[1].rz).toBe(6);
    expect(read.displacements[1]).toBe(read.displacements[1]);
    expect(JSON.parse(JSON.stringify(read))).toEqual(c);
  });

  it("a case sent as JSON is read as it is", () => {
    const text = new TextEncoder().encode('OWJ1\0\0\0\0{"id":"W","displacements":[],"reactions":[],"frames":[],"links":[]}');
    const buf = text.buffer.slice(text.byteOffset, text.byteOffset + text.byteLength);
    expect(decodeCase(buf as ArrayBuffer, { id: "W" }, undefined)).toEqual({ id: "W", displacements: [], reactions: [], frames: [], links: [] });
  });
});

describe("mode shapes on demand", () => {
  it("a summary without shapes gives pending modes; a shape is read from its bytes in the joints' order", () => {
    const mode = { mode: 1, period: 0.5, frequency: 2, circular_frequency: 12.566, eigenvalue: 157.9, participation: [1, 0, 0, 0, 0, 0], mass_ratio: [0.8, 0, 0, 0, 0, 0], sum_mass_ratio: [0.8, 0, 0, 0, 0, 0] };
    const summary = {
      title: "",
      units: { force: "kN", length: "m" },
      notes: [],
      cases: [],
      modal: { id: "Modal", modes: [mode], masses: [], free_mass: [1, 1, 1, 0, 0, 0], centre_of_mass: [0, 0, 0], joints: ["1", "~2"] },
    } as unknown as ResultsSummary;
    const r = fromSummary(summary);
    expect(r.modal!.modes[0]).toEqual({ ...mode, shape: [], pending: true });
    const shape = decodeShape(binary([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]), r.modal!.joints);
    expect(shape[1].ry).toBe(11);
    expect(plain(shape)).toEqual([
      { node: "1", ux: 1, uy: 2, uz: 3, rx: 4, ry: 5, rz: 6 },
      { node: "~2", ux: 7, uy: 8, uz: 9, rx: 10, ry: 11, rz: 12 },
    ]);
    // Shapes in the summary (the browser's worker sends them) stay as they are.
    const full = { ...summary, modal: { ...summary.modal!, modes: [{ ...mode, shape: [{ node: "1", ux: 1, uy: 0, uz: 0, rx: 0, ry: 0, rz: 0 }] }] } };
    expect(fromSummary(full).modal!.modes[0].pending).toBeUndefined();
  });
});
