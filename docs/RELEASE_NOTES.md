# Release notes

## 0.1.0

The first public release of OSAB (Open Structural Analysis for Buildings): a free structural analysis app for
buildings, with a desktop app for Windows, a web demo and a command-line tool.

### Downloads
- **Windows desktop app:** `OSAB_0.1.0_x64-setup.exe` (installer; `.osab` files then open with a double-click), or
  `OSAB_0.1.0_x64_portable.exe` (no installation: put it anywhere and run it; needs Microsoft Edge WebView2, which
  Windows 10 and 11 have). Windows may warn about an unknown publisher on
  the first run (the installer is not code-signed yet): choose *More info > Run anyway*.
- **Command-line tool** (for scripts, servers and agents): `osab-cli-linux-x86_64` (Linux x86_64, one file, any distribution;
  rename it to `osab`) and `osab-cli-windows-x86_64.exe` (Windows; rename it to `osab.exe`). See *For scripts and agents* in the README.
- **Web demo:** https://www.gifariz.com/osab/ (nothing to install; models up to 8,000 analysis joints).

### What it does
- **Modelling:** frames and trusses (sections from shapes or a steel list, releases, end offsets, insertion points,
  property modifiers), floors and walls (thin and thick shells, membrane floors, openings), joint, line and area
  springs, rigid links, rigid and semi-rigid diaphragms, stories, groups, pier and spandrel labels.
- **Meshing:** automatic floor and wall meshes (general or rectangular, finer near openings and corners), stored
  meshes, areas kept as one element.
- **Loads:** load patterns (joint, frame and area loads, self-weight, user wind loads), load cases and load
  combinations (linear add, envelope, SRSS).
- **Analysis:** linear static, P-Delta (from the mass or iteratively from loads), modal analysis (Eigen or Ritz
  vectors), response spectrum cases (user functions or ASCE 7-16; CQC, SRSS, absolute; directional rules,
  eccentricity).
- **Results:** displacements, reactions, frame, link and shell forces, base reactions, story drifts and forces,
  pier and spandrel forces, centres of mass and rigidity, modal periods, mass ratios and load participation;
  deformed shapes, force diagrams and contours, tables, export to Excel and CSV.
- **Files:** models are `.osab` files (JSON inside, described in docs/MODEL_FORMAT.md); double-clicking one opens
  it in the desktop app. Results are saved next to the model and reopen without a new run.

### Large and untidy models
Tried on real building models of up to about 64,000 joints and 63,000 shells (analysed in about 7 minutes on a
12-core PC with 32 GB of memory):
- **Warnings instead of refusals:** an unstable model (a part not held, or a mechanism) runs with a tiny
  stiffness in each such direction, and the messages list them; a combination with no load cases, or a user
  wind load at a story where its diaphragm has no joints, is left out with a warning.
- **The modal case** holds the unstable directions, retries its completeness check, ends with a warning when its
  iterations stop improving, and if it still fails, the other cases keep their results.
- **Memory:** before the heavy part of a run, OSAB estimates the memory it needs and stops with a clear message
  (or runs anyway, if you choose) instead of making the computer stop answering.
- **Speed:** the solver uses every processor core (the command line too; `--threads N` for fewer).

### Verification
Every feature is checked on benchmark models against hand calculations and OpenSees, and against a commercial
program. The table is in docs/VERIFICATION.md; where each modeling rule comes from is in docs/SOURCES.md.

### Known limits
- The desktop app is for Windows only; on macOS and Linux, use the web demo or the command-line tool.
- Linear analysis only, apart from P-Delta: no nonlinear static cases, staged construction or time history yet.
- Not included: partial fixity springs at frame ends, compression-only or tension-only springs, absolute-add and
  range-add combinations, design checks (VERIFICATION.md, *Not included*).
- Not included yet: automatic code-based seismic and wind loads (give them as joint loads or user story loads),
  links fixed in only some directions (e.g. expansion joints), keeping one joint out of its floor's diaphragm,
  user story loads on seismic patterns, importing models from other programs.
- Memory: a large model needs a few times the memory of the factorized stiffness; about 600,000 equations
  fit a PC with 32 GB, and models with many combinations with a maximum and a minimum need more.
- Refused, with a message: an area kept as one element that overlaps a meshed area in the same plane, and a
  very short frame piece whose automatic end offsets are longer than the piece.

### Licence
Apache License 2.0. OSAB is a tool for engineers: the engineer using it is responsible for the model, for checking
its results and for the design decisions made with them.
