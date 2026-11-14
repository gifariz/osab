// Stories and labels in the app: Define > Story Data moves the structure with its stories, adds and
// deletes stories; labels show in Properties; All stories selects the same label on every story.

import { expect, test, type Page } from "@playwright/test";

import { clickAt, menu, openApp } from "./helpers";

/** Stories Base 0, Story1 3, Story2 6: columns at x = 0 and 6 on both stories and a beam at each level. */
async function openFrame(page: Page) {
  await openApp(page);
  await page.waitForFunction(() => window.__osab !== undefined);
  await page.evaluate(() => {
    const m = structuredClone(window.__osab!.state().model);
    m.gui!.grid!.stories = [
      { label: "Base", z: 0 },
      { label: "Story1", z: 3 },
      { label: "Story2", z: 6 },
    ];
    const id: Record<string, string> = {};
    m.nodes = [];
    for (const x of [0, 6])
      for (const z of [0, 3, 6]) {
        id[`${x},${z}`] = String(m.nodes.length + 1);
        m.nodes.push({ id: id[`${x},${z}`], x, y: 0, z });
      }
    m.frames = [];
    const add = (a: string, b: string) => m.frames.push({ id: `M${m.frames.length + 1}`, type: "frame", nodes: [id[a], id[b]], material: "Steel", section: "FSEC1" });
    for (const x of [0, 6]) {
      add(`${x},0`, `${x},3`);
      add(`${x},3`, `${x},6`);
    }
    add("0,3", "6,3");
    add("0,6", "6,6");
    m.supports = [
      { node: id["0,0"], fix: "fixed" },
      { node: id["6,0"], fix: "fixed" },
    ];
    window.__osab!.open(m, "stories.json");
  });
}
const model = (page: Page) => page.evaluate(() => window.__osab!.state().model);

test("Story Data: a new height moves the stories above, a new story and a deleted one", async ({ page }) => {
  await openFrame(page);
  await menu(page, "Define", "Story Data");
  const dlg = page.getByRole("dialog", { name: "Story Data" });
  await dlg.getByLabel("Story height 1").fill("4");
  await dlg.getByRole("button", { name: "OK" }).click();
  let m = await model(page);
  expect(m.nodes.map((n) => n.z)).toEqual([0, 4, 7, 0, 4, 7]);
  expect(m.gui!.grid!.stories.map((s) => [s.label, s.z])).toEqual([["Base", 0], ["Story1", 4], ["Story2", 7]]);

  // Delete Story1: its columns and beam go, Story2's columns reach down to the base.
  await menu(page, "Define", "Story Data");
  await dlg.getByRole("button", { name: "Delete story Story1" }).click();
  await expect(dlg).toContainText("Deleting Story1 deletes its structure: 3 object(s)");
  await dlg.getByRole("button", { name: "OK" }).click();
  m = await model(page);
  expect(m.gui!.grid!.stories.map((s) => [s.label, s.z])).toEqual([["Base", 0], ["Story2", 7]]);
  expect(m.frames.map((f) => f.id)).toEqual(["M2", "M4", "M6"]);
  expect(m.nodes.map((n) => n.z).sort()).toEqual([0, 0, 7, 7]);
  await expect(page.locator(".notice, [role=status]").first()).toBeVisible();

  // Undo puts it back in one step; then a new story on top of the base (3 m) lifts the rest.
  await page.keyboard.press("Control+z");
  expect((await model(page)).frames).toHaveLength(6);
  await menu(page, "Define", "Story Data");
  await dlg.getByRole("button", { name: "Add a story above Base" }).click();
  await dlg.getByRole("button", { name: "OK" }).click();
  m = await model(page);
  expect(m.gui!.grid!.stories.map((s) => s.z)).toEqual([0, 3, 7, 10]);
});

test("labels show in Properties; All stories selects the same label on every story", async ({ page }) => {
  await openFrame(page);
  await page.getByRole("button", { name: "Elev X-Z" }).click();
  await clickAt(page, [0, 0, 1.5]); // the Story1 column at x = 0
  const props = page.getByLabel("Assignments and loads");
  await expect(props).toContainText("C1");
  await expect(props).toContainText("Story1");
  expect((await page.evaluate(() => window.__osab!.state().selection)).frames).toEqual(["M1"]);

  await page.keyboard.press("Escape");
  await page.getByLabel("Selection on stories").selectOption("all");
  await clickAt(page, [0, 0, 1.5]);
  const sel = await page.evaluate(() => window.__osab!.state().selection);
  expect(sel.frames).toEqual(["M1", "M2"]);
});
