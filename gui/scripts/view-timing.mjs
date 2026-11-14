// Times the 3D view on a model in the real desktop app: open and draw, click and box selection, hover,
// rotate and zoom, and with results the deformed shape, shell contours and an animated mode. Starts
// osab-gui.exe with WebView2 remote debugging (or attaches to a running app, e.g. `npm run app` started with
// the same WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS) and drives it through Playwright over CDP, with the mouse.
//
//   node scripts/view-timing.mjs --model big.json [--results few-cases.json] [--out dir] [--label name]
//        [--exe path/to/osab-gui.exe | --attach 9333] [--profile] [--memory] [--only open,select,...]
//        [--save-as private/path/model.osab] measures saving the model and its results.
//        [--reopen private/path/model.osab] starts a fresh app and measures opening its saved results.
//
// Each action reports its wall time (from the action until the view is drawn and frames come at the normal
// rate again) and the page's busy time (main-thread task time). With --profile, each action's CPU profile is
// written to the output folder with a summary of the functions that took the time. With --memory, each action
// also reports the heap after a forced garbage collection (what stays alive) and, from a sampling heap
// profile, the functions that made what stays alive. Models stay where they are; only times, counts and
// screenshots go to the output folder.

import { spawn, execSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "@playwright/test";

const repo = fileURLToPath(new URL("../../", import.meta.url));
const args = process.argv.slice(2);
const arg = (name, fallback = null) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const flag = (name) => args.includes(`--${name}`);
const modelPath = arg("model", arg("reopen"));
const resultsPath = arg("results");
const label = arg("label", "run");
const out = arg("out", join(repo, "target", "view-timing", label));
const attach = arg("attach");
const exe = arg("exe", join(repo, "target", "release", "osab-gui.exe"));
const profile = flag("profile");
const memory = flag("memory");
const saveAs = arg("save-as");
const reopenPath = arg("reopen");
const only = arg("only")?.split(",") ?? null;
const port = Number(attach ?? 9333);
if (!modelPath) {
  console.error("usage: node scripts/view-timing.mjs --model big.json [--results few-cases.json] [--save-as model.osab] [--reopen model.osab] [--out dir] [--label name] [--exe path | --attach port] [--profile] [--memory]");
  process.exit(2);
}
mkdirSync(out, { recursive: true });
const want = (step) => !only || only.includes(step);

// ---- the app ----

let app = null;
const launchTime = Date.now();
if (!attach) {
  try {
    if (execSync('tasklist /FI "IMAGENAME eq osab-gui.exe"', { encoding: "utf8" }).includes("osab-gui.exe")) {
      console.error("osab-gui.exe is already running: close it first (a second window would join its WebView2 and miss the debugging port)");
      process.exit(2);
    }
  } catch {
    // tasklist missing: carry on
  }
  app = spawn(exe, reopenPath ? [reopenPath] : [], {
    env: {
      ...process.env,
      // Remote debugging, and no throttling when the window is behind another one.
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --disable-features=CalculateNativeWinOcclusion --disable-background-timer-throttling --disable-renderer-backgrounding --enable-precise-memory-info`,
    },
    stdio: "ignore",
  });
}

let browser;
for (let i = 0; i < 120 && !browser; i++) {
  try {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  } catch {
    await new Promise((r) => setTimeout(r, 500));
  }
}
if (!browser) throw new Error("could not attach to the app's WebView2");
const page = browser.contexts()[0].pages()[0];
await page.waitForFunction(() => window.__osab !== undefined, null, { timeout: 60000 });
const cdp = await page.context().newCDPSession(page);
await cdp.send("Performance.enable");
if (memory) await cdp.send("HeapProfiler.enable");
if (profile) {
  await cdp.send("Profiler.enable");
  await cdp.send("Profiler.setSamplingInterval", { interval: 250 });
}

// In-page helpers: frames, waiting until the view is drawn and quiet, and a frame recorder.
await page.evaluate(() => {
  const frame = () => new Promise((r) => requestAnimationFrame(() => r(performance.now())));
  window.__vt = {
    frame,
    /** Milliseconds from t0 until frames come at most `gap` ms apart again (6 in a row), after `until` holds. */
    async settle(t0, until, gap = 40) {
      if (until) {
        const check = new Function(`return (${until})`);
        while (!check()) await new Promise((r) => setTimeout(r, 20));
      }
      let last = await frame();
      let firstQuiet = last;
      let n = 0;
      while (n < 6) {
        const now = await frame();
        if (now - last <= gap) {
          if (n === 0) firstQuiet = last;
          n++;
        } else n = 0;
        last = now;
        if (now - t0 > 900000) break;
      }
      return firstQuiet - t0;
    },
    record() {
      const ts = [];
      let on = true;
      const loop = (t) => {
        ts.push(t);
        if (on) requestAnimationFrame(loop);
      };
      requestAnimationFrame(loop);
      return () => {
        on = false;
        return ts;
      };
    },
    heapMB() {
      return performance.memory ? Math.round(performance.memory.usedJSHeapSize / 1048576) : null;
    },
  };
});

const busy = async () => {
  const { metrics } = await cdp.send("Performance.getMetrics");
  const get = (n) => metrics.find((m) => m.name === n)?.value ?? 0;
  return { task: get("TaskDuration") * 1000, script: get("ScriptDuration") * 1000 };
};

const report = { label, model: basename(modelPath), results: resultsPath && basename(resultsPath), steps: [] };
const env = await page.evaluate(() => {
  const gl = document.createElement("canvas").getContext("webgl2");
  const dbg = gl?.getExtension("WEBGL_debug_renderer_info");
  return { width: innerWidth, height: innerHeight, dpr: devicePixelRatio, gpu: dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : null, ua: navigator.userAgent };
});
report.env = env;
console.log(`${label}: ${env.width}x${env.height} @${env.dpr}, ${env.gpu}`);

/** Profile names: one file per action. */
let profileIndex = 0;
async function startProfile() {
  if (profile) await cdp.send("Profiler.start");
}
async function stopProfile(name) {
  if (!profile) return null;
  const { profile: p } = await cdp.send("Profiler.stop");
  const file = join(out, `${String(++profileIndex).padStart(2, "0")}-${name.replace(/[^a-z0-9]+/gi, "_")}.cpuprofile`);
  writeFileSync(file, JSON.stringify(p));
  return summarize(p);
}

/** Top functions of a CPU profile: self time and time including callees (ms). */
function summarize(p) {
  const nodes = new Map(p.nodes.map((n) => [n.id, n]));
  const parent = new Map();
  for (const n of p.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  const name = (n) => {
    const f = n.callFrame;
    const file = f.url ? f.url.split("/").pop().split("?")[0] : "";
    return `${f.functionName || "(anonymous)"} ${file}:${f.lineNumber + 1}`;
  };
  const self = new Map();
  const total = new Map();
  for (let i = 0; i < p.samples.length; i++) {
    const dt = (p.timeDeltas[i + 1] ?? 0) / 1000;
    let id = p.samples[i];
    const n0 = nodes.get(id);
    const k0 = name(n0);
    self.set(k0, (self.get(k0) ?? 0) + dt);
    const seen = new Set();
    while (id !== undefined) {
      const k = name(nodes.get(id));
      if (!seen.has(k)) {
        seen.add(k);
        total.set(k, (total.get(k) ?? 0) + dt);
      }
      id = parent.get(id);
    }
  }
  const skip = (k) => /^\((root|program|idle|garbage collector)\)/.test(k);
  const top = (m, n) =>
    [...m]
      .filter(([k]) => !skip(k))
      .sort((a, b) => b[1] - a[1])
      .slice(0, n)
      .map(([k, v]) => [k, Math.round(v)]);
  return { self: top(self, 12), total: top(total, 25), gc: Math.round(self.get("(garbage collector) :0") ?? 0) };
}

async function shot(name) {
  await page.screenshot({ path: join(out, `${name}.png`) });
}

/** The heap after a full garbage collection (MB): what stays alive. */
async function liveMB() {
  await cdp.send("HeapProfiler.collectGarbage");
  await cdp.send("HeapProfiler.collectGarbage");
  return page.evaluate(() => window.__vt.heapMB());
}

/** Sampling heap profile of an action (--memory): started before it, read after a full collection. */
async function startSampling() {
  if (memory) await cdp.send("HeapProfiler.startSampling", { samplingInterval: 65536 });
}
async function stopSampling() {
  if (!memory) return null;
  const live = await liveMB();
  const { profile: p } = await cdp.send("HeapProfiler.getSamplingProfile");
  await cdp.send("HeapProfiler.stopSampling");
  // Live bytes by the function that made them (self), and including the functions it called.
  const self = new Map();
  const total = new Map();
  const walk = (n, stack) => {
    const f = n.callFrame;
    const file = f.url ? f.url.split("/").pop().split("?")[0] : "";
    const k = `${f.functionName || "(anonymous)"} ${file}:${f.lineNumber + 1}`;
    self.set(k, (self.get(k) ?? 0) + n.selfSize);
    const here = stack.includes(k) ? stack : [...stack, k];
    let sum = n.selfSize;
    for (const c of n.children ?? []) sum += walk(c, here);
    if (!stack.includes(k)) total.set(k, (total.get(k) ?? 0) + sum);
    return sum;
  };
  const all = walk(p.head, []);
  const top = (m, n) =>
    [...m]
      .filter(([k]) => !/^\(root\)/.test(k))
      .sort((a, b) => b[1] - a[1])
      .slice(0, n)
      .map(([k, v]) => [k, Math.round(v / 1048576)]);
  return { liveMB: live, sampledMB: Math.round(all / 1048576), self: top(self, 15), total: top(total, 25) };
}

/** One action: wall time until drawn and quiet, busy time, heap after (and with --memory what stays alive). */
async function timed(name, action, until = null) {
  const b0 = await busy();
  await startSampling();
  await startProfile();
  const t0 = await page.evaluate(() => performance.now());
  let wall;
  try {
    await action();
    wall = await page.evaluate(([t, u]) => window.__vt.settle(t, u), [t0, until]);
  } catch (e) {
    const step = { name, error: String(e).split("\n")[0] };
    report.steps.push(step);
    console.log(`  ${name}: FAILED ${step.error}`);
    throw e;
  }
  const b1 = await busy();
  const prof = await stopProfile(name);
  const heap = await page.evaluate(() => window.__vt.heapMB());
  const mem = await stopSampling();
  const step = { name, wallMs: Math.round(wall), busyMs: Math.round(b1.task - b0.task), scriptMs: Math.round(b1.script - b0.script), heapMB: heap, ...(mem ? { liveMB: mem.liveMB, memory: mem } : {}), ...(prof ? { profile: prof } : {}) };
  report.steps.push(step);
  console.log(`  ${name}: ${step.wallMs} ms (busy ${step.busyMs} ms, script ${step.scriptMs} ms, heap ${heap} MB${mem ? `, live ${mem.liveMB} MB` : ""})`);
  return step;
}

/** A run of small actions (mouse moves, wheel steps): each followed by a frame; per-event wall and busy time. */
async function timedRun(name, count, each, before = async () => {}, after = async () => {}) {
  await before();
  const b0 = await busy();
  await startProfile();
  const t0 = await page.evaluate(() => performance.now());
  for (let i = 0; i < count; i++) {
    await each(i);
    await page.evaluate(() => window.__vt.frame());
  }
  const t1 = await page.evaluate(() => window.__vt.frame());
  const b1 = await busy();
  const prof = await stopProfile(name);
  await after();
  const step = { name, events: count, perEventMs: Math.round(((t1 - t0) / count) * 10) / 10, busyPerEventMs: Math.round(((b1.task - b0.task) / count) * 10) / 10, ...(prof ? { profile: prof } : {}) };
  report.steps.push(step);
  console.log(`  ${name}: ${step.perEventMs} ms per event (busy ${step.busyPerEventMs} ms), ${count} events`);
  return step;
}

const box = await page.locator("[data-testid=viewport]").boundingBox();
const vp = { x: box.x, y: box.y, w: box.width, h: box.height, cx: box.x + box.width / 2, cy: box.y + box.height / 2 };
const state = (f, a) => page.evaluate(f, a);

/** Opens a model file's JSON through the app's own open path (normalize, load, draw). */
async function openModel(path, name) {
  const raw = JSON.parse(readFileSync(path, "utf8"));
  await page.evaluate(() => window.__osab.state().setDisplay({ mode: "model" }));
  return timed(name, () => page.evaluate(([m, n]) => window.__osab.open(m, n), [raw, basename(path)]));
}

/** Screen point (page pixels) of a model point. */
const screen = (p) => page.evaluate(([q]) => window.__osab.project(q), [p]).then((s) => (s ? { x: s.x + vp.x, y: s.y + vp.y } : null));

const selectionSummary = () =>
  state(() => {
    const s = window.__osab.state().selection;
    return { nodes: s.nodes.length, frames: s.frames.length, links: s.links.length, areas: s.areas.length, first: [...s.frames, ...s.areas, ...s.nodes].slice(0, 3) };
  });

async function modelSteps(tag) {
  const counts = await state(() => {
    const m = window.__osab.state().model;
    return { nodes: m.nodes.length, frames: m.frames.length, areas: m.areas.length, stories: (m.stories ?? []).length };
  });
  report[`${tag}Counts`] = counts;
  await shot(`${tag}-open`);

  if (want("select")) {
    // Click: the frame whose middle is nearest the middle of the view.
    const target = await state(([cx, cy]) => {
      const st = window.__osab.state();
      const m = st.model;
      const at = new Map(m.nodes.map((n) => [n.id, [n.x, n.y, n.z]]));
      let best = null;
      let bestD = Infinity;
      for (const f of m.frames) {
        const a = at.get(f.nodes[0]);
        const b = at.get(f.nodes[1]);
        if (!a || !b) continue;
        const s = window.__osab.project([(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2]);
        if (!s) continue;
        const d = Math.hypot(s.x - cx, s.y - cy);
        if (d < bestD) [best, bestD] = [{ id: f.id, x: s.x, y: s.y }, d];
      }
      return best;
    }, [vp.w / 2, vp.h / 2]);
    if (target) {
      const x = target.x + vp.x;
      const y = target.y + vp.y;
      await page.mouse.move(x, y);
      await timed(`${tag}: click-select one object`, () => page.mouse.click(x, y));
      report[`${tag}Clicked`] = await selectionSummary();
      await shot(`${tag}-clicked`);
      await timed(`${tag}: click it again (deselect)`, () => page.mouse.click(x, y));
    }

    // Box: a plan view of a middle story, a window over the whole view.
    const story = await state(() => {
      const m = window.__osab.state().model;
      const st = [...(m.stories ?? [])].sort((a, b) => a.z - b.z);
      const s = st[Math.floor(st.length / 2)];
      return s ? { id: s.id, z: s.z } : null;
    });
    if (story) {
      report[`${tag}Story`] = story.id;
      await timed(`${tag}: plan view of a story`, () => state(([z]) => window.__osab.state().setView({ kind: "plan", at: z }), [story.z]));
      await shot(`${tag}-plan`);
      await page.mouse.move(vp.x + 12, vp.y + 12);
      await timed(`${tag}: box-select the story (plan)`, async () => {
        await page.mouse.down();
        await page.mouse.move(vp.cx, vp.cy, { steps: 4 });
        await page.mouse.move(vp.x + vp.w - 12, vp.y + vp.h - 12, { steps: 4 });
        await page.mouse.up();
      });
      report[`${tag}BoxPlan`] = await selectionSummary();
      await shot(`${tag}-plan-boxed`);
      await timed(`${tag}: clear selection (Esc)`, () => page.keyboard.press("Escape"));
      await timed(`${tag}: back to the 3D view`, () => state(() => window.__osab.state().setView({ kind: "3d", at: 0 })));
      // 3D: a crossing box (right to left) over the story's joints.
      const band = await state(([z]) => {
        const m = window.__osab.state().model;
        let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
        for (const n of m.nodes) {
          if (Math.abs(n.z - z) > 1e-6) continue;
          const s = window.__osab.project([n.x, n.y, n.z]);
          if (!s) continue;
          [x0, y0, x1, y1] = [Math.min(x0, s.x), Math.min(y0, s.y), Math.max(x1, s.x), Math.max(y1, s.y)];
        }
        return Number.isFinite(x0) ? { x0, y0, x1, y1 } : null;
      }, [story.z]);
      if (band) {
        await page.mouse.move(vp.x + band.x1 + 4, vp.y + band.y1 + 2);
        await timed(`${tag}: crossing box over the story (3D)`, async () => {
          await page.mouse.down();
          await page.mouse.move(vp.x + (band.x0 + band.x1) / 2, vp.y + (band.y0 + band.y1) / 2, { steps: 4 });
          await page.mouse.move(vp.x + band.x0 - 4, vp.y + band.y0 - 2, { steps: 4 });
          await page.mouse.up();
        });
        report[`${tag}Box3D`] = await selectionSummary();
        await shot(`${tag}-3d-boxed`);
        await timed(`${tag}: clear selection (Esc), 3D`, () => page.keyboard.press("Escape"));
      }
    }
  }

  if (want("check")) {
    // Analyze > Check Model: the dialog lists what the check found.
    await page.mouse.move(vp.x + 5, vp.y + 5);
    await timed(`${tag}: check model`, () => state(() => window.__osab.state().openDialog("check")), "document.querySelector('.dialog') !== null");
    report[`${tag}Check`] = await state(() => document.querySelector(".dialog")?.textContent?.slice(0, 400) ?? null);
    await shot(`${tag}-check`);
    await state(() => window.__osab.state().openDialog(null));
  }

  if (want("edit")) {
    // An edit of one joint (made special), then Undo: each a new model to draw.
    await page.mouse.move(vp.x + 5, vp.y + 5);
    await timed(`${tag}: edit one joint`, () =>
      state(() => {
        const s = window.__osab.state();
        const id = s.model.nodes[Math.floor(s.model.nodes.length / 2)].id;
        s.edit((m) => ({ ...m, nodes: m.nodes.map((n) => (n.id === id ? { ...n, special: !n.special } : n)) }));
      }),
    );
    await timed(`${tag}: undo`, () => state(() => window.__osab.state().undo()));
  }

  if (want("hover")) {
    const n = 40;
    await timedRun(
      `${tag}: hover`,
      n,
      (i) => page.mouse.move(vp.x + vp.w * (0.3 + (0.4 * i) / (n - 1)), vp.cy + vp.h * 0.1 * Math.sin(i / 3)),
      () => page.mouse.move(vp.x + vp.w * 0.3, vp.cy),
    );
  }

  if (want("rotate")) {
    await timedRun(
      `${tag}: rotate (right-drag)`,
      40,
      (i) => page.mouse.move(vp.cx + 8 * (i + 1), vp.cy + (i % 2)),
      async () => {
        await page.mouse.move(vp.cx, vp.cy);
        await page.mouse.down({ button: "right" });
      },
      () => page.mouse.up({ button: "right" }),
    );
    await page.evaluate(() => window.__vt.settle(performance.now()));
    await shot(`${tag}-rotated`);
    await timedRun(
      `${tag}: zoom (wheel)`,
      20,
      (i) => page.mouse.wheel(0, i < 10 ? -120 : 120),
      () => page.mouse.move(vp.cx, vp.cy),
    );
    await page.evaluate(() => window.__vt.settle(performance.now()));
  }
}

async function labelSteps(tag) {
  // Joint and member labels on (at most 2000 texts are drawn), then a rotation: texts follow every frame.
  const set = (on) =>
    state(([v]) => {
      const s = window.__osab.state();
      const o = s.display.options;
      s.setDisplay({ options: { ...o, joint: { ...o.joint, labels: v }, frame: { ...o.frame, labels: v } } });
    }, [on]);
  await timed(`${tag}: labels on`, () => set(true));
  await shot(`${tag}-labels`);
  await timedRun(
    `${tag}: rotate, labels on`,
    20,
    (i) => page.mouse.move(vp.cx + 8 * (i + 1), vp.cy),
    async () => {
      await page.mouse.move(vp.cx, vp.cy);
      await page.mouse.down({ button: "right" });
    },
    () => page.mouse.up({ button: "right" }),
  );
  await page.evaluate(() => window.__vt.settle(performance.now()));
  await timed(`${tag}: labels off`, () => set(false));
}

async function resultSteps(tag) {
  // Run (F5); the model check's warnings are run anyway.
  await startSampling();
  const t0 = Date.now();
  await page.keyboard.press("F5");
  let logged = 0;
  for (;;) {
    const s = await state(() => ({ running: window.__osab.state().running, results: window.__osab.state().results !== null, dialog: window.__osab.state().dialog, notice: window.__osab.state().notice, heap: window.__vt.heapMB() }));
    if (Date.now() - t0 > logged + 10000) {
      logged = Date.now() - t0;
      console.log(`    ${Math.round(logged / 1000)} s: running ${s.running}, results ${s.results}, heap ${s.heap} MB`);
    }
    if (s.results && !s.running) break;
    if (s.dialog === "check") {
      const run = page.locator(".dialog button.primary");
      if ((await run.count()) === 0) throw new Error("the model check found errors");
      await run.click();
    }
    if (!s.running && !s.results && s.notice?.level === "error") throw new Error(`analysis failed: ${s.notice.text}`);
    if (Date.now() - t0 > 30 * 60000) throw new Error("analysis took over 30 min");
    await new Promise((r) => setTimeout(r, 250));
  }
  report[`${tag}RunSeconds`] = Math.round((Date.now() - t0) / 100) / 10;
  console.log(`  ${tag}: analysis ${report[`${tag}RunSeconds`]} s (results in the window; heap ${await page.evaluate(() => window.__vt.heapMB())} MB)`);
  // The run ends in the deformed shape: drawn when the first case's values are here.
  const t1 = await page.evaluate(() => performance.now());
  const drawn = await page.evaluate(([t]) => window.__vt.settle(t, "!window.__osab.state().results.cases.some((c, i) => i === 0 && c.pending)"), [t1]);
  console.log(`  ${tag}: deformed shape after the run ${Math.round(drawn)} ms (heap ${await page.evaluate(() => window.__vt.heapMB())} MB)`);
  report[`${tag}AfterRunMs`] = Math.round(drawn);
  const runMem = await stopSampling();
  if (runMem) {
    report.steps.push({ name: `${tag}: run and first view`, liveMB: runMem.liveMB, memory: runMem });
    console.log(`  ${tag}: live after the run and its first view ${runMem.liveMB} MB`);
  }
  await resultViews(tag);
}

async function resultViews(tag) {
  const shown = "(() => { const s = window.__osab.state(); const r = s.results; const c = r.cases.find((c) => c.id === s.display.caseId) ?? r.cases[0]; return !c.pending; })()";
  await timed(`${tag}: model view (results hidden)`, () => state(() => window.__osab.state().setDisplay({ mode: "model" })));
  await timed(`${tag}: deformed shape`, () => state(() => window.__osab.state().setDisplay({ mode: "deformed" })), shown);
  await shot(`${tag}-deformed`);
  if (want("rotate")) {
    await timedRun(
      `${tag}: rotate, deformed shape`,
      20,
      (i) => page.mouse.move(vp.cx + 8 * (i + 1), vp.cy),
      async () => {
        await page.mouse.move(vp.cx, vp.cy);
        await page.mouse.down({ button: "right" });
      },
      () => page.mouse.up({ button: "right" }),
    );
  }
  await timed(`${tag}: shell contours (M11)`, () => state(() => window.__osab.state().setDisplay({ mode: "shells", shellComponent: "M11" })), shown);
  await shot(`${tag}-contours`);
  if (want("hover")) {
    await timedRun(`${tag}: hover, contours`, 10, (i) => page.mouse.move(vp.x + vp.w * (0.35 + 0.03 * i), vp.cy), () => page.mouse.move(vp.x + vp.w * 0.35, vp.cy));
  }
  if (want("rotate")) {
    await timedRun(
      `${tag}: rotate, contours`,
      20,
      (i) => page.mouse.move(vp.cx + 8 * (i + 1), vp.cy),
      async () => {
        await page.mouse.move(vp.cx, vp.cy);
        await page.mouse.down({ button: "right" });
      },
      () => page.mouse.up({ button: "right" }),
    );
  }
  // Drawn when the shape is here (the desktop app sends it when the mode is shown).
  await timed(`${tag}: mode 1 shape`, () => state(() => window.__osab.state().setDisplay({ mode: "modes", modeNumber: 1, animate: false })), "!window.__osab.state().results.modal?.modes[0]?.pending");
  await shot(`${tag}-mode1`);
  // Animation: frames drawn in 5 s.
  const b0 = await busy();
  await startProfile();
  await state(() => {
    window.__vtStop = window.__vt.record();
    window.__osab.state().setDisplay({ animate: true });
  });
  await new Promise((r) => setTimeout(r, 5000));
  const ts = await state(() => {
    window.__osab.state().setDisplay({ animate: false });
    return window.__vtStop();
  });
  const b1 = await busy();
  const prof = await stopProfile(`${tag}: animate mode 1`);
  const gaps = ts.slice(1).map((t, i) => t - ts[i]);
  const span = ts.length > 1 ? ts[ts.length - 1] - ts[0] : 0;
  const step = {
    name: `${tag}: animate mode 1 (5 s)`,
    frames: ts.length,
    fps: Math.round((ts.length / Math.max(span, 1)) * 10000) / 10,
    longestFrameMs: Math.round(Math.max(0, ...gaps)),
    busyPerFrameMs: Math.round(((b1.task - b0.task) / Math.max(ts.length, 1)) * 10) / 10,
    ...(prof ? { profile: prof } : {}),
  };
  report.steps.push(step);
  console.log(`  ${step.name}: ${step.fps} fps, longest frame ${step.longestFrameMs} ms (busy ${step.busyPerFrameMs} ms per frame)`);
  await page.evaluate(() => window.__vt.settle(performance.now()));
  await timed(`${tag}: back to the model view`, () => state(() => window.__osab.state().setDisplay({ mode: "model" })));
}

async function persistenceSteps(path) {
  const expected = await state(() => {
    const r = window.__osab.state().results;
    if (!r) throw new Error("No results to save");
    return { cases: r.cases.length, modes: r.modal?.modes.length ?? 0 };
  });
  await timed("save .osab with results", () => page.evaluate(async (path) => {
    const s = window.__osab.state();
    await window.__TAURI_INTERNALS__.invoke("save_model", { path, model: s.model, withResults: true });
    s.markSaved(path.split(/[\\/]/).pop(), path);
  }, path));
  const verified = await page.evaluate(async (path) => {
    const r = await window.__TAURI_INTERNALS__.invoke("open_results", { path });
    return { cases: r?.cases.length ?? 0, modes: r?.modal?.modes.length ?? 0 };
  }, path);
  if (verified.cases !== expected.cases || verified.modes !== expected.modes)
    throw new Error("Saved results do not contain the expected cases and modes");
  report.savedResults = { ...verified, verified: true };
}

async function reopenedSteps() {
  await page.waitForFunction(() => window.__osab.state().results !== null || window.__osab.state().notice?.level === "error", null, { timeout: 180000 });
  const loaded = await state(() => {
    const s = window.__osab.state();
    return { path: s.filePath, cases: s.results?.cases.length ?? 0, modes: s.results?.modal?.modes.length ?? 0, notice: s.notice };
  });
  if (!loaded.cases) throw new Error(`Saved results did not reopen: ${JSON.stringify(loaded)}`);
  if (loaded.path?.replaceAll("\\", "/").toLowerCase() !== reopenPath.replaceAll("\\", "/").toLowerCase())
    throw new Error("The app opened a different model path");
  await page.waitForFunction(() => !window.__osab.state().results?.cases[0]?.pending, null, { timeout: 180000 });
  await page.evaluate(() => window.__vt.settle(performance.now()));
  const wallMs = Date.now() - launchTime;
  const live = memory ? await liveMB() : null;
  const heap = await page.evaluate(() => window.__vt.heapMB());
  report.steps.push({ name: "fresh app: reopen .osab with saved results", wallMs, heapMB: heap, liveMB: live });
  report.savedResults = { cases: loaded.cases, modes: loaded.modes, verified: true };
  console.log(`  fresh app: reopen with saved results ${wallMs} ms (heap ${heap} MB, live ${live} MB)`);
  await modelSteps("saved-model");
  await resultViews("saved results");
}

let failed = false;
try {
  if (reopenPath) await reopenedSteps();
  if (!reopenPath && want("open")) {
    await openModel(modelPath, "open and draw");
    await modelSteps("model");
    if (want("labels")) await labelSteps("model");
  }
  if (!reopenPath && resultsPath) {
    await openModel(resultsPath, "open the results model");
    await resultSteps("results");
    if (saveAs) await persistenceSteps(saveAs);
  }
} catch (e) {
  failed = true;
  console.error(String(e));
} finally {
  report.when = new Date().toISOString();
  writeFileSync(join(out, "timing.json"), JSON.stringify(report, null, 2));
  console.log(`written ${join(out, "timing.json")}`);
  if (profile) {
    for (const s of report.steps.filter((x) => x.profile)) {
      console.log(`\n== ${s.name}: self ms`);
      for (const [k, v] of s.profile.self) console.log(`   ${String(v).padStart(7)}  ${k}`);
      console.log(`   -- including callees`);
      for (const [k, v] of s.profile.total) console.log(`   ${String(v).padStart(7)}  ${k}`);
    }
  }
  await browser.close().catch(() => {});
  app?.kill();
}
process.exit(failed ? 1 : 0);
