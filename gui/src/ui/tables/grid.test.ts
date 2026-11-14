import { describe, expect, it } from "vitest";

import { cellTest, compareCells, viewRows, type Cell } from "./grid";

describe("table filters", () => {
  it("compare numbers, take ranges, else look for the text", () => {
    const pass = (f: string, cells: string[]) => cells.filter(cellTest(f)!);
    expect(pass("> 2", ["1", "2", "3", "", "x"])).toEqual(["3"]);
    expect(pass("<=-1.5", ["-2", "-1.5", "0"])).toEqual(["-2", "-1.5"]);
    expect(pass("!= 0", ["0", "1e-3", "-4"])).toEqual(["1e-3", "-4"]);
    expect(pass("= 0", ["0", "0.0", "1"])).toEqual(["0", "0.0"]);
    expect(pass("3..1", ["0.5", "1", "2.5", "3", "4"])).toEqual(["1", "2.5", "3"]);
    expect(pass("dead", ["DEAD", "LIVE", "Dead (Max)"])).toEqual(["DEAD", "Dead (Max)"]);
    expect(cellTest("  ")).toBeNull();
  });
});

describe("table sorting", () => {
  it("numbers by value, text in natural order, empty cells last", () => {
    expect([3, -1, "", 2].sort(compareCells)).toEqual([-1, 2, 3, ""]);
    expect(["A10", "A2", "b1", "A1"].sort(compareCells)).toEqual(["A1", "A2", "A10", "b1"]);
  });

  it("filters every column, then sorts stably both ways", () => {
    const rows: Cell[][] = [
      ["C1", 2],
      ["C2", -5],
      ["C3", 2],
      ["D1", ""],
      ["D2", 7],
    ];
    const shown = (v: Cell) => String(v);
    expect(viewRows(rows, shown, ["c", ""], null)).toEqual([0, 1, 2]);
    expect(viewRows(rows, shown, ["", ""], { col: 1, dir: 1 })).toEqual([1, 0, 2, 4, 3]);
    expect(viewRows(rows, shown, ["", ""], { col: 1, dir: -1 })).toEqual([4, 0, 2, 1, 3]);
    expect(viewRows(rows, shown, ["", ">0"], { col: 0, dir: -1 })).toEqual([4, 2, 0]);
  });
});
