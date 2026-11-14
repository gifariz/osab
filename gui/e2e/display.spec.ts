// View > Set Display Options (Ctrl+W): texts on the objects, the object types drawn and their colours;
// OK, Apply and Cancel; the options come back with the model file; the Model Explorer's menus on
// sections select, show and hide the objects using them, which does not bring back a type that is unticked.

import { expect, test } from "@playwright/test";

import { bench, expandTree, menu, openApp } from "./helpers";

test("Ctrl+W sets what the view shows; the options open with the model", async ({ page }) => {
  await openApp(page);
  await page.waitForFunction(() => window.__osab !== undefined);
  await page.evaluate((m) => window.__osab!.open(m, "b05.json"), bench("b05_portal_frame", "model.json"));
  const options = () => page.evaluate(() => window.__osab!.state().display.options);
  const texts = async () => (await page.locator(".lbl-elem").allTextContents()).sort();
  await expect.poll(texts).toEqual([]);

  // Object Assignments: frame labels and sections, written on each member in one label; Apply keeps it open.
  await page.locator("[data-testid=viewport]").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("Control+w");
  const dialog = page.getByRole("dialog", { name: "Set Display Options" });
  await dialog.getByRole("tab", { name: "Object Assignments" }).click();
  const frames = dialog.getByRole("group", { name: "Frames" });
  await frames.getByLabel("Labels", { exact: true }).check();
  await frames.getByLabel("Sections").check();
  await dialog.getByRole("button", { name: "Apply" }).click();
  await expect(dialog).toBeVisible();
  await expect.poll(texts).toEqual(["C1\nCOL", "C2\nCOL", "B1\nBEAM"].sort());

  // General: columns not drawn, colours by object type; Cancel drops what was not applied.
  await dialog.getByRole("tab", { name: "General" }).click();
  await dialog.getByLabel("Columns").uncheck();
  await dialog.getByRole("button", { name: "Cancel" }).click();
  expect((await options()).hiddenTypes).toEqual([]);
  await menu(page, "View", "Set Display Options");
  await dialog.getByLabel("Columns").uncheck();
  await dialog.getByLabel("Colors by").selectOption("types");
  await dialog.getByRole("button", { name: "OK" }).click();
  await expect(dialog).toBeHidden();
  const o = await options();
  expect(o.hiddenTypes).toEqual(["frame:column"]);
  expect(o.colorBy).toBe("types");
  await expect.poll(texts).toEqual(["B1\nBEAM"]);

  // Model Explorer: Hide / Show Objects Using It on a section. Showing COL's members does not bring the
  // columns back while Columns is unticked.
  const tree = page.getByRole("tree", { name: "Model explorer" });
  const row = (name: string) => tree.getByRole("treeitem", { name, exact: true });
  await expandTree(page);
  await row("BEAM").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Hide Objects Using It", exact: true }).click();
  expect((await page.evaluate(() => window.__osab!.state().hidden.frames))).toEqual(["2"]);
  await expect.poll(texts).toEqual([]);
  await row("BEAM").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Show Objects Using It", exact: true }).click();
  await row("COL").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Show Objects Using It", exact: true }).click();
  await expect.poll(texts).toEqual(["B1\nBEAM"]);
  // Objects of a type not drawn are not selected either.
  const selected = () => page.evaluate(() => window.__osab!.state().selection.frames);
  await row("COL").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Select Objects Using It", exact: true }).click();
  expect(await selected()).toEqual([]);
  await row("BEAM").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Select Objects Using It", exact: true }).click();
  expect(await selected()).toEqual(["2"]);
  await row("BEAM").click({ button: "right" });
  await page.getByRole("menuitem", { name: "Deselect Objects Using It", exact: true }).click();
  expect(await selected()).toEqual([]);

  // View > Show Labels turns the labels of every kind on and off; the dialog's other items stay.
  await menu(page, "View", "Show Labels");
  expect((await options()).joint.labels).toBe(true);
  await menu(page, "View", "Show Labels");
  const after = await options();
  expect([after.joint.labels, after.frame.labels, after.frame.sections]).toEqual([false, false, true]);

  // Saved in the file's gui.viewOptions: a model with them opens with them.
  const saved = { ...bench("b05_portal_frame", "model.json"), gui: { viewOptions: { frame: { ids: true }, grid: true } } };
  await page.evaluate((m) => window.__osab!.open(m, "b05.json"), saved);
  const opened = await options();
  expect([opened.frame.ids, opened.frame.sections, opened.grid, opened.hiddenTypes]).toEqual([true, false, true, []]);
  await expect.poll(texts).toEqual(["1", "2", "3"]);
  await expect(page.locator(".lbl-grid").first()).toBeVisible();
});
