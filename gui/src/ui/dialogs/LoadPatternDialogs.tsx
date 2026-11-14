// Define > Load Patterns (name, type, self-weight multiplier, a Wind pattern's user wind loads) and
// Define > Load Cases (rows of load pattern and scale factor).

import { useState } from "react";

import { t, type Key } from "../../i18n";
import { copyDefinition, deleteLoadCase, deleteLoadPattern, setLoadCase, setWindLoads, upsertLoadPattern, withStories, type WindSettings } from "../../model/ops";
import { PATTERN_TYPES, type LoadCase, type LoadPattern, type PatternType, type WindLoad } from "../../model/types";
import { inputNumber } from "../../scene/format";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import type { Quantity } from "../../units/units";
import { Dialog, NumberField, parseNumber } from "../Dialog";
import { ListPicker, initialItem } from "./DefineDialogs";
import { SpectrumCaseForm } from "./SpectrumDialogs";

const close = () => useStore.getState().openDialog(null);

export function LoadPatternsDialog() {
  const model = useStore((s) => s.model);
  const [selected, setSelected] = useState<string | null>(() => initialItem(model.load_patterns));
  const current = model.load_patterns.find((p) => p.id === selected);
  return (
    <Dialog title={t("lp.title")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("lp.list")} items={model.load_patterns.map((p) => p.id)} selected={selected} onSelect={setSelected} />
        <PatternForm key={selected ?? "(new)"} pattern={current} onSaved={setSelected} />
      </div>
    </Dialog>
  );
}

function PatternForm({ pattern, onSaved }: { pattern?: LoadPattern; onSaved(id: string | null): void }) {
  const [name, setName] = useState(pattern?.id ?? "");
  const [type, setType] = useState<PatternType>(pattern?.type ?? "dead");
  const [multiplier, setMultiplier] = useState(inputNumber(pattern?.self_weight ?? 0));
  const edit = useStore((s) => s.edit);
  const save = () => {
    const sw = parseNumber(multiplier, true);
    if (sw === null) return useStore.getState().notify(t("err.selfWeightNumber"), "error");
    if (edit((m) => upsertLoadPattern(m, pattern?.id ?? null, { id: name, type, self_weight: sw }))) onSaved(name.trim());
  };
  const copy = () => {
    if (!pattern) return;
    let copied = "";
    if (
      edit((m) => {
        const r = copyDefinition(m, "pattern", pattern.id);
        copied = r.id;
        return r.model;
      })
    )
      onSaved(copied);
  };
  return (
    <div className="define-form">
      <h4>{pattern ? t("lp.pattern", { id: pattern.id }) : t("lp.new")}</h4>
      <NumberField label={t("dlg.name")} value={name} onChange={setName} width={140} />
      <label className="row">
        <span>{t("lp.type")}</span>
        <select aria-label={t("lp.type")} value={type} onChange={(e) => setType(e.target.value as PatternType)} style={{ width: 180 }}>
          {PATTERN_TYPES.map((k) => (
            <option key={k} value={k}>
              {t(`lp.type.${k}` as Key)}
            </option>
          ))}
        </select>
      </label>
      <NumberField label={t("df.selfWeight")} value={multiplier} onChange={setMultiplier} width={80} />
      <p className="hint">{t("lp.hint")}</p>
      <p className="hint">{t("df.selfWeightHint")}</p>
      <div className="buttons">
        <button className="primary" onClick={save}>
          {pattern ? t("dlg.apply") : t("dlg.add")}
        </button>
        {pattern && <button onClick={copy}>{t("ex.copy")}</button>}
        {pattern && (
          <button
            onClick={() => {
              if (edit((m) => deleteLoadPattern(m, pattern.id))) onSaved(null);
            }}
          >
            {t("dlg.delete")}
          </button>
        )}
      </div>
      {pattern?.type === "wind" && <WindLoadsForm key={pattern.id} pattern={pattern} />}
    </div>
  );
}

/** A user wind load row as typed (values in display units). */
interface WindRow {
  story: string;
  diaphragm: string;
  fx: string;
  fy: string;
  mz: string;
  x: string;
  y: string;
}

const WIND_VALUES: { key: "fx" | "fy" | "mz" | "x" | "y"; label: Key; q: Quantity }[] = [
  { key: "fx", label: "lp.fx", q: "force" },
  { key: "fy", label: "lp.fy", q: "force" },
  { key: "mz", label: "lp.mz", q: "moment" },
  { key: "x", label: "lp.x", q: "distance" },
  { key: "y", label: "lp.y", q: "distance" },
];

/** The user wind loads of a Wind pattern: story forces per story and diaphragm at a point. */
function WindLoadsForm({ pattern }: { pattern: LoadPattern }) {
  const model = useStore((s) => s.model);
  const u = useUnits();
  const stories = (withStories(model).stories ?? []).slice(1).map((s) => s.id);
  const dias = (model.diaphragms ?? []).map((d) => d.id);
  const rowOf = (w: WindLoad): WindRow => ({
    story: w.story,
    diaphragm: w.diaphragm,
    fx: u.show(w.fx ?? 0, "force"),
    fy: u.show(w.fy ?? 0, "force"),
    mz: u.show(w.mz ?? 0, "moment"),
    x: u.show(w.x, "distance"),
    y: u.show(w.y, "distance"),
  });
  const blank = (): WindRow => ({ story: stories[stories.length - 1] ?? "", diaphragm: dias[0] ?? "", fx: "0", fy: "0", mz: "0", x: "0", y: "0" });
  const [rows, setRows] = useState<WindRow[]>(() => (pattern.wind_loads ?? []).map(rowOf));
  const [point, setPoint] = useState<"centre-of-mass" | "coordinates">(pattern.wind_point ?? "centre-of-mass");
  const [semi, setSemi] = useState<"floor" | "mass" | "nearest-joint">(pattern.wind_semi_rigid ?? "floor");
  const atCentre = point === "centre-of-mass";
  const [paste, setPaste] = useState("");
  const [error, setError] = useState("");
  const setRow = (i: number, patch: Partial<WindRow>) => setRows(rows.map((r, k) => (k === i ? { ...r, ...patch } : r)));
  const save = () => {
    const out: WindLoad[] = [];
    for (const r of rows) {
      const v: Partial<Record<(typeof WIND_VALUES)[number]["key"], number>> = {};
      for (const c of WIND_VALUES) {
        const x = u.read(r[c.key], c.q, true);
        if (x === null) return setError(t("err.windNumbers"));
        v[c.key] = x;
      }
      out.push({ story: r.story, diaphragm: r.diaphragm, fx: v.fx, fy: v.fy, mz: v.mz, x: v.x!, y: v.y! });
    }
    const settings: WindSettings = {
      ...(point === "coordinates" ? { wind_point: "coordinates" as const } : {}),
      ...(semi !== "floor" ? { wind_semi_rigid: semi } : {}),
    };
    if (useStore.getState().edit((m) => setWindLoads(m, pattern.id, out, settings))) setError("");
  };
  // Rows pasted from a spreadsheet: story, diaphragm, Fx, Fy, Mz, X, Y per line (tabs, commas or spaces).
  const readPaste = () => {
    const lines = paste.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
    const add: WindRow[] = [];
    for (const l of lines) {
      const p = l.split(/[\t,;]+|\s+/).filter(Boolean);
      if (p.length !== 7) return setError(t("lp.pasteBad", { line: l }));
      add.push({ story: p[0], diaphragm: p[1], fx: p[2], fy: p[3], mz: p[4], x: p[5], y: p[6] });
    }
    setRows([...rows, ...add]);
    setPaste("");
    setError("");
  };
  return (
    <div className="wind-loads">
      <h4>{t("lp.windTitle")}</h4>
      <p className="hint">{t("lp.windHint")}</p>
      <label className="row">
        <span>{t("lp.windPoint")}</span>
        <select aria-label={t("lp.windPoint")} value={point} onChange={(e) => setPoint(e.target.value as typeof point)} style={{ width: 260 }}>
          <option value="centre-of-mass">{t("lp.windPoint.centre-of-mass")}</option>
          <option value="coordinates">{t("lp.windPoint.coordinates")}</option>
        </select>
      </label>
      <label className="row">
        <span>{t("lp.windSemi")}</span>
        <select aria-label={t("lp.windSemi")} value={semi} onChange={(e) => setSemi(e.target.value as typeof semi)} style={{ width: 360 }}>
          <option value="floor">{t("lp.windSemi.floor")}</option>
          <option value="mass">{t("lp.windSemi.mass")}</option>
          <option value="nearest-joint">{t("lp.windSemi.nearest-joint")}</option>
        </select>
      </label>
      <table className="data combo-rows">
        <thead>
          <tr>
            <th>{t("lp.story")}</th>
            <th>{t("lp.diaphragm")}</th>
            {WIND_VALUES.map((c) => (
              <th key={c.key}>
                {t(c.label)} ({u.label(c.q)})
              </th>
            ))}
            <th />
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td>
                <select aria-label={`${t("lp.story")} ${i + 1}`} value={r.story} onChange={(e) => setRow(i, { story: e.target.value })}>
                  {[...new Set([...stories, r.story])].map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <select aria-label={`${t("lp.diaphragm")} ${i + 1}`} value={r.diaphragm} onChange={(e) => setRow(i, { diaphragm: e.target.value })}>
                  {[...new Set([...dias, r.diaphragm])].map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </td>
              {WIND_VALUES.map((c) => (
                <td key={c.key}>
                  <input
                    aria-label={`${t(c.label)} ${i + 1}`}
                    value={r[c.key]}
                    onChange={(e) => setRow(i, { [c.key]: e.target.value })}
                    disabled={atCentre && (c.key === "x" || c.key === "y")}
                    title={atCentre && (c.key === "x" || c.key === "y") ? t("lp.xyUnused") : undefined}
                    style={{ width: 64 }}
                  />
                </td>
              ))}
              <td>
                <button aria-label={t("df.comboRemoveRow", { n: i + 1 })} onClick={() => setRows(rows.filter((_, k) => k !== i))}>
                  ✕
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="buttons">
        <button onClick={() => setRows([...rows, blank()])} disabled={!stories.length || !dias.length}>
          {t("df.comboAddRow")}
        </button>
      </div>
      <label className="row">
        <span>{t("lp.paste")}</span>
        <textarea aria-label={t("lp.paste")} rows={3} cols={40} value={paste} onChange={(e) => setPaste(e.target.value)} />
      </label>
      <div className="buttons">
        <button onClick={readPaste} disabled={!paste.trim()}>
          {t("lp.pasteRead")}
        </button>
        <button className="primary" onClick={save}>
          {t("lp.windApply")}
        </button>
      </div>
      {error && <p className="error">{error}</p>}
    </div>
  );
}

/** Define > Load Cases: linear static and response spectrum cases in one list; a new case's type is chosen in the form. */
export function LoadCasesDialog() {
  const model = useStore((s) => s.model);
  const spectrum = model.spectrum_cases ?? [];
  const all = [...model.load_cases, ...spectrum];
  const [selected, setSelected] = useState<string | null>(() => initialItem(all));
  const [newType, setNewType] = useState<"static" | "spectrum">("static");
  const current = model.load_cases.find((c) => c.id === selected);
  const currentSpectrum = spectrum.find((c) => c.id === selected);
  const typeSelect = (
    <label className="row">
      <span>{t("lc.caseType")}</span>
      <select aria-label={t("lc.caseType")} value={newType} onChange={(e) => setNewType(e.target.value as "static" | "spectrum")}>
        <option value="static">{t("lc.type.static")}</option>
        <option value="spectrum">{t("lc.type.spectrum")}</option>
      </select>
    </label>
  );
  const isSpectrum = currentSpectrum !== undefined || (selected === null && newType === "spectrum");
  const key = `${selected ?? "(new)"}|${selected === null ? newType : ""}`;
  return (
    <Dialog title={t("df.loadCases")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("df.loadCasesList")} items={all.map((c) => c.id)} selected={selected} onSelect={setSelected} />
        {isSpectrum ? (
          <SpectrumCaseForm key={key} spectrumCase={currentSpectrum} onSaved={setSelected} typeSelect={selected === null ? typeSelect : <CaseType spectrum />} />
        ) : (
          <CaseForm key={key} loadCase={current} onSaved={setSelected} typeSelect={selected === null ? typeSelect : <CaseType />} />
        )}
      </div>
    </Dialog>
  );
}

/** The type of an existing case (it cannot change). */
function CaseType({ spectrum }: { spectrum?: boolean }) {
  return (
    <p className="row">
      <span>{t("lc.caseType")}</span>
      <span>{t(spectrum ? "lc.type.spectrum" : "lc.type.static")}</span>
    </p>
  );
}

interface CaseRow {
  pattern: string;
  scale: string;
}

function CaseForm({ loadCase, onSaved, typeSelect }: { loadCase?: LoadCase; onSaved(id: string | null): void; typeSelect?: React.ReactNode }) {
  const model = useStore((s) => s.model);
  const first = model.load_patterns[0]?.id ?? "";
  const [id, setId] = useState(loadCase?.id ?? "");
  const [rows, setRows] = useState<CaseRow[]>(
    loadCase ? loadCase.loads.map((l) => ({ pattern: l.pattern, scale: inputNumber(l.scale) })) : first ? [{ pattern: first, scale: "1" }] : [],
  );
  const [error, setError] = useState("");
  const setRow = (i: number, patch: Partial<CaseRow>) => setRows(rows.map((r, k) => (k === i ? { ...r, ...patch } : r)));
  const save = () => {
    const loads: LoadCase["loads"] = [];
    for (const r of rows) {
      const s = parseNumber(r.scale);
      if (s === null) return setError(t("err.caseScale"));
      loads.push({ pattern: r.pattern, scale: s });
    }
    if (useStore.getState().edit((m) => setLoadCase(m, loadCase?.id ?? null, { id, loads }))) {
      setError("");
      onSaved(id.trim());
    }
  };
  return (
    <div className="define-form combo-form">
      <h4>{loadCase ? t("df.loadCase", { id: loadCase.id }) : t("df.newLoadCase")}</h4>
      {typeSelect}
      <label className="row">
        <span>{t("dlg.name")}</span>
        <input aria-label={t("dlg.name")} value={id} onChange={(e) => setId(e.target.value)} style={{ width: 140 }} />
      </label>
      <table className="data combo-rows">
        <thead>
          <tr>
            <th>{t("lc.pattern")}</th>
            <th>{t("df.comboScale")}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td>
                <select aria-label={`${t("lc.pattern")} ${i + 1}`} value={r.pattern} onChange={(e) => setRow(i, { pattern: e.target.value })}>
                  {model.load_patterns.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.id}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <input aria-label={`${t("df.comboScale")} ${i + 1}`} value={r.scale} onChange={(e) => setRow(i, { scale: e.target.value })} style={{ width: 80 }} />
              </td>
              <td>
                <button aria-label={t("df.comboRemoveRow", { n: i + 1 })} onClick={() => setRows(rows.filter((_, k) => k !== i))}>
                  ✕
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="buttons">
        <button onClick={() => setRows([...rows, { pattern: first, scale: "1" }])} disabled={!first}>
          {t("df.comboAddRow")}
        </button>
      </div>
      <p className="hint">{t("df.casesHint")}</p>
      {error && <p className="error">{error}</p>}
      <div className="buttons">
        <button className="primary" onClick={save}>
          {loadCase ? t("dlg.apply") : t("dlg.add")}
        </button>
        {loadCase && (
          <button
            onClick={() => {
              if (useStore.getState().edit((m) => deleteLoadCase(m, loadCase.id))) onSaved(null);
            }}
          >
            {t("dlg.delete")}
          </button>
        )}
      </div>
    </div>
  );
}

/** Display > Show Loads: the load pattern whose loads the model view draws, or none. */
export function ShowLoadsDialog() {
  const model = useStore((s) => s.model);
  const shown = useStore((s) => s.display.loadsPattern);
  const [pattern, setPattern] = useState(shown ?? model.load_patterns[0]?.id ?? "");
  const ok = () => {
    useStore.getState().setDisplay({ loadsPattern: pattern || null, mode: "model" });
    close();
  };
  return (
    <Dialog
      title={t("sl.title")}
      buttons={
        <>
          <button className="primary" onClick={ok}>
            {t("dlg.ok")}
          </button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <label className="row">
        <span>{t("sl.pattern")}</span>
        <select aria-label={t("sl.pattern")} value={pattern} onChange={(e) => setPattern(e.target.value)}>
          <option value="">{t("side.none")}</option>
          {model.load_patterns.map((p) => (
            <option key={p.id}>{p.id}</option>
          ))}
        </select>
      </label>
      <p className="hint">{t("sl.hint")}</p>
    </Dialog>
  );
}
