import { describe, expect, it } from "vitest";

import { sheetRows } from "../dialogs/TablesDialog";
import { columnName, crc32, sheetNames, workbook, zip } from "./xlsx";

/** The files of a stored (uncompressed) zip, each checked against its CRC-32. */
function unzip(data: Uint8Array): Record<string, string> {
  const v = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const out: Record<string, string> = {};
  let at = 0;
  while (v.getUint32(at, true) === 0x04034b50) {
    const crc = v.getUint32(at + 14, true);
    const size = v.getUint32(at + 18, true);
    const nameLen = v.getUint16(at + 26, true);
    const name = new TextDecoder().decode(data.subarray(at + 30, at + 30 + nameLen));
    const body = data.subarray(at + 30 + nameLen, at + 30 + nameLen + size);
    expect(crc32(body), name).toBe(crc);
    out[name] = new TextDecoder().decode(body);
    at += 30 + nameLen + size;
  }
  expect(v.getUint32(data.length - 22, true)).toBe(0x06054b50);
  expect(v.getUint16(data.length - 22 + 10, true)).toBe(Object.keys(out).length);
  return out;
}

describe("xlsx writer", () => {
  it("CRC-32 and column letters", () => {
    expect(crc32(new TextEncoder().encode("123456789"))).toBe(0xcbf43926);
    expect([0, 25, 26, 27, 701, 702].map(columnName)).toEqual(["A", "Z", "AA", "AB", "ZZ", "AAA"]);
  });

  it("sheet names are short, clean and different", () => {
    expect(sheetNames(["Joint Displacements", "a/b", "Joint Displacements", "x".repeat(40)])).toEqual([
      "Joint Displacements",
      "a b",
      "Joint Displacements 2",
      "x".repeat(31),
    ]);
  });

  it("a workbook holds each sheet: text, numbers and bold headings", () => {
    const files = unzip(workbook([{ name: "T1", rows: [["Joint", "UX"], ["J<1>", 1.5e-3]], boldRows: 1 }, { name: "T2", rows: [["", 2]] }]));
    expect(Object.keys(files)).toEqual([
      "[Content_Types].xml",
      "_rels/.rels",
      "xl/workbook.xml",
      "xl/_rels/workbook.xml.rels",
      "xl/styles.xml",
      "xl/worksheets/sheet1.xml",
      "xl/worksheets/sheet2.xml",
    ]);
    expect(files["xl/workbook.xml"]).toContain('<sheet name="T1" sheetId="1" r:id="rId1"/>');
    const s1 = files["xl/worksheets/sheet1.xml"];
    expect(s1).toContain('<c r="A1" s="1" t="inlineStr"><is><t xml:space="preserve">Joint</t></is></c>');
    expect(s1).toContain("J&lt;1&gt;");
    expect(s1).toContain('<c r="B2"><v>0.0015</v></c>');
    // An empty text cell is left out.
    expect(files["xl/worksheets/sheet2.xml"]).toContain('<row r="1"><c r="B1"><v>2</v></c></row>');
    expect(zip([]).length).toBe(22);
  });

  it("a table's sheet: its name, the column names, their units, the values", () => {
    const r = sheetRows("Joint Reactions", { head: ["Joint", "FX (kN)"], qs: [null, "force"], rows: [["1", 2000]] }, (v) => (typeof v === "number" ? v / 1000 : v));
    expect(r.rows).toEqual([["TABLE: Joint Reactions"], ["Joint", "FX"], ["", "kN"], ["1", 2]]);
    expect(r.boldRows).toBe(3);
  });
});
