// Edit tools in the app: special joints (Assign > Special Joint) stay when their objects are deleted.

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
    m.nodes = [];
    for (const x of [0, 6]) for (const z of [0, 3, 6]) m.nodes.push({ id: String(m.nodes.length + 1), x, y: 0, z });
    const id = (x: number, z: number) => m.nodes.find((n) => n.x === x && n.z === z)!.id;
    m.frames = [];
    const add = (a: string, b: string) => m.frames.push({ id: `M${m.frames.length + 1}`, type: "frame", nodes: [a, b], material: "Steel", section: "FSEC1" });
    for (const x of [0, 6]) {
      add(id(x, 0), id(x, 3));
      add(id(x, 3), id(x, 6));
    }
    add(id(0, 3), id(6, 3));
    add(id(0, 6), id(6, 6));
    m.supports = [
      { node: id(0, 0), fix: "fixed" },
      { node: id(6, 0), fix: "fixed" },
    ];
    window.__osab!.open(m, "frame.json");
  });
}
const model = (page: Page) => page.evaluate(() => window.__osab!.state().model);
const jointAt = async (page: Page, x: number, z: number) => (await model(page)).nodes.find((n) => n.x === x && n.z === z);

test("a special joint stays when its members are deleted; a plain one goes", async ({ page }) => {
  await openFrame(page);
  // Mark the top right joint special.
  await clickAt(page, [6, 0, 6]);
  await menu(page, "Assign", "Special Joint");
  const dlg = page.getByRole("dialog", { name: "Special Joint (1 joint)" });
  await dlg.getByLabel("Special joint").check();
  await dlg.getByRole("button", { name: "OK" }).click();
  expect((await jointAt(page, 6, 6))?.special).toBe(true);
  await expect(page.locator(".side-panel")).toContainText(/Special joint\s*Yes/);

  // Delete the Story2 column at x = 6 and the top beam: the joint stays, with nothing attached.
  await page.keyboard.press("Escape");
  await clickAt(page, [6, 0, 4.5]);
  await clickAt(page, [3, 0, 6]);
  await page.keyboard.press("Delete");
  expect((await model(page)).frames.map((f) => f.id)).toEqual(["M1", "M2", "M3", "M5"]);
  expect(await jointAt(page, 6, 6)).toBeDefined();

  // Turning Special off for that joint, now with nothing attached, deletes it at once.
  await clickAt(page, [6, 0, 6]);
  await menu(page, "Assign", "Special Joint");
  await expect(dlg.getByLabel("Special joint")).toBeChecked();
  await dlg.getByLabel("Special joint").uncheck();
  await dlg.getByRole("button", { name: "OK" }).click();
  expect(await jointAt(page, 6, 6)).toBeUndefined();

  // Without the special flag the same delete takes the joint too.
  for (let k = 0; k < 3; k++) await page.keyboard.press("Control+z");
  expect((await jointAt(page, 6, 6))?.special).toBeUndefined();
  await page.keyboard.press("Escape");
  await clickAt(page, [6, 0, 4.5]);
  await clickAt(page, [3, 0, 6]);
  await page.keyboard.press("Delete");
  expect(await jointAt(page, 6, 6)).toBeUndefined();
  expect(await jointAt(page, 0, 6)).toBeDefined(); // still used by the column at x = 0
});

const frameAt = async (page: Page, a: [number, number, number], b: [number, number, number]) => {
  const m = await model(page);
  const at = (id: string) => {
    const n = m.nodes.find((x) => x.id === id)!;
    return [n.x, n.y, n.z].join(",");
  };
  return m.frames.find((f) => {
    const ends = f.nodes.map(at).sort();
    return ends[0] === [a, b].map((p) => p.join(",")).sort()[0] && ends[1] === [a, b].map((p) => p.join(",")).sort()[1];
  });
};

test("Edit > Move: a beam moved up leaves its shared joints; a joint moved by two picked points stretches its column", async ({ page }) => {
  await openFrame(page);
  await clickAt(page, [3, 0, 6]); // the top beam
  await page.keyboard.press("Control+m");
  const dlg = page.getByRole("dialog", { name: "Move" });
  await expect(dlg).toContainText("Selected: 0 joints, 1 frames, 0 links, 0 areas.");
  await dlg.getByLabel("dZ").fill("1");
  await dlg.getByRole("button", { name: "OK" }).click();
  expect((await frameAt(page, [0, 0, 7], [6, 0, 7]))?.id).toBe("M6");
  expect((await model(page)).nodes).toHaveLength(8); // the columns keep the joints at 6 m
  await page.keyboard.press("Control+z");

  // The top right joint, by two picked joints: from (0, 0, 3) to (0, 0, 6), so 3 m up.
  await page.keyboard.press("Escape");
  await clickAt(page, [6, 0, 6]);
  await menu(page, "Edit", "Move");
  await dlg.getByRole("button", { name: "Pick in View" }).click();
  await expect(dlg).toBeHidden();
  await clickAt(page, [0, 0, 3]);
  await clickAt(page, [0, 0, 6]);
  await expect(dlg).toBeVisible();
  await expect(dlg.getByLabel("dZ")).toHaveValue("3");
  await dlg.getByRole("button", { name: "OK" }).click();
  expect(await jointAt(page, 6, 9)).toBeDefined();
  expect((await frameAt(page, [6, 0, 3], [6, 0, 9]))?.id).toBe("M4");
  expect((await frameAt(page, [0, 0, 6], [6, 0, 9]))?.id).toBe("M6");
});

test("Edit > Replicate: radial copies about a picked joint, then a mirror image that deletes the original", async ({ page }) => {
  await openFrame(page);
  await clickAt(page, [6, 0, 4.5]); // the Story2 column at x = 6
  await page.keyboard.press("Control+r");
  const dlg = page.getByRole("dialog", { name: "Replicate" });
  await dlg.getByRole("tab", { name: "Radial" }).click();
  await dlg.getByRole("button", { name: "Pick in View" }).click();
  await clickAt(page, [0, 0, 0]); // the axis: vertical through the base joint at x = 0
  await expect(dlg.getByLabel("X", { exact: true })).toHaveValue("0");
  await dlg.getByLabel(/^Angle/).fill("90");
  await dlg.getByLabel("Number of copies").fill("3");
  await dlg.getByRole("button", { name: "OK" }).click();
  await expect(page.getByRole("status")).toContainText("objects copied");
  for (const [x, y] of [
    [0, 6],
    [-6, 0],
    [0, -6],
  ])
    expect(await frameAt(page, [x, y, 3], [x, y, 6])).toBeDefined();
  // The copies are selected.
  expect((await page.evaluate(() => window.__osab!.state().selection.frames)).length).toBe(3);

  // Mirror the top beam about the plane x = 10 and delete the original: its joints stay with the columns.
  await page.keyboard.press("Escape");
  await clickAt(page, [3, 0, 6]);
  await page.keyboard.press("Control+r");
  await dlg.getByRole("tab", { name: "Mirror" }).click();
  await dlg.getByLabel("X1").fill("10");
  await dlg.getByLabel("X2").fill("10");
  await dlg.getByLabel("Y2").fill("1");
  await dlg.getByLabel("Delete original objects").check();
  await dlg.getByRole("button", { name: "OK" }).click();
  expect(await frameAt(page, [14, 0, 6], [20, 0, 6])).toBeDefined();
  expect(await frameAt(page, [0, 0, 6], [6, 0, 6])).toBeUndefined();
  expect(await jointAt(page, 0, 6)).toBeDefined();
});

test("Edit > Replicate, Story: a copy onto an object already there is left out", async ({ page }) => {
  await openFrame(page);
  await clickAt(page, [3, 0, 3]); // the Story1 beam; Story2 has its own
  await clickAt(page, [0, 0, 1.5]); // and the Story1 column at x = 0
  await page.keyboard.press("Control+r");
  const dlg = page.getByRole("dialog", { name: "Replicate" });
  await dlg.getByRole("tab", { name: "Story" }).click();
  await dlg.getByLabel("Story2").check();
  await dlg.getByRole("button", { name: "OK" }).click();
  await expect(page.getByRole("status")).toContainText("0 objects copied. 2 left out");
  expect((await model(page)).frames).toHaveLength(6);
});
