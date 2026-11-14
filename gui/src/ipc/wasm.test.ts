// The WebAssembly solver (the browser's) on benchmark models: the reference results, and errors
// as data. Runs the module in Node; `npm test` builds it first (`npm run build:wasm`).

import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { beforeAll, describe, expect, it } from "vitest";

import { referenceChecks, type RefValue } from "../../e2e/compare";
import type { Results } from "../model/types";
import type { MeshView, Oriented, SpectrumPoints } from "./backend";
import { WasmSolver } from "./wasm";
import { normalizeModel as readModel } from "../model/ops";
import { buildTable } from "../ui/tables/tableData";
import { modelUnits } from "../units/display";

const wasmFile = fileURLToPath(new URL("./osab.wasm", import.meta.url));
const benchFile = (name: string, f: string) =>
  JSON.parse(readFileSync(fileURLToPath(new URL(`../../../tests/benchmarks/${name}/${f}`, import.meta.url)), "utf8"));

let solver: WasmSolver;
beforeAll(async () => {
  if (!existsSync(wasmFile)) throw new Error("src/ipc/osab.wasm is missing. Build it first: npm run build:wasm");
  solver = await WasmSolver.load(readFileSync(wasmFile));
});

describe("WebAssembly solver", () => {

  it("b67_pier_spandrel: pier and spandrel forces match statics; the tables show them", () => {
    const model = benchFile("b67_pier_spandrel", "model.json");
    const reply = solver.call<Results>("analyze", model);
    expect(reply.error).toBeUndefined();
    const r = reply.ok!;
    for (const [file, n] of [
      ["hand.json", 84],
    ] as const) {
      const refs: RefValue[] = benchFile("b67_pier_spandrel", file).values;
      const off = referenceChecks(r, refs).filter((c) => !(Math.abs(c.got - c.want) <= c.tolerance));
      expect(off.slice(0, 5)).toEqual([]);
      expect(refs.length).toBe(n);
    }
    // The tables: Pier Forces of case L as the reference program prints them (kN, kN-m, 4 decimals).
    const m = readModel(model);
    const ctx = { model: m, results: r, cases: r.cases, heads: r.cases.filter((c) => c.id === "L"), patterns: null, U: modelUnits(m) };
    const piers = buildTable("pierForces", ctx);
    expect(piers.head.slice(0, 4)).toEqual(["Case", "Story", "Pier", "Location"]);
    const row = (end: string) => piers.rows.find((x) => x[3] === end)!.slice(4).map((v) => Number((v as number).toFixed(4)));
    expect(row("Top")).toEqual([-17, 11, 13, 68, -19, 74]);
    expect(row("Bottom")).toEqual([-17, 11, 13, 68, 20, 107]);
    const spandrels = buildTable("spandrelForces", ctx);
    const left = spandrels.rows.find((x) => x[3] === "Left")!.slice(4) as number[];
    expect(left.map((v) => Number(v.toFixed(4)))).toEqual([14.0113, 7.9428, 2.9723, -2.3083, -0.989, 7.5944]);
    const section = buildTable("pierSections", ctx).rows[0];
    expect(section.slice(0, 2)).toEqual(["L1", "P"]);
    (section.slice(2) as number[]).forEach((v, k) => expect(v).toBeCloseTo([0, 3, 0, 0, 3, 0, 3][k], 12));
    expect(buildTable("pierLabels", ctx).rows).toEqual([
      ["P", ""],
      ["Q", ""],
    ]);
    expect(buildTable("spandrelLabels", ctx).rows).toEqual([["S", "No", ""]]);
  });

  it("gives a spectrum function's points (ASCE 7-16 from the code's tables) and says what is wrong", () => {
    const ok = solver.call<SpectrumPoints>("spectrum", { id: "A", asce7_16: { ss: 0.8, s1: 0.35, tl: 8, site_class: "D" } });
    expect(ok.ok!.asce7_16!.fa).toBeCloseTo(1.18, 12);
    expect(ok.ok!.points[0]).toEqual([0, 0.4 * ok.ok!.asce7_16!.sds]);
    const bad = solver.call<SpectrumPoints>("spectrum", { id: "F", asce7_16: { ss: 0.8, s1: 0.35, tl: 8, site_class: "F" } });
    expect(bad.error!.message).toMatch(/fa and fv/);
  });


  it("answers the mesh and orient calls", () => {
    const model = benchFile("b42_l_floor_beams", "model.json");
    const view = solver.call<MeshView>("mesh", model);
    expect(view.ok!.mesh.shells.length).toBeGreaterThan(20);
    const o = solver.call<Oriented>("orient", model);
    expect(o.ok!.model.frames.length).toBe(model.frames.length);
  });

  it("returns errors as data, with the size limit's numbers", () => {
    const portal = benchFile("b05_portal_frame", "model.json");
    expect(solver.call("analyze", "{").error!.kind).toBe("json");
    const unstable = solver.call("analyze", { ...portal, supports: [], on_instability: "stop" }).error!;
    expect(unstable.kind).toBe("unstable");
    expect(unstable.node).toBeTruthy();
    // B05 has 4 joints.
    expect(solver.call("analyze", portal, 4).ok).toBeTruthy();
    const big = solver.call("analyze", portal, 3).error!;
    expect([big.kind, big.joints, big.limit]).toEqual(["too_big", 4, 3]);
    expect(solver.stopped).toBe(false);
  });

  it("a module that stops answers 'crash' and refuses further calls", async () => {
    // A minimal module with the same exports whose `call` traps (`unreachable`) and leaves no reply,
    // as the real one does when it runs out of memory.
    const name = (s: string) => [s.length, ...new TextEncoder().encode(s)];
    const section = (id: number, b: number[]) => [id, b.length, ...b];
    const i32 = 0x7f;
    const bytes = new Uint8Array([
      ...[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00],
      // Types: (i32) -> i32, (i32 x 5) -> (), () -> i32.
      ...section(1, [3, 0x60, 1, i32, 1, i32, 0x60, 5, i32, i32, i32, i32, i32, 0, 0x60, 0, 1, i32]),
      ...section(3, [4, 0, 1, 2, 2]),
      ...section(5, [1, 0, 1]),
      ...section(7, [5, ...name("memory"), 2, 0, ...name("alloc"), 0, 0, ...name("call"), 0, 1, ...name("reply_ptr"), 0, 2, ...name("reply_len"), 0, 3]),
      // alloc: 16; call: unreachable; reply_ptr, reply_len: 0.
      ...section(10, [4, 4, 0, 0x41, 16, 0x0b, 3, 0, 0x00, 0x0b, 4, 0, 0x41, 0, 0x0b, 4, 0, 0x41, 0, 0x0b]),
    ]);
    const broken = await WasmSolver.load(bytes);
    expect(broken.call("analyze", {})).toEqual({ error: { kind: "crash", message: "" } });
    expect(broken.stopped).toBe(true);
    expect(() => broken.call("analyze", {})).toThrow();
  });
});
