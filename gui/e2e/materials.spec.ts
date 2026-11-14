// Material types by clicking: a new model's Steel shows Fy and Fu, Concrete f'c; a material made Other shows no
// strengths and keeps none; a new deck section takes the first steel material as its deck material.

import { expect, test } from "@playwright/test";

import { menu, openApp } from "./helpers";

test("material types: strengths follow the type; a new deck uses the steel material", async ({ page }) => {
  await openApp(page);
  await menu(page, "Define", "Materials");
  const dlg = page.getByRole("dialog", { name: "Materials" });
  await dlg.getByLabel("Materials", { exact: true }).selectOption("Steel");
  await expect(dlg.getByLabel("Material type")).toHaveValue("steel");
  await expect(dlg.getByLabel("Fy", { exact: true })).toHaveValue("345");
  await expect(dlg.getByLabel("Fu", { exact: true })).toHaveValue("450");
  await dlg.getByLabel("Materials", { exact: true }).selectOption("Concrete");
  await expect(dlg.getByLabel("f'c", { exact: true })).toHaveValue("28");
  await expect(dlg.getByLabel("Fy", { exact: true })).toHaveCount(0);

  await dlg.getByRole("button", { name: "Add New…" }).click();
  await dlg.getByLabel("Name", { exact: true }).fill("Timber");
  await dlg.getByLabel("Material type").selectOption("other");
  await expect(dlg.getByLabel("Fy", { exact: true })).toHaveCount(0);
  await expect(dlg.getByLabel("f'c", { exact: true })).toHaveCount(0);
  await dlg.getByLabel("Modulus of elasticity E", { exact: true }).fill("11000");
  await dlg.getByRole("button", { name: "Add", exact: true }).click();
  await dlg.getByRole("button", { name: "Close", exact: true }).click();
  const m = await page.evaluate(() => window.__osab!.state().model);
  expect(m.materials.find((x) => x.id === "Timber")).toMatchObject({ type: "other", E: 11e6 });
  expect(m.materials.find((x) => x.id === "Timber")?.fy).toBeUndefined();

  // A new deck section: the deck material is the steel one, the slab material the concrete one.
  await menu(page, "Define", "Area Sections");
  const as = page.getByRole("dialog", { name: "Area Sections" });
  await as.getByRole("button", { name: "Add New…" }).click();
  await as.getByLabel("Property type").selectOption("deck");
  await expect(as.getByLabel("Deck material")).toHaveValue("Steel");
  await expect(as.getByLabel("Slab material")).toHaveValue("Concrete");
  // The drop-downs start where the number fields start.
  const left = async (label: string) => (await as.getByLabel(label, { exact: true }).boundingBox())!.x;
  expect(Math.abs((await left("Deck material")) - (await left("Slab depth")))).toBeLessThan(1);
  expect(Math.abs((await left("Shell type")) - (await left("Slab depth")))).toBeLessThan(1);
});

test("windows hide their explanations until ? is pressed; notes always show", async ({ page }) => {
  await openApp(page);
  await menu(page, "Define", "Area Sections");
  const dlg = page.getByRole("dialog", { name: "Area Sections" });
  await dlg.getByLabel("Area Sections", { exact: true }).selectOption("Deck1");
  const hint = dlg.getByText("A deck is always a Membrane floor");
  await expect(hint).toBeHidden();
  await expect(dlg.getByText("Weight per area:")).toBeVisible();
  const help = dlg.getByRole("button", { name: "Help" });
  await help.click();
  await expect(hint).toBeVisible();
  await expect(help).toHaveAttribute("aria-pressed", "true");
  // One switch for every window, kept after a reload.
  await page.reload();
  await menu(page, "Define", "Area Sections");
  await page.getByRole("dialog", { name: "Area Sections" }).getByLabel("Area Sections", { exact: true }).selectOption("Deck1");
  await expect(page.getByRole("dialog").getByText("A deck is always a Membrane floor")).toBeVisible();
  await page.getByRole("dialog").getByRole("button", { name: "Help" }).click();
  await expect(page.getByRole("dialog").getByText("A deck is always a Membrane floor")).toBeHidden();
});
