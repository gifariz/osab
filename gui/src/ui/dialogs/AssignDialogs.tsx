// Assign menu: joint restraints, joint springs, joint loads, frame section/type, frame loads. Each acts on
// the selection.

import { useState } from "react";

import { t, tn, type Key } from "../../i18n";

import { assignFrame, restraintsOf, setFrameLoads, setNodeLoads, setRestraints, setSprings, springsOf, type LoadMode, type FrameLoadSpec } from "../../model/ops";
import { FORCES, type LoadDir } from "../../model/types";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import type { Quantity } from "../../units/units";
import { Dialog, NumberField, parseNumber } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

/** Value of the "None" section choice (a null object); "" means keep. */
export const NONE = "\u0000none";

const DOF_LABELS: Key[] = ["as.dofUX", "as.dofUY", "as.dofUZ", "as.dofRX", "as.dofRY", "as.dofRZ"];

export function RestraintsDialog() {
  const model = useStore((s) => s.model);
  const nodes = useStore((s) => s.selection.nodes);
  const [flags, setFlags] = useState<boolean[]>(nodes.length ? restraintsOf(model, nodes[0]) : [false, false, false, false, false, false]);
  const apply = (ok: boolean) => {
    if (useStore.getState().edit((m) => setRestraints(m, nodes, flags)) && ok) close();
  };
  const preset = (f: boolean[]) => setFlags(f);
  return (
    <Dialog
      title={tn("as.restraintsTitle", nodes.length)}
      buttons={
        <>
          <button className="primary" onClick={() => apply(true)}>
            {t("dlg.ok")}
          </button>
          <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <div className="buttons presets">
        <button onClick={() => preset([true, true, true, true, true, true])}>{t("as.fixed")}</button>
        <button onClick={() => preset([true, true, true, false, false, false])}>{t("as.pinned")}</button>
        <button onClick={() => preset([false, false, true, false, false, false])}>{t("as.roller")}</button>
        <button onClick={() => preset([false, false, false, false, false, false])}>{t("as.free")}</button>
      </div>
      <div className="checks">
        {DOF_LABELS.map((label, i) => (
          <label key={label} className="check">
            <input type="checkbox" checked={flags[i]} onChange={(e) => setFlags(flags.map((f, k) => (k === i ? e.target.checked : f)))} /> {t(label)}
          </label>
        ))}
      </div>
      {model.dimension === "2D" && <p className="hint note">{t("as.planeRestraints")}</p>}
      {model.dimension === "grid" && <p className="hint note">{t("as.gridRestraints")}</p>}
    </Dialog>
  );
}

const SPRING_LABELS = ["UX", "UY", "UZ", "RX", "RY", "RZ"];

/** Assign > Joint Springs: six stiffness values in global axes, shown for the first joint. */
export function SpringsDialog() {
  const model = useStore((s) => s.model);
  const nodes = useStore((s) => s.selection.nodes);
  const U = useUnits();
  const springQ = (i: number): Quantity => (i < 3 ? "transStiffness" : "rotStiffness");
  const [values, setValues] = useState<string[]>(() => (nodes.length ? springsOf(model, nodes[0]) : [0, 0, 0, 0, 0, 0]).map((k, i) => U.show(k, springQ(i))));
  const [mode, setMode] = useState<LoadMode>("replace");
  const [error, setError] = useState("");
  const apply = (ok: boolean) => {
    const v: number[] = [];
    for (const [i, text] of values.entries()) {
      const n = U.read(text, springQ(i), true);
      if (n === null || n < 0) return setError(t("as.springsError"));
      v.push(n);
    }
    if (useStore.getState().edit((m) => setSprings(m, nodes, v, mode)) && ok) close();
  };
  return (
    <Dialog
      title={tn("as.springsTitle", nodes.length)}
      buttons={
        <>
          <button className="primary" onClick={() => apply(true)}>
            {t("dlg.ok")}
          </button>
          <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <p className="hint">{t("as.springsHint")}</p>
      <div className="field-grid">
        {SPRING_LABELS.map((label, i) => (
          <NumberField
            key={label}
            label={label}
            value={values[i]}
            onChange={(v) => setValues(values.map((x, k) => (k === i ? v : x)))}
            unit={U.label(springQ(i))}
            width={90}
          />
        ))}
      </div>
      {model.dimension === "2D" && <p className="hint note">{t("as.planeSprings")}</p>}
      {model.dimension === "grid" && <p className="hint note">{t("as.gridSprings")}</p>}
      <ModeRadio value={mode} onChange={setMode} noun={t("dlg.nounSprings")} />
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

export function PatternSelect({ value, onChange }: { value: string; onChange(v: string): void }) {
  const patterns = useStore((s) => s.model.load_patterns);
  return (
    <label className="row">
      <span>{t("dlg.loadPattern")}</span>
      <select aria-label={t("dlg.loadPattern")} value={value} onChange={(e) => onChange(e.target.value)}>
        {patterns.map((c) => (
          <option key={c.id}>{c.id}</option>
        ))}
      </select>
    </label>
  );
}

export function ModeRadio({ value, onChange, noun = t("dlg.nounLoads") }: { value: LoadMode; onChange(v: LoadMode): void; noun?: string }) {
  const opts: [LoadMode, string][] = [
    ["add", t("dlg.modeAdd", { noun })],
    ["replace", t("dlg.modeReplace", { noun })],
    ["delete", t("dlg.modeDelete", { noun })],
  ];
  return (
    <div className="radio-list">
      {opts.map(([m, label]) => (
        <label key={m}>
          <input type="radio" name="load-mode" checked={value === m} onChange={() => onChange(m)} /> {label}
        </label>
      ))}
    </div>
  );
}

/** Keeps the dialog's load pattern in step with the "show loads" choice so new loads are visible. */
export function usePattern(): [string, (v: string) => void] {
  const s = useStore.getState();
  const [patternId, setPatternId] = useState(s.display.loadsPattern ?? s.model.load_patterns[0]?.id ?? "");
  return [
    patternId,
    (v) => {
      setPatternId(v);
      useStore.getState().setDisplay({ loadsPattern: v });
    },
  ];
}

export function JointLoadsDialog() {
  const model = useStore((s) => s.model);
  const nodes = useStore((s) => s.selection.nodes);
  const [patternId, setPatternId] = usePattern();
  const [mode, setMode] = useState<LoadMode>("add");
  const [values, setValues] = useState<Record<string, string>>({ fx: "0", fy: "0", fz: "0", mx: "0", my: "0", mz: "0" });
  const [error, setError] = useState("");
  const U = useUnits();
  const apply = (ok: boolean) => {
    const v: Record<string, number> = {};
    for (const [i, f] of FORCES.entries()) {
      const n = U.read(values[f], i < 3 ? "force" : "moment", true);
      if (n === null) return setError(t("dlg.enterNumbers"));
      v[f] = n;
    }
    if (!patternId) return setError(t("dlg.definePatternFirst"));
    useStore.getState().setDisplay({ loadsPattern: patternId });
    if (useStore.getState().edit((m) => setNodeLoads(m, patternId, nodes, v, mode)) && ok) close();
  };
  const plane = model.dimension === "2D";
  return (
    <Dialog
      title={tn("as.jointLoadsTitle", nodes.length)}
      buttons={
        <>
          <button className="primary" onClick={() => apply(true)}>
            {t("dlg.ok")}
          </button>
          <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <PatternSelect value={patternId} onChange={setPatternId} />
      <p className="hint">{t("as.jointLoadsHint")}</p>
      <div className="field-grid">
        {FORCES.map((f, i) => (
          <NumberField
            key={f}
            label={f.toUpperCase()}
            value={values[f]}
            onChange={(v) => setValues({ ...values, [f]: v })}
            unit={U.label(i < 3 ? "force" : "moment")}
            width={90}
          />
        ))}
      </div>
      {plane && <p className="hint note">{t("as.planeLoads")}</p>}
      {model.dimension === "grid" && <p className="hint note">{t("as.gridLoads")}</p>}
      <ModeRadio value={mode} onChange={setMode} />
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

const DIRECTIONS: [string, Key][] = [
  ["G", "as.dirGravity"],
  ["X", "as.dirX"],
  ["Y", "as.dirY"],
  ["Z", "as.dirZ"],
  ["1", "as.dir1Frame"],
  ["2", "as.dir2"],
  ["3", "as.dir3"],
];

export function FrameLoadsDialog() {
  const frames = useStore((s) => s.selection.frames);
  const [patternId, setPatternId] = usePattern();
  const [type, setType] = useState<FrameLoadSpec["type"]>("uniform");
  const [dir, setDir] = useState("G");
  const [value, setValue] = useState("10");
  const [value2, setValue2] = useState("20");
  const [relative, setRelative] = useState(true);
  const [from, setFrom] = useState("0");
  const [to, setTo] = useState("1");
  const [at, setAt] = useState("0.5");
  const [mode, setMode] = useState<LoadMode>("add");
  const [error, setError] = useState("");
  const U = useUnits();
  // Distances along the member: fractions of its length, or in display units.
  const dist = (text: string, blankZero = false) => (relative ? parseNumber(text, blankZero) : U.read(text, "distance", blankZero));
  const apply = (ok: boolean) => {
    const v = U.read(value, type === "point" ? "force" : "forcePerLength");
    if (v === null) return setError(t("as.enterLoad"));
    if (!patternId) return setError(t("dlg.definePatternFirst"));
    // Gravity loads are typed positive downwards and stored as -Z.
    const d: LoadDir = dir === "G" ? "Z" : (dir as LoadDir);
    const w = dir === "G" ? -v : v;
    let spec: FrameLoadSpec;
    if (type === "uniform" || type === "trapezoidal") {
      const a = dist(from, true);
      const b = dist(to);
      if (a === null) return setError(t("as.enterStart"));
      const whole = relative && a === 0 && (b === null || b === 1);
      const range = whole ? {} : { from: a, ...(b === null ? {} : { to: b }) };
      if (type === "uniform") spec = { type: "uniform", dir: d, w, ...range };
      else {
        const v2 = U.read(value2, "forcePerLength");
        if (v2 === null) return setError(t("as.enterLoad"));
        spec = { type: "trapezoidal", dir: d, w1: w, w2: dir === "G" ? -v2 : v2, ...range };
      }
    } else {
      const x = dist(at);
      if (x === null) return setError(t("as.enterDistance"));
      spec = { type: "point", dir: d, p: w, at: x };
    }
    useStore.getState().setDisplay({ loadsPattern: patternId });
    if (useStore.getState().edit((m) => setFrameLoads(m, patternId, frames, type, spec, mode, relative)) && ok) close();
  };
  return (
    <Dialog
      title={tn("as.frameLoadsTitle", frames.length)}
      buttons={
        <>
          <button className="primary" onClick={() => apply(true)}>
            {t("dlg.ok")}
          </button>
          <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <div className="tabs">
        <button className={type === "uniform" ? "tab active" : "tab"} onClick={() => setType("uniform")}>
          {t("as.distributed")}
        </button>
        <button className={type === "trapezoidal" ? "tab active" : "tab"} onClick={() => setType("trapezoidal")}>
          {t("as.trapezoidal")}
        </button>
        <button className={type === "point" ? "tab active" : "tab"} onClick={() => setType("point")}>
          {t("as.point")}
        </button>
      </div>
      <PatternSelect value={patternId} onChange={setPatternId} />
      <label className="row">
        <span>{t("as.direction")}</span>
        <select aria-label={t("as.loadDirection")} value={dir} onChange={(e) => setDir(e.target.value)}>
          {DIRECTIONS.map(([k, label]) => (
            <option key={k} value={k}>
              {t(label)}
            </option>
          ))}
        </select>
      </label>
      <NumberField
        label={type === "uniform" ? t("as.loadW") : type === "trapezoidal" ? t("as.loadW1") : t("as.loadP")}
        value={value}
        onChange={setValue}
        unit={U.label(type === "point" ? "force" : "forcePerLength")}
      />
      {type === "trapezoidal" && <NumberField label={t("as.loadW2")} value={value2} onChange={setValue2} unit={U.label("forcePerLength")} />}
      <label className="check">
        <input type="checkbox" checked={relative} onChange={(e) => setRelative(e.target.checked)} /> {t("as.relative")}
      </label>
      {type !== "point" ? (
        <div className="field-row">
          <NumberField label={t("as.from")} value={from} onChange={setFrom} width={70} unit={relative ? "" : U.label("distance")} />
          <NumberField label={t("as.to")} value={to} onChange={setTo} width={70} unit={relative ? "" : U.label("distance")} />
        </div>
      ) : (
        <NumberField label={t("as.distance")} value={at} onChange={setAt} width={70} unit={relative ? "" : U.label("distance")} />
      )}
      <p className="hint">{t("as.frameLoadsHint")}</p>
      <ModeRadio value={mode} onChange={setMode} />
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

export function FrameAssignDialog() {
  const model = useStore((s) => s.model);
  const frames = useStore((s) => s.selection.frames);
  const first = model.frames.find((e) => e.id === frames[0]);
  const same = <K extends "section" | "material" | "type">(k: K) =>
    frames.every((id) => model.frames.find((e) => e.id === id)?.[k] === first?.[k]) ? first?.[k] ?? "" : "";
  const [section, setSection] = useState<string>(same("section") || (first && !first.section ? NONE : ""));
  const [material, setMaterial] = useState<string>(same("material"));
  const [type, setType] = useState<string>(same("type"));
  const [angle, setAngle] = useState(frames.length === 1 ? String(first?.angle ?? 0) : "");
  const [error, setError] = useState("");
  const apply = (ok: boolean) => {
    const a = angle.trim() === "" ? undefined : parseNumber(angle);
    if (a === null) return setError(t("as.angleNumber"));
    const patch = {
      ...(section ? { section: section === NONE ? "" : section } : {}),
      ...(material ? { material } : {}),
      ...(type ? { type: type as "frame" | "truss" } : {}),
      ...(a !== undefined ? { angle: a } : {}),
    };
    if (useStore.getState().edit((m) => assignFrame(m, frames, patch)) && ok) close();
  };
  const keep = <option value="">{t("dlg.keep")}</option>;
  return (
    <Dialog
      title={tn("as.frameAssignTitle", frames.length)}
      buttons={
        <>
          <button className="primary" onClick={() => apply(true)}>
            {t("dlg.ok")}
          </button>
          <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <label className="row">
        <span>{t("as.section")}</span>
        <select aria-label={t("as.section")} value={section} onChange={(e) => setSection(e.target.value)}>
          {keep}
          {model.sections.map((s) => (
            <option key={s.id}>{s.id}</option>
          ))}
          <option value={NONE}>{t("obj.noSection")}</option>
        </select>
      </label>
      <label className="row">
        <span>{t("as.material")}</span>
        <select aria-label={t("as.material")} value={material} onChange={(e) => setMaterial(e.target.value)}>
          {keep}
          {model.materials.map((s) => (
            <option key={s.id}>{s.id}</option>
          ))}
        </select>
      </label>
      <label className="row">
        <span>{t("as.type")}</span>
        <select aria-label={t("as.type")} value={type} onChange={(e) => setType(e.target.value)}>
          {keep}
          <option value="frame">{t("as.typeFrame")}</option>
          <option value="truss">{t("as.typeTruss")}</option>
        </select>
      </label>
      <NumberField label={t("as.localAngle")} value={angle} onChange={setAngle} unit="°" width={70} />
      <p className="hint">{t("as.angleHint")}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}
