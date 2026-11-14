// The static web build (playwright.web.config.ts only): every file comes from the site path (/osab/)
// (apart from the host's analytics script),
// the solver module is served as WebAssembly, and the page runs a model.

import { expect, test } from "@playwright/test";

import { bench, openApp } from "./helpers";

test("the built site loads everything from its base path and solves", async ({ page, baseURL }) => {
  const base = new URL(baseURL!);
  const outside: string[] = [];
  page.on("request", (r) => {
    const u = new URL(r.url());
    // The site host's analytics script (published site only) is the one file from the site root.
    if (u.origin === base.origin && !u.pathname.startsWith(base.pathname) && !u.pathname.startsWith("/_vercel/insights/")) outside.push(u.pathname);
  });
  const wasm = page.waitForResponse((r) => r.url().endsWith(".wasm"));
  await openApp(page);
  await page.evaluate((m) => window.__osab!.open(m, "b05.json"), bench("b05_portal_frame", "model.json"));
  await page.keyboard.press("F5");
  await expect(page.getByRole("status")).toContainText("Analysis complete. Equilibrium check passed.");

  const res = await wasm;
  expect(new URL(res.url()).pathname.startsWith(`${base.pathname}assets/`)).toBe(true);
  expect(res.headers()["content-type"]).toContain("application/wasm");
  expect(outside).toEqual([]);
});
