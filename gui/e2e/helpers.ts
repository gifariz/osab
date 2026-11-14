// Shared steps for the click tests: find model points on screen, click them, use menus and dialogs,
// and compare results with a benchmark's reference values.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { expect, type Page } from "@playwright/test";

import { referenceChecks, type IdMap, type RefValue } from "./compare";
import { normalizeModel } from "../src/model/ops";
import type { Model } from "../src/model/types";
import { namesByKind } from "../src/model/relabel";

const repo = fileURLToPath(new URL("../../", import.meta.url));

/**
 * Loads the app and waits until its test hook (`window.__osab`, set once the 3D view is up) is there, so a
 * test never asks for it a moment too early.
 */
export async function openApp(page: Page, url = "./") {
  await page.goto(url);
  await page.waitForFunction(() => !!window.__osab);
}

/** Opens every row of the model explorer (it starts collapsed). */
export async function expandTree(page: Page) {
  const closed = page.getByRole("tree", { name: "Model explorer" }).locator(".tree-box", { hasText: "+" });
  while ((await closed.count()) > 0) await closed.first().click();
}

/** A file of tests/benchmarks/<name>/, parsed as JSON. */
export const bench = (name: string, f: string) => JSON.parse(readFileSync(`${repo}tests/benchmarks/${name}/${f}`, "utf8"));

/**
 * Benchmark ids -> the ids of the benchmark opened as an example (the app's names). Mesh joints
 * (`~3`) keep their names; frame pieces and mesh elements (`M1-2`, `A1-3`) follow their object.
 */
export function exampleIds(name: string): IdMap {
  const r = namesByKind(normalizeModel(bench(name, "model.json")));
  const lookup = (map: Map<string, string>) => (id: string) => {
    if (map.has(id)) return map.get(id)!;
    const cut = id.lastIndexOf("-");
    return cut > 0 && map.has(id.slice(0, cut)) ? map.get(id.slice(0, cut)) + id.slice(cut) : id;
  };
  const members = new Map([...r.frames, ...r.links]);
  return { node: lookup(r.nodes), element: lookup(members), area: lookup(r.areas), shell: lookup(r.areas) };
}

/**
 * Benchmark ids, which are labels (C1, B1, F1, L1…), -> the ids of a model drawn in the app (M1, A1, K1…),
 * through the labels the app gave its objects; frame pieces and mesh elements (`F1-3`) follow their object.
 */
export function labelIds(m: Model, node: (id: string) => string = (id) => id): IdMap {
  const of = (list: { id: string; label?: string }[]) => new Map(list.filter((o) => o.label).map((o) => [o.label!, o.id]));
  const lookup = (map: Map<string, string>) => (id: string) => {
    if (map.has(id)) return map.get(id)!;
    const cut = id.lastIndexOf("-");
    return cut > 0 && map.has(id.slice(0, cut)) ? map.get(id.slice(0, cut)) + id.slice(cut) : id;
  };
  const areas = lookup(of(m.areas));
  return { node, element: lookup(of([...m.frames, ...m.links])), area: areas, shell: areas };
}

export type Vec3 = [number, number, number];

/** Screen position of a model point (the app exposes its viewport projection for tests). */
export async function screen(page: Page, p: Vec3) {
  return page.evaluate((pt) => {
    const r = document.querySelector("[data-testid=viewport]")!.getBoundingClientRect();
    const s = window.__osab!.project(pt as Vec3)!;
    return { x: r.left + s.x, y: r.top + s.y };
  }, p);
}

export async function clickAt(page: Page, p: Vec3, button: "left" | "right" = "left") {
  const s = await screen(page, p);
  await page.mouse.move(s.x, s.y);
  await page.mouse.click(s.x, s.y, { button });
}

export async function fill(page: Page, label: string, value: string) {
  const input = page.getByRole("dialog").getByLabel(label, { exact: true });
  await input.fill(value);
}

export async function menu(page: Page, title: string, item: string) {
  await page.locator(".menubar").getByRole("button", { name: title, exact: true }).click();
  await page.getByRole("menuitem", { name: new RegExp(item) }).click();
}

/**
 * Display > Tables, once its chooser is open: ticks `tables` in the tree, picks the cases (result keys
 * such as "ENV|max"; all when not given) and presses OK. The viewer opens on the first table.
 */
export async function chooseTables(page: Page, tables: string[], cases?: string[]) {
  const dlg = page.getByRole("dialog", { name: "Choose Tables" });
  for (const name of tables) await dlg.getByLabel(name, { exact: true }).check();
  if (cases) await dlg.getByLabel("Load cases for tables").selectOption(cases);
  const shots = process.env.OSAB_SHOTS;
  if (shots) await page.screenshot({ path: `${shots}/tables-choose-${tables[0].replace(/\W+/g, "-")}.png` });
  await dlg.getByRole("button", { name: "OK" }).click();
  await expect(page.getByRole("dialog", { name: "Tables", exact: true })).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/tables-view-${tables[0].replace(/\W+/g, "-")}.png` });
}

/**
 * Define > Load Patterns: the new model's Dead becomes DEAD (its case follows the name) with self-weight
 * multiplier 0, and Live goes (with its case). A new model has Dead x 1 and Live; the click tests that
 * rebuild a benchmark compare with references made with a DEAD case and without self-weight.
 */
export async function noSelfWeight(page: Page) {
  await menu(page, "Define", "Load Patterns");
  const dlg = page.getByRole("dialog");
  await dlg.getByLabel("Load patterns", { exact: true }).selectOption("Live");
  await dlg.getByRole("button", { name: "Delete", exact: true }).click();
  await dlg.getByLabel("Load patterns", { exact: true }).selectOption("Dead");
  await dlg.getByLabel("Name", { exact: true }).fill("DEAD");
  await dlg.getByLabel("Self weight multiplier", { exact: true }).fill("0");
  await dlg.getByRole("button", { name: "Apply", exact: true }).click();
  await dlg.getByRole("button", { name: "Close", exact: true }).click();
}

/** Define > Load Patterns: adds a pattern of a type (and with it a load case of the same name). */
export async function addPattern(page: Page, name: string, type = "Other") {
  await menu(page, "Define", "Load Patterns");
  const dlg = page.getByRole("dialog");
  await dlg.getByRole("button", { name: "Add New…" }).click();
  await dlg.getByLabel("Name", { exact: true }).fill(name);
  await dlg.getByLabel("Type", { exact: true }).selectOption({ label: type });
  await dlg.getByRole("button", { name: "Add", exact: true }).click();
  await dlg.getByRole("button", { name: "Close", exact: true }).click();
}

/**
 * Every member's end length offsets to None (Assign > Frame End Length Offsets). New members get automatic
 * offsets; the click tests that rebuild an older benchmark compare with references made without offsets. Selects everything in the 3D view first and leaves nothing selected.
 */
export async function noEndOffsets(page: Page) {
  await page.getByRole("button", { name: "3D" }).click();
  await page.keyboard.press("Control+a");
  await menu(page, "Assign", "Frame End Length Offsets");
  const dlg = page.getByRole("dialog");
  await dlg.getByLabel("None", { exact: true }).check();
  await dlg.getByRole("button", { name: "OK", exact: true }).click();
  await page.keyboard.press("Escape");
}

export type { IdMap, RefValue } from "./compare";

/**
 * Compares the app's current results with reference values (`referenceChecks`: `osab bench`'s rule).
 * Returns how many were compared.
 */
export async function compareResults(page: Page, refs: RefValue[], ids: IdMap = {}) {
  const results = (await page.evaluate(() => window.__osab!.allResults()))!;
  const checks = referenceChecks(results, refs, ids);
  const off = checks.filter((c) => !(Math.abs(c.got - c.want) <= c.tolerance)).map((c) => `${c.label}: ${c.got} vs ${c.want} (± ${c.tolerance})`);
  expect(off.slice(0, 10)).toEqual([]);
  return checks.length;
}
