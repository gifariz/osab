// The browser's solver (WebAssembly in a worker): a long run can be stopped and the next run starts a
// new worker; a model above the browser's size limit stops with a message before the solve.

import { expect, test, type Page } from "@playwright/test";

import { openApp } from "./helpers";

/** A 20 x 20 m slab on four corner supports, meshed n x n: (n + 1)^2 analysis joints. */
function slab(n: number) {
  const corners = [
    [0, 0],
    [20, 0],
    [20, 20],
    [0, 20],
  ];
  return {
    materials: [{ id: "C", E: 25e6, nu: 0.2 }],
    area_sections: [{ id: "S", type: "shell-thin", material: "C", thickness: 0.2 }],
    nodes: corners.map(([x, y], i) => ({ id: String(i + 1), x, y, z: 0 })),
    areas: [{ id: "F1", nodes: ["1", "2", "3", "4"], section: "S", mesh: { size: 20 / n } }],
    supports: ["1", "2", "3", "4"].map((node) => ({ node, fix: "fixed" })),
    load_cases: [{ id: "Q", area_loads: [{ area: "F1", dir: "Z", q: -10 }] }],
  };
}

async function open(page: Page, model: unknown) {
  await page.evaluate((m) => window.__osab!.open(m, "slab.json"), model);
}

test("a long run can be stopped, and the next run works", async ({ page }) => {
  await openApp(page);
  await open(page, slab(85));
  await page.keyboard.press("F5");
  // The Stop button shows once a run takes longer than half a second.
  const stop = page.getByRole("button", { name: "Running… · Stop" });
  await stop.click();
  await expect(page.getByRole("status")).toContainText("Analysis stopped.");
  await expect(stop).toBeHidden();
  expect(await page.evaluate(() => window.__osab!.state().results)).toBeNull();

  await page.keyboard.press("F5");
  await expect(page.getByRole("status")).toContainText("Analysis complete. Equilibrium check passed.", { timeout: 120_000 });
  expect(await page.evaluate(() => window.__osab!.state().results!.mesh!.joints.length)).toBe(86 * 86 - 4);
});

test("a model above the size limit stops before the solve", async ({ page }) => {
  await openApp(page);
  await open(page, slab(150));
  await page.keyboard.press("F5");
  await expect(page.getByRole("status")).toContainText(
    /This model has 22801 analysis joints; the browser version solves up to \d+\. Use the desktop app for bigger models\./,
  );
  expect(await page.evaluate(() => window.__osab!.state().running)).toBe(false);
});
