// Measure (View toolbar): two clicks give the distance between two points, with its X, Y and Z parts, in the 3D
// view and in an elevation; joints snap, and the model is not changed.
import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { clickAt, openApp } from "./helpers";

const repo = new URL("../../", import.meta.url);
const portal = JSON.parse(readFileSync(new URL("examples/portal_frame.osab", repo), "utf8"));

test("Measure gives the distance between two clicked joints, in 3D and in an elevation", async ({ page }) => {
  await openApp(page);
  await page.evaluate((m) => window.__osab!.open(m, "portal.osab"), portal);
  const before = await page.evaluate(() => JSON.stringify(window.__osab!.state().model));

  await page.getByRole("button", { name: /Measure/ }).click();
  await clickAt(page, [0, 0, 0]);
  await expect(page.getByRole("status")).toContainText("Click the second point.");
  await clickAt(page, [6, 0, 4]);
  // sqrt(6^2 + 4^2) = 7.2111 m.
  await expect(page.getByRole("status")).toContainText(/Distance 7\.21\d* m/);
  await expect(page.getByRole("status")).toContainText(/ΔX 6(\.0+)?,/);
  await expect(page.getByRole("status")).toContainText(/ΔZ 4(\.0+)?\)/);

  // An elevation: the column's height.
  await page.getByRole("button", { name: "Elev X-Z", exact: true }).click();
  await clickAt(page, [0, 0, 0]);
  await clickAt(page, [0, 0, 4]);
  await expect(page.getByRole("status")).toContainText(/Distance 4(\.0+)? m/);

  // Measuring never edits the model; Esc leaves the tool.
  expect(await page.evaluate(() => JSON.stringify(window.__osab!.state().model))).toBe(before);
  await page.keyboard.press("Escape");
  expect(await page.evaluate(() => window.__osab!.state().tool)).toBe("select");
});
