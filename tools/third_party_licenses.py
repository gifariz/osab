"""Writes THIRD_PARTY_LICENSES.md: the libraries OSAB's programs are built with (Rust crates of the solver, mesher,
command line, web demo and desktop app; npm packages of the app's runtime), with their licences.

  python tools/third_party_licenses.py

Needs cargo and npm (run `npm ci` in gui/ first). Development-only tools (test runners, build tools) are left out:
they are not part of what OSAB ships.
"""

import collections
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OWN = {"osab-solver", "osab-mesh", "osab-cli", "osab-web", "osab-gui"}


def rust():
    out = subprocess.run(
        ["cargo", "tree", "-e", "normal", "--prefix", "none", "--no-dedupe", "--format", "{p}|{l}",
         *[a for p in sorted(OWN) for a in ("-p", p)]],
        cwd=ROOT, capture_output=True, text=True, check=True).stdout
    rows = set()
    for line in out.splitlines():
        if "|" not in line:
            continue
        name_ver, lic = line.split("|", 1)
        parts = name_ver.split()
        if len(parts) < 2 or parts[0] in OWN:
            continue
        rows.add((parts[0], parts[1].lstrip("v"), lic.strip() or "(none given)"))
    return sorted(rows)


def npm():
    gui = ROOT / "gui"
    out = subprocess.run("npm ls --omit=dev --all --parseable", cwd=gui, capture_output=True, text=True,
                         shell=True).stdout
    rows = set()
    for d in out.splitlines()[1:]:
        p = json.loads((Path(d.strip()) / "package.json").read_text(encoding="utf-8"))
        lic = p.get("license")
        lic = lic.get("type") if isinstance(lic, dict) else lic
        rows.add((p["name"], p["version"], lic or "(none given)"))
    return sorted(rows)


def table(rows):
    lines = ["| Library | Version | Licence |", "|---|---|---|"]
    lines += [f"| {n} | {v} | {l} |" for n, v, l in rows]
    return "\n".join(lines)


def summary(rows):
    count = collections.Counter(l for _, _, l in rows)
    return ", ".join(f"{l} ({n})" for l, n in count.most_common())


def main():
    r, n = rust(), npm()
    text = f"""# Third-party licences

OSAB is licensed under the Apache License 2.0 (see LICENSE and NOTICE). It is built with the libraries below,
each under its own licence; all of them allow use in OSAB under Apache-2.0. The MPL-2.0 crates (used by the
desktop app's web view) are used unchanged; their source is on crates.io. Development tools (test runners,
build tools) are not part of what OSAB ships and are not listed.

Made by `python tools/third_party_licenses.py`; run it again when dependencies change.

## npm packages (the app's user interface)

{len(n)} packages: {summary(n)}.

{table(n)}

## Rust crates (solver, mesher, command line, web demo, desktop app)

{len(r)} crates: {summary(r)}.

{table(r)}
"""
    (ROOT / "THIRD_PARTY_LICENSES.md").write_text(text, encoding="utf-8")
    print(f"written THIRD_PARTY_LICENSES.md: {len(n)} npm packages, {len(r)} Rust crates")


if __name__ == "__main__":
    main()
