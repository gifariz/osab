// Builds the solver as WebAssembly (the `web/` crate) and copies it to src/ipc/osab.wasm, where the
// browser's solver worker and the Vitest test load it. Always a release build: a debug build is many
// times slower. Needs the target once: rustup target add wasm32-unknown-unknown

import { spawnSync } from "node:child_process";
import { copyFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const repo = fileURLToPath(new URL("../../", import.meta.url));
const args = ["build", "-q", "--release", "--target", "wasm32-unknown-unknown", "-p", "osab-web"];
const run = spawnSync("cargo", args, { cwd: repo, stdio: "inherit" });
if (run.status !== 0) {
  console.error("build-wasm: cargo failed. If the target is missing: rustup target add wasm32-unknown-unknown");
  process.exit(run.status ?? 1);
}
copyFileSync(`${repo}target/wasm32-unknown-unknown/release/osab_web.wasm`, fileURLToPath(new URL("../src/ipc/osab.wasm", import.meta.url)));
