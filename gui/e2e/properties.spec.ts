// The Properties of one selected object: Geometry, Assignments and Loads; values edited in place (drop-downs,
// numbers), multi-value rows expand and their "…" opens the window for them. The left panel: the explorer
// above the properties (the line between them dragged), or two tabs; the side panel's explanations behind ?.

import { expect, test } from "@playwright/test";

import { bench, clickAt, openApp } from "./helpers";

test("one selected member: groups, rows that expand, values edited in place, … opens the window", async ({ page }) => {
  await openApp(page);
  await page.waitForFunction(() => window.__osab !== undefined);
  await page.evaluate((m) => window.__osab!.open(m, "b05.json"), bench("b05_portal_frame", "model.json"));
  await page.getByRole("button", { name: "Elev X-Z" }).click();
  await clickAt(page, [3, 0, 4]); // the beam
  const props = page.getByLabel("Assignments and loads");
  for (const g of ["Geometry", "Assignments", "Loads"])
    await expect(props.getByRole("button", { name: g, exact: true })).toHaveAttribute("aria-expanded", "true");
  await expect(props).toContainText("DEAD");
  await expect(props).toContainText("15 kN/m");
  // The buttons under the properties are gone.
  await expect(page.getByRole("button", { name: "Frame loads…" })).toHaveCount(0);

  // Short labels, the full name on hover; collapsed: the summary only, expanded: each value.
  const modifiers = props.getByRole("button", { name: /Modifiers/ });
  await expect(props.locator(".prop-k", { hasText: /^.?Modifiers$/ })).toHaveAttribute("title", "Property modifiers");
  await expect(modifiers).toContainText("Default");
  await expect(modifiers).toHaveAttribute("aria-expanded", "false");
  await modifiers.click();
  await expect(props).toContainText("I33");

  // A group closed stays closed.
  await props.getByRole("button", { name: "Geometry", exact: true }).click();
  await expect(props).not.toContainText("Length");
  await props.getByRole("button", { name: "Geometry", exact: true }).click();
  await expect(props).toContainText("Length");

  // In place: the section from its drop-down, the axis angle typed (Enter), each one undo step.
  await props.getByLabel("Section", { exact: true }).selectOption("COL");
  await props.getByLabel("Local axis angle", { exact: true }).fill("30");
  await props.getByLabel("Local axis angle", { exact: true }).press("Enter");
  let beam = await page.evaluate(() => window.__osab!.state().model.frames.find((f) => f.id === "2")!);
  expect([beam.section, beam.angle]).toEqual(["COL", 30]);
  await page.keyboard.press("Control+z");
  await page.keyboard.press("Control+z");
  beam = await page.evaluate(() => window.__osab!.state().model.frames.find((f) => f.id === "2")!);
  expect([beam.section, beam.angle]).toEqual(["BEAM", undefined]);
  // Undo clears the selection: the beam again.
  await clickAt(page, [3, 0, 4]);
  await expect(props.getByLabel("Section", { exact: true })).toHaveValue("BEAM");

  // … opens the window that edits the value (for this object).
  await props.getByRole("button", { name: "Edit End length offsets…" }).click();
  await expect(page.getByRole("dialog", { name: /End Length Offsets/i })).toBeVisible();
  await page.keyboard.press("Escape");
  await props.getByRole("button", { name: "Edit Loads…" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");

  // A joint: its restraint from the drop-down.
  await page.keyboard.press("Escape"); // clears the selection
  await clickAt(page, [0, 0, 0]);
  await props.getByLabel("Restraints", { exact: true }).selectOption("pinned");
  expect(await page.evaluate(() => window.__osab!.state().model.supports.find((s) => s.node === "1")!.fix)).toBe("pinned");
});

test("left panel: explorer above properties by default, the split dragged, tabs on request; ? shows the side panel's explanations", async ({ page }) => {
  await openApp(page);
  const explorer = page.getByRole("region", { name: "Model" });
  const properties = page.getByRole("region", { name: "Properties" });
  await expect(explorer).toBeVisible();
  await expect(properties).toBeVisible();
  await expect(properties).toContainText("Select an object to see and edit its properties.");
  await expect(page.getByRole("tab")).toHaveCount(0);

  // Drag the line between them down: the explorer gets taller; kept after a reload.
  const split = page.getByRole("separator", {
    name: "Split between the model explorer and the properties",
  });
  const before = (await explorer.boundingBox())!.height;
  const box = (await split.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + 102, { steps: 5 });
  await page.mouse.up();
  const after = (await explorer.boundingBox())!.height;
  expect(after).toBeGreaterThan(before + 60);
  await page.reload();
  expect(Math.abs((await page.getByRole("region", { name: "Model" }).boundingBox())!.height - after)).toBeLessThan(3);

  // The drawing tips: only with ? on.
  await page.getByRole("button", { name: "╱ Frame" }).click();
  const tip = page.getByText("End i is the lower end");
  await expect(tip).toBeHidden();
  await properties.getByRole("button", { name: "Help" }).click();
  await expect(tip).toBeVisible();
  await properties.getByRole("button", { name: "Help" }).click();
  await page.keyboard.press("Escape");

  // As tabs, and back.
  await page.getByRole("button", { name: "Show as tabs" }).click();
  await expect(page.getByRole("tab", { name: "Model" })).toBeVisible();
  await page.getByRole("tab", { name: "Properties" }).click();
  await expect(page.getByLabel("Assignments and loads")).toHaveCount(0);
  await page.getByRole("button", { name: "Show the explorer above the properties" }).click();
  await expect(page.getByRole("tab")).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Model" })).toBeVisible();
});
