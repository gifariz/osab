import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";

import { en } from "./en";
import { id } from "./id";
import { placeholders, setLang, t, tn } from "./index";

const src = join(fileURLToPath(new URL(".", import.meta.url)), "..");

function files(dir: string, ext: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    if (statSync(p).isDirectory()) return files(p, ext);
    return p.endsWith(ext) && !p.includes(".test.") ? [p] : [];
  });
}

describe("language files", () => {
  afterEach(() => setLang("en"));

  it("Indonesian has every English key, no empty text and the same placeholders", () => {
    for (const key of Object.keys(en) as (keyof typeof en)[]) {
      expect(id[key], key).toBeTruthy();
      expect(placeholders(id[key]), key).toEqual(placeholders(en[key]));
    }
    expect(Object.keys(id).sort()).toEqual(Object.keys(en).sort());
  });

  it("fills placeholders and picks the singular form", () => {
    expect(t("status.joint", { id: "7", x: 1, y: 2, z: 3 })).toBe("Joint 7  (1, 2, 3)");
    expect(tn("side.nJoints", 1)).toBe("1 joint");
    expect(tn("side.nJoints", 3)).toBe("3 joints");
    setLang("id");
    expect(t("cmd.save")).toBe("Simpan");
    expect(tn("side.nMembers", 2)).toBe("2 batang");
  });

  it("every t()/tn() key used in the code exists", () => {
    const used = new Set<string>();
    for (const f of [...files(src, ".ts"), ...files(src, ".tsx")]) {
      for (const m of readFileSync(f, "utf8").matchAll(/\btn?\(\s*"([a-z0-9]+\.[A-Za-z0-9_.]+)"/g)) used.add(m[1]);
    }
    const missing = [...used].filter((k) => !(k in en));
    expect(missing).toEqual([]);
  });

  it("no plain English text is left in the UI components", () => {
    const problems: string[] = [];
    for (const f of files(join(src, "ui"), ".tsx")) {
      const text = readFileSync(f, "utf8");
      // JSX text between tags (ignoring {expressions}) and literal title/placeholder/aria-label attributes.
      for (const m of text.matchAll(/>([^<>{}]*[A-Za-z]{2,}[^<>{}]*)</g)) {
        // Skip TypeScript between generics and comparisons: real JSX text has no code characters.
        if (!/[;=()"'`|&?:]|const|return/.test(m[1])) problems.push(`${f}: ${m[1].trim()}`);
      }
      for (const m of text.matchAll(/\s(title|placeholder|aria-label|label)="([^"]*[A-Za-z]{2,}[^"]*)"/g)) problems.push(`${f}: ${m[1]}="${m[2]}"`);
    }
    expect(problems).toEqual([]);
  });

  it("no other program is named in the UI text (OSAB describes features in its own words)", () => {
    const names = /\b(ETABS|SAFE|SAP2000|CSI|OpenSees|Midas|AutoCAD)\b/;
    const named = [...Object.entries(en), ...Object.entries(id)].filter(([, v]) => names.test(v)).map(([k]) => k);
    expect(named).toEqual([]);
  });
});
