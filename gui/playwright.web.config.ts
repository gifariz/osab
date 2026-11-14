import { defineConfig } from "@playwright/test";

// The web demo as it is deployed: the static build (`npm run build:web`, base /osab/) served by
// `vite preview`, or the live site when OSAB_WEB_URL is set, e.g.
//   OSAB_WEB_URL=https://www.gifariz.com/osab/ npm run e2e:web
const live = process.env.OSAB_WEB_URL;
const url = live ?? "http://localhost:4173/osab/";

export default defineConfig({
  testDir: "e2e",
  testMatch: ["webbuild.spec.ts", "demo.spec.ts", "browser.spec.ts", "portal.spec.ts", "units.spec.ts", "patterns.spec.ts"],
  timeout: 180_000,
  reporter: [["list"]],
  use: {
    baseURL: url,
    viewport: { width: 1280, height: 800 },
    screenshot: "only-on-failure",
    // As in playwright.config.ts: start from an empty model; demo.spec.ts starts without this.
    storageState: { cookies: [], origins: [{ origin: new URL(url).origin, localStorage: [{ name: "osab.startEmpty", value: "1" }] }] },
  },
  webServer: live
    ? undefined
    : {
        command: "npm run build:web && npm run preview:web",
        url,
        reuseExistingServer: false,
        timeout: 300_000,
      },
});
