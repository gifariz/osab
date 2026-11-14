// Options > Preferences switches the UI to Indonesian and back; the choice is kept on this computer.

import { expect, test } from "@playwright/test";

import { menu, openApp } from "./helpers";

test("switch the UI language to Indonesian and back", async ({ page }) => {
  await openApp(page);
  await menu(page, "Options", "Preferences");
  await page.getByRole("dialog").getByLabel("Language").selectOption("id");
  await page.getByRole("dialog").getByRole("button", { name: "OK" }).click();

  const bar = page.locator(".menubar");
  await expect(bar.getByRole("button", { name: "Berkas", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "▶ Jalankan" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Properti" })).toBeVisible();

  // Kept after a reload (browser storage of the app window).
  await page.reload();
  await expect(bar.getByRole("button", { name: "Berkas", exact: true })).toBeVisible();

  await menu(page, "Opsi", "Preferensi");
  await page.getByRole("dialog").getByLabel("Bahasa").selectOption("en");
  await page.getByRole("dialog").getByRole("button", { name: "OK" }).click();
  await expect(bar.getByRole("button", { name: "File", exact: true })).toBeVisible();
});
