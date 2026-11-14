# OSAB

**OSAB** (Open Structural Analysis for Buildings) is a free, easy-to-use structural FEM app: model visually, analyze, see results. Frames first, then shells and more.

**Try it in your browser:** https://gifariz.com/osab/ (web demo; models up to 8,000 analysis
joints; nothing is uploaded).

**Download:** [the latest release](https://github.com/gifariz/osab/releases/latest): the Windows desktop app
(installer or portable) and the command-line tool for Windows and Linux.

**What it does:** a desktop app (Windows, English or Indonesian) to draw frames, slabs and walls on a
grid, assign sections, supports, springs, rigid links, releases, end offsets, diaphragms, property
modifiers and loads, and run linear static and modal analysis: deformed shapes, force diagrams, shell
force contours, mode shapes, reactions and tables. Floors and walls are meshed automatically.

## Verification
OSAB's results are checked against hand calculations and OpenSees on its benchmark models, and against a
commercial building analysis program (those results are not published). The table,
and what OSAB does differently: [docs/VERIFICATION.md](docs/VERIFICATION.md). Where each modeling rule
comes from: [docs/SOURCES.md](docs/SOURCES.md).

## Use the app
- **Installer:** `OSAB_0.1.0_x64-setup.exe`, or the portable `osab-gui.exe` (needs WebView2, which
  Windows 10/11 normally have). Both are built by CI on `main` (GitHub Actions artifact *OSAB-windows*)
  or locally with `npm run app:build` (see below). The EXE is not code-signed yet, so Windows may warn
  the first time.
- How to model a portal frame, a slab and a wall: [docs/USER_GUIDE.md](docs/USER_GUIDE.md).

## For scripts and agents
OSAB runs without a window, so scripts, servers and AI agents can use it: write a model as JSON, run the
command-line tool, read the results as JSON.

1. **Get the tool:** the single-file `osab` (Linux) or `osab.exe` (Windows) from the release page, or build it
   with `cargo build --release -p osab-cli` (the file is then in `target/release/`).
2. **Write a model:** an `.osab` file is JSON; every field is described in [docs/MODEL_FORMAT.md](docs/MODEL_FORMAT.md),
   and [examples/portal_frame.osab](examples/portal_frame.osab) is a small complete one. A model saved in the app
   works too.
3. **Run it:**
   ```
   osab solve model.osab -o results.json
   ```
   It meshes the model, runs every load case, combination, modal and spectrum case, prints one line per case
   (largest displacement and the equilibrium check) and writes the results. `--only displacements,reactions,frames`
   keeps only the parts you need (also `links`, `shells`, `stories`, `piers`); It uses every core (`--threads N` sets fewer).
   A model that cannot run gives an error message naming the object and the reason, and a non-zero exit code.
4. **Read the results:** one JSON object, `{"title", "units", "notes", "cases": [...]}`. Each case has its `id`, the
   joint `displacements` and `reactions` (`ux uy uz rx ry rz`, `fx fy fz mx my mz`), each frame's `stations`
   along its length (`x`, `P V2 V3 T M2 M3` in its local axes, and the displacements there), shell corner forces,
   the `equilibrium` check and the `base_reaction`; the fields are in
   [docs/MODEL_FORMAT.md](docs/MODEL_FORMAT.md#results-file).

For a quick call without files, `osab api analyze` reads a model on standard input and answers
`{"ok": results}` or `{"error": ...}` on standard output, the same replies the app gets.

## Quick start (developers)
Needs Rust (stable) and, for the app, Node.js and the VS C++ build tools.
```
cargo test                                           # solver unit tests + all benchmarks
cargo run -p osab-cli -- solve examples/portal_frame.osab -o results.json
cargo run -p osab-cli -- bench                      # comparison table for every benchmark
```
Desktop app (in `gui/`):
```
npm install          # once
rustup target add wasm32-unknown-unknown   # once: the solver's browser build (dev:web, tests)
npm run app          # the desktop app in development mode (Tauri)
npm run dev:web      # the same UI in a browser at http://localhost:5173 (solver as WebAssembly)
npm test             # UI unit tests (Vitest)
npm run e2e          # Playwright: builds benchmark models by clicking and compares their results with the references
npm run app:build    # release: target/release/osab-gui.exe and the NSIS installer
npm run smoke        # drives the real release EXE and checks B05 and B24 through the app's solver commands
npm run build:web    # the web demo as static files in gui/dist-web/ (site path /osab/)
npm run e2e:web      # Playwright against that build (served at http://localhost:4173/osab/)
```

## Layout
| Folder | What |
|---|---|
| `solver/` | Rust solver library (`osab-solver`) |
| `mesh/` | Mesh step (`osab-mesh`): turns drawn floors and walls into analysis elements |
| `web/` | The solver and mesh step as WebAssembly for the web demo |
| `cli/` | `osab` developer command line |
| `gui/` | Desktop app: React + three.js UI (`src/`), Tauri shell (`src-tauri/`), click test (`e2e/`) |
| `tests/benchmarks/` | Benchmark models with reference results (`hand.json`, `opensees.json`, `opensees_stations.json`) |
| `examples/` | Example models |
| `docs/` | Model format, plans, user guide |

## Docs
- [docs/USER_GUIDE.md](docs/USER_GUIDE.md): using the app
- [docs/VERIFICATION.md](docs/VERIFICATION.md): benchmarks and differences
- [docs/SOURCES.md](docs/SOURCES.md): where each modeling rule comes from
- [docs/MODEL_FORMAT.md](docs/MODEL_FORMAT.md): model JSON, axes and sign conventions
- [ROADMAP.md](ROADMAP.md): phases and what comes when

## Licence
Apache License 2.0: see [LICENSE](LICENSE) and [NOTICE](NOTICE). The libraries OSAB is built with and their
licences are listed in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) (`python tools/third_party_licenses.py`).

## Disclaimer
OSAB is a tool for engineers: the engineer using it is responsible for the model, for checking its results and for the design decisions made with them.
