// Display > Show Loads / Hide Loads and the Model explorer's Show Loads / Hide Loads: no loads are drawn
// until a pattern is chosen. F4 (Undeformed Shape) goes back to the default display.

import { expect, test } from "@playwright/test";

import { bench, expandTree, menu, openApp } from "./helpers";

test("loads are drawn only for the pattern chosen in Display > Show Loads or the explorer", async ({ page }) => {
  await openApp(page);
  await page.waitForFunction(() => window.__osab !== undefined);
  await page.evaluate((m) => window.__osab!.open(m, "b05.json"), bench("b05_portal_frame", "model.json"));
  const shown = () => page.evaluate(() => window.__osab!.state().display.loadsPattern);
  const loadLabels = page.locator(".lbl-load");

  // Nothing drawn after opening a file.
  expect(await shown()).toBeNull();
  await expect(loadLabels).toHaveCount(0);

  // Display > Show Loads: pick DEAD.
  await menu(page, "Display", "Show Loads");
  await page.getByRole("dialog").getByLabel("Load pattern").selectOption("DEAD");
  await page.getByRole("dialog").getByRole("button", { name: "OK" }).click();
  expect(await shown()).toBe("DEAD");
  await expect(loadLabels.first()).toBeVisible();

  // Display > Hide Loads.
  await menu(page, "Display", "Hide Loads");
  expect(await shown()).toBeNull();
  await expect(loadLabels).toHaveCount(0);

  // The explorer: right-click a pattern > Show Loads, then the same row offers Hide Loads.
  await expandTree(page);
  const row = page.getByRole("tree", { name: "Model explorer" }).getByRole("treeitem", { name: "WIND", exact: true }).first(); // the pattern (its case follows)
  await row.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Show Loads", exact: true }).click();
  expect(await shown()).toBe("WIND");
  await row.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Hide Loads", exact: true }).click();
  expect(await shown()).toBeNull();

  // Show Loads > None hides them too.
  await row.dblclick();
  expect(await shown()).toBe("WIND");
  await menu(page, "Display", "Show Loads");
  await page.getByRole("dialog").getByLabel("Load pattern").selectOption("");
  await page.getByRole("dialog").getByRole("button", { name: "OK" }).click();
  expect(await shown()).toBeNull();
});

test("F4 goes back to the default display: no loads, labels, extrusion, local axes or mesh", async ({ page }) => {
  await openApp(page);
  await page.waitForFunction(() => window.__osab !== undefined);
  await page.evaluate((m) => window.__osab!.open(m, "b05.json"), bench("b05_portal_frame", "model.json"));
  const display = () => page.evaluate(() => {
    const d = window.__osab!.state().display;
    const o = d.options;
    return { mode: d.mode, loads: d.loadsPattern, labels: o.frame.labels && o.joint.labels, extrude: o.extrudeFrames && o.extrudeAreas, localAxes: o.frame.localAxes && o.area.localAxes, mesh: o.mesh };
  });
  const defaults = { mode: "model", loads: null, labels: false, extrude: false, localAxes: false, mesh: false };
  expect(await display()).toEqual(defaults);

  await expandTree(page);
  await page.getByRole("tree", { name: "Model explorer" }).getByRole("treeitem", { name: "DEAD", exact: true }).first().dblclick();
  for (const item of ["Show Labels", "Extrude Frames and Areas", "Show Local Axes", "Show Analysis Mesh"]) await menu(page, "View", item);
  await page.keyboard.press("F5");
  await expect(page.getByRole("status")).toContainText("Analysis complete");
  await page.keyboard.press("F6");
  expect(await display()).toEqual({ mode: "deformed", loads: "DEAD", labels: true, extrude: true, localAxes: true, mesh: true });

  // F4, then the toolbar's Model button does the same; the results stay.
  await page.keyboard.press("F4");
  expect(await display()).toEqual(defaults);
  expect(await page.evaluate(() => window.__osab!.state().results !== null)).toBe(true);
  await menu(page, "View", "Show Labels");
  await page.locator(".toolbar").getByRole("button", { name: "Model", exact: true }).click();
  expect(await display()).toEqual(defaults);
});
