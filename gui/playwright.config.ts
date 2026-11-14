import { defineConfig } from "@playwright/test";

// Runs the app UI in Chromium against `npm run dev:web` (the real solver, as WebAssembly in a worker).
export default defineConfig({
  testDir: "e2e",
  // The static build's own check runs in playwright.web.config.ts.
  testIgnore: "webbuild.spec.ts",
  timeout: 180_000,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:5173",
    viewport: { width: 1280, height: 800 },
    screenshot: "only-on-failure",
    // Start from an empty model (the web demo opens its example otherwise); demo.spec.ts starts without this.
    storageState: { cookies: [], origins: [{ origin: "http://localhost:5173", localStorage: [{ name: "osab.startEmpty", value: "1" }] }] },
  },
  webServer: {
    command: "npm run dev:web",
    url: "http://localhost:5173",
    reuseExistingServer: true,
    timeout: 300_000,
  },
});
