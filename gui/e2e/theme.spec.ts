// Options > Preferences > Theme: light (the default), dark or the same as Windows; kept on this computer.

import { expect, test, type Page } from "@playwright/test";

import { bench, menu, openApp } from "./helpers";

const shots = process.env.OSAB_SHOTS;

async function chooseTheme(page: Page, value: "system" | "light" | "dark") {
  await menu(page, "Options", "Preferences");
  await page.getByRole("dialog").getByLabel("Theme").selectOption(value);
  await page.getByRole("dialog").getByRole("button", { name: "OK" }).click();
}

const viewportColour = (page: Page) => page.evaluate(() => getComputedStyle(document.querySelector("[data-testid=viewport]")!).backgroundColor);

test("switch between light and dark themes", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await openApp(page);
  const html = page.locator("html");
  // Light by default, whatever the system says.
  await expect(html).toHaveAttribute("data-theme", "light");

  // "Same as Windows" follows the system setting, also when it changes.
  await chooseTheme(page, "system");
  await expect(html).toHaveAttribute("data-theme", "dark");
  await page.emulateMedia({ colorScheme: "light" });
  await expect(html).toHaveAttribute("data-theme", "light");

  await chooseTheme(page, "dark");
  await expect(html).toHaveAttribute("data-theme", "dark");
  expect(await viewportColour(page)).toBe("rgb(13, 17, 23)");
  // Kept after a reload, whatever the system says.
  await page.reload();
  await expect(html).toHaveAttribute("data-theme", "dark");

  await page.evaluate((m) => window.__osab!.open(m, "b24.json"), bench("b24_slab_on_columns", "model.json"));
  await page.keyboard.press("F5");
  await expect(page.getByText("Analysis complete. Equilibrium check passed.")).toBeVisible();
  if (shots) {
    await page.screenshot({ path: `${shots}/theme-dark.png` });
    await page.getByRole("tab", { name: "Model" }).click();
    await menu(page, "Define", "Area Sections");
    await page.screenshot({ path: `${shots}/theme-dark-dialog.png` });
    await page.getByRole("dialog").getByRole("button", { name: "Close", exact: true }).click();
  }

  await chooseTheme(page, "light");
  await expect(html).toHaveAttribute("data-theme", "light");
  expect(await viewportColour(page)).toBe("rgb(255, 255, 255)");
  if (shots) {
    await page.screenshot({ path: `${shots}/theme-light.png` });
    await menu(page, "Display", "Shell Forces");
    await page.screenshot({ path: `${shots}/theme-light-shells.png` });
    await menu(page, "Display", "Member Forces");
    await page.screenshot({ path: `${shots}/theme-light-forces.png` });
  }
});
