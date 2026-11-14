// The 3D view's rotation (right-drag): it turns about the point under the mouse where the drag starts, Z
// stays up, and it stops short of a straight top view instead of flipping over.

import { expect, test } from "@playwright/test";

import { bench, menu, screen, type Vec3, openApp } from "./helpers";

test("3D rotation turns about the point under the mouse, Z up, no flip", async ({ page }) => {
  await openApp(page);
  await page.evaluate((m) => window.__osab!.open(m, "b32.json"), bench("b32_floor_on_beams", "model.json"));
  const at = (p: Vec3) => screen(page, p);
  const rightDrag = async (from: Vec3, dx: number, dy: number) => {
    const s = await at(from);
    await page.mouse.move(s.x, s.y);
    await page.mouse.down({ button: "right" });
    await page.mouse.move(s.x + dx, s.y + dy, { steps: 12 });
    await page.mouse.up({ button: "right" });
  };

  // Start on a corner column's base: it stays put while the rest turns.
  const corner: Vec3 = [10, 0, 0];
  const before = await at(corner);
  const other0 = await at([0, 8, 3.5]);
  await rightDrag(corner, -180, 70);
  const after = await at(corner);
  const other1 = await at([0, 8, 3.5]);
  expect(Math.hypot(after.x - before.x, after.y - before.y)).toBeLessThan(1);
  expect(Math.hypot(other1.x - other0.x, other1.y - other0.y)).toBeGreaterThan(50);

  // Z stays up: the middle column (through the screen's middle after Home) is drawn vertical.
  await page.keyboard.press("Home");
  const [base, top] = [await at([5, 4, 0]), await at([5, 4, 3.5])];
  expect(Math.abs(top.x - base.x)).toBeLessThan(1);
  expect(top.y).toBeLessThan(base.y);

  // A long drag down stops 5 degrees short of a top view: columns keep some length on screen.
  await rightDrag([5, 4, 3.5], 0, 2000);
  const [b2, t2] = [await at([5, 4, 0]), await at([5, 4, 3.5])];
  expect(Math.hypot(t2.x - b2.x, t2.y - b2.y)).toBeGreaterThan(5);
});

test("the left panel is resized by dragging its edge, kept after a reload; double-click resets it", async ({ page }) => {
  await openApp(page);
  const panel = page.locator(".side-panel");
  const edge = page.getByRole("separator", { name: "Resize the left panel" });
  const width = async () => (await panel.boundingBox())!.width;
  expect(await width()).toBe(250);
  const box = (await edge.boundingBox())!;
  await page.mouse.move(box.x + 2, box.y + 100);
  await page.mouse.down();
  await page.mouse.move(box.x + 122, box.y + 100, { steps: 5 });
  await page.mouse.up();
  expect(await width()).toBe(370);
  // The 3D view takes the rest of the window.
  const view = (await page.getByTestId("viewport").boundingBox())!;
  expect(view.x).toBeGreaterThanOrEqual(370);
  await page.reload();
  expect(await width()).toBe(370);
  // Limits, arrow keys, double-click back to the default.
  await edge.focus();
  await page.keyboard.press("ArrowLeft");
  expect(await width()).toBe(350);
  const now = (await edge.boundingBox())!;
  await page.mouse.move(now.x + 2, now.y + 100);
  await page.mouse.down();
  await page.mouse.move(now.x - 400, now.y + 100, { steps: 5 });
  await page.mouse.up();
  expect(await width()).toBe(180);
  await edge.dblclick();
  expect(await width()).toBe(250);
});

test("Ctrl+D and View > Show Grid hide and show the grid", async ({ page }) => {
  await openApp(page);
  await page.waitForFunction(() => window.__osab !== undefined);
  const grid = () => page.evaluate(() => window.__osab!.state().display.options.grid);
  expect(await grid()).toBe(false); // off for a new view
  await page.locator("[data-testid=viewport]").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("Control+d");
  expect(await grid()).toBe(true);
  await menu(page, "View", "Show Grid");
  expect(await grid()).toBe(false);
});

test("the navigation cube shows in the 3D view only; clicking it turns the view and Home goes back", async ({ page }) => {
  await openApp(page);
  const cube = page.getByTestId("nav-cube");
  await expect(cube).toBeVisible();
  const box = (await cube.locator("canvas").boundingBox())!;
  // A click on the cube (its middle is the face nearest the viewer) turns the view; the view stays 3D.
  await cube.locator("canvas").click({ position: { x: box.width / 2, y: box.height * 0.3 } });
  expect(await page.evaluate(() => window.__osab!.state().view.kind)).toBe("3d");
  await cube.getByRole("button", { name: /Home view/ }).click();
  await page.getByRole("button", { name: "Plan", exact: true }).click();
  await expect(cube).toBeHidden();
  await page.getByRole("button", { name: "3D", exact: true }).click();
  await expect(cube).toBeVisible();
});

test("the Iso button switches the 3D view between perspective and parallel projection, kept in the view options", async ({ page }) => {
  await openApp(page);
  const iso = page.getByRole("button", { name: "Iso", exact: true });
  const parallel = () => page.evaluate(() => window.__osab!.state().display.options.parallel);
  expect(await parallel()).toBe(false);
  await iso.click();
  expect(await parallel()).toBe(true);
  // The cube still turns the view, and plan and elevation views work as before.
  await expect(page.getByTestId("nav-cube")).toBeVisible();
  await page.getByRole("button", { name: "Plan", exact: true }).click();
  await page.getByRole("button", { name: "3D", exact: true }).click();
  expect(await parallel()).toBe(true);
  await iso.click();
  expect(await parallel()).toBe(false);
});
