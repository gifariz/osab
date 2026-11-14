// Define > Response Spectrum Functions (user tables and ASCE 7-16, with a plot) and the response spectrum
// case form of Define > Load Cases.

import { useEffect, useState } from "react";

import { t, type Key } from "../../i18n";
import { backend, BackendError, type SpectrumPoints } from "../../ipc/backend";
import { DEFAULT_MODES, G, deleteSpectrumCase, deleteSpectrumFunction, setSpectrumCase, setSpectrumFunction } from "../../model/spectrum";
import {
  DIRECTIONAL_COMBINATIONS,
  MODAL_COMBINATIONS,
  SITE_CLASSES,
  SPECTRUM_DIRS,
  type Asce716,
  type SpectrumCase,
  type SpectrumFunction,
  type SpectrumLoad,
} from "../../model/types";
import { inputNumber } from "../../scene/format";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import { Dialog, NumberField, parseNumber } from "../Dialog";
import { ListPicker, initialItem } from "./DefineDialogs";

const close = () => useStore.getState().openDialog(null);

export function SpectrumFunctionsDialog() {
  const model = useStore((s) => s.model);
  const list = model.spectrum_functions ?? [];
  const [selected, setSelected] = useState<string | null>(() => initialItem(list));
  const current = list.find((f) => f.id === selected);
  return (
    <Dialog title={t("rs.functions")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("rs.functionsList")} items={list.map((f) => f.id)} selected={selected} onSelect={setSelected} />
        <FunctionForm key={selected ?? "(new)"} fn={current} onSaved={setSelected} />
      </div>
    </Dialog>
  );
}

/** Table rows as typed: one "period value" pair per line (spaces, tabs or a comma between). */
const pointsText = (p: [number, number][]) => p.map(([x, v]) => `${inputNumber(x)}\t${inputNumber(v)}`).join("\n");

function readPoints(text: string): [number, number][] | null {
  const out: [number, number][] = [];
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue;
    const parts = line.trim().split(/[\s,;]+/);
    if (parts.length !== 2) return null;
    const [x, v] = parts.map(Number);
    if (!Number.isFinite(x) || !Number.isFinite(v)) return null;
    out.push([x, v]);
  }
  return out;
}

/** A starting table: a typical design spectrum shape, in g. */
const STARTER: [number, number][] = [
  [0, 0.4],
  [0.1, 1],
  [0.5, 1],
  [1, 0.5],
  [2, 0.25],
  [4, 0.125],
];

function FunctionForm({ fn, onSaved }: { fn?: SpectrumFunction; onSaved(id: string | null): void }) {
  const [id, setId] = useState(fn?.id ?? "");
  const [kind, setKind] = useState<"table" | "asce">(fn?.asce7_16 ? "asce" : "table");
  const [damping, setDamping] = useState(inputNumber(fn?.damping ?? 0.05));
  const [table, setTable] = useState(pointsText(fn?.points ?? STARTER));
  const a = fn?.asce7_16;
  const [ss, setSs] = useState(inputNumber(a?.ss ?? 1));
  const [s1, setS1] = useState(inputNumber(a?.s1 ?? 0.4));
  const [tl, setTl] = useState(inputNumber(a?.tl ?? 8));
  const [site, setSite] = useState<Asce716["site_class"]>(a?.site_class ?? "D");
  const [fa, setFa] = useState(a?.fa === undefined ? "" : inputNumber(a.fa));
  const [fv, setFv] = useState(a?.fv === undefined ? "" : inputNumber(a.fv));
  const [error, setError] = useState("");

  /** The function as typed, or a message when a field is not a number. */
  const typed = (): SpectrumFunction | string => {
    const d = parseNumber(damping);
    if (d === null) return t("dlg.enterNumbers");
    if (kind === "table") {
      const points = readPoints(table);
      return points ? { id: id.trim(), damping: d, points } : t("rs.badTable");
    }
    const [vs, v1, vl] = [ss, s1, tl].map((x) => parseNumber(x));
    const [vfa, vfv] = [fa, fv].map((x) => (x.trim() === "" ? undefined : parseNumber(x)));
    if (vs === null || v1 === null || vl === null || vfa === null || vfv === null) return t("dlg.enterNumbers");
    const asce: Asce716 = { ss: vs, s1: v1, tl: vl, site_class: site };
    if (vfa !== undefined) asce.fa = vfa;
    if (vfv !== undefined) asce.fv = vfv;
    return { id: id.trim() || "RS", damping: d, asce7_16: asce };
  };

  // The plot: the solver's points (an ASCE 7-16 function's come from the code's tables there).
  const [plot, setPlot] = useState<SpectrumPoints | null>(null);
  const [plotError, setPlotError] = useState("");
  const shown = typed();
  const key = JSON.stringify(shown);
  useEffect(() => {
    if (typeof shown === "string") {
      setPlot(null);
      setPlotError(shown);
      return;
    }
    let live = true;
    const timer = setTimeout(() => {
      backend.spectrum({ ...shown, id: shown.id || "RS" }).then(
        (p) => live && (setPlot(p), setPlotError("")),
        (e) => live && (setPlot(null), setPlotError(e instanceof BackendError ? e.message : String(e))),
      );
    }, 150);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  const save = () => {
    const f = typed();
    if (typeof f === "string") return setError(f);
    if (useStore.getState().edit((m) => setSpectrumFunction(m, fn?.id ?? null, { ...f, id: id.trim() }))) {
      setError("");
      onSaved(id.trim());
    }
  };
  const remove = () => {
    if (fn && useStore.getState().edit((m) => deleteSpectrumFunction(m, fn.id))) onSaved(null);
  };

  return (
    <div className="define-form spectrum-form">
      <h4>{fn ? t("rs.function", { id: fn.id }) : t("rs.newFunction")}</h4>
      <label className="row">
        <span>{t("dlg.name")}</span>
        <input aria-label={t("dlg.name")} value={id} onChange={(e) => setId(e.target.value)} style={{ width: 140 }} />
      </label>
      <label className="row">
        <span>{t("rs.kind")}</span>
        <select aria-label={t("rs.kind")} value={kind} onChange={(e) => setKind(e.target.value as "table" | "asce")} style={{ width: 160 }}>
          <option value="table">{t("rs.kind.table")}</option>
          <option value="asce">{t("rs.kind.asce")}</option>
        </select>
      </label>
      <NumberField label={t("rs.damping")} value={damping} onChange={setDamping} width={70} />
      {kind === "table" ? (
        <label className="field spectrum-table">
          <span className="field-label">{t("rs.points")}</span>
          <textarea aria-label={t("rs.points")} rows={8} cols={24} value={table} onChange={(e) => setTable(e.target.value)} spellCheck={false} />
        </label>
      ) : (
        <div className="field-grid">
          <NumberField label={t("rs.ss")} value={ss} onChange={setSs} unit="g" width={70} />
          <NumberField label={t("rs.s1")} value={s1} onChange={setS1} unit="g" width={70} />
          <NumberField label={t("rs.tl")} value={tl} onChange={setTl} unit="s" width={70} />
          <label className="field">
            <span className="field-label">{t("rs.siteClass")}</span>
            <select aria-label={t("rs.siteClass")} value={site} onChange={(e) => setSite(e.target.value as Asce716["site_class"])}>
              {SITE_CLASSES.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </label>
          <NumberField label={t("rs.fa")} value={fa} onChange={setFa} width={70} />
          <NumberField label={t("rs.fv")} value={fv} onChange={setFv} width={70} />
        </div>
      )}
      <p className="hint">{t(kind === "table" ? "rs.tableHint" : "rs.asceHint")}</p>
      {plot?.asce7_16 && (
        <p className="hint note" data-testid="asce-values">
          {t("rs.asceValues", {
            fa: inputNumber(round(plot.asce7_16.fa)),
            fv: inputNumber(round(plot.asce7_16.fv)),
            sds: inputNumber(round(plot.asce7_16.sds)),
            sd1: inputNumber(round(plot.asce7_16.sd1)),
          })}
        </p>
      )}
      {plot ? <SpectrumPlot points={plot.points} /> : <p className="hint note">{plotError}</p>}
      {error && <p className="error">{error}</p>}
      <div className="buttons">
        <button className="primary" onClick={save}>
          {fn ? t("dlg.apply") : t("dlg.add")}
        </button>
        {fn && <button onClick={remove}>{t("dlg.delete")}</button>}
      </div>
    </div>
  );
}

const round = (v: number) => Number(v.toPrecision(4));

/** Value against period, straight lines between the points and flat beyond them, to 4 s or the last point. */
function SpectrumPlot({ points }: { points: [number, number][] }) {
  const [w, h, pad] = [300, 150, 28];
  const tMax = Math.max(4, points[points.length - 1][0]);
  const vMax = Math.max(...points.map((p) => p[1]), 1e-9) * 1.1;
  const x = (tp: number) => pad + ((w - pad - 8) * tp) / tMax;
  const y = (v: number) => h - pad + 8 - ((h - pad) * v) / vMax;
  const line = [[0, points[0][1]] as [number, number], ...points, [tMax, points[points.length - 1][1]] as [number, number]];
  return (
    <svg className="spectrum-plot" width={w} height={h + 8} role="img" aria-label={t("rs.plot")}>
      <line x1={pad} y1={y(0)} x2={w - 8} y2={y(0)} className="axis" />
      <line x1={pad} y1={y(0)} x2={pad} y2={8} className="axis" />
      <polyline points={line.map(([tp, v]) => `${x(tp)},${y(v)}`).join(" ")} className="curve" fill="none" />
      <text x={w - 8} y={h + 4} textAnchor="end">
        {t("rs.plotAxis", { t: inputNumber(round(tMax)) })}
      </text>
      <text x={pad + 4} y={16}>
        {inputNumber(round(vMax / 1.1))}
      </text>
    </svg>
  );
}

// ---- Response spectrum cases (Define > Load Cases) ----

interface LoadRow {
  dir: SpectrumLoad["dir"];
  fn: string;
  scale: string;
  angle: string;
}

export function SpectrumCaseForm({ spectrumCase, onSaved, typeSelect }: { spectrumCase?: SpectrumCase; onSaved(id: string | null): void; typeSelect?: React.ReactNode }) {
  const model = useStore((s) => s.model);
  const U = useUnits();
  const functions = model.spectrum_functions ?? [];
  const firstFn = functions[0]?.id ?? "";
  const c = spectrumCase;
  const [id, setId] = useState(c?.id ?? "");
  const [rows, setRows] = useState<LoadRow[]>(
    c
      ? c.loads.map((l) => ({ dir: l.dir, fn: l.function, scale: U.show(l.scale, "acceleration"), angle: inputNumber(l.angle ?? 0) }))
      : [{ dir: "X", fn: firstFn, scale: U.show(G, "acceleration"), angle: "0" }],
  );
  const [damping, setDamping] = useState(inputNumber(c?.damping ?? 0.05));
  const [modalRule, setModalRule] = useState<SpectrumCase["modal_combination"]>(c?.modal_combination ?? "cqc");
  const [dirRule, setDirRule] = useState<SpectrumCase["directional_combination"]>(c?.directional_combination ?? "srss");
  const [absFactor, setAbsFactor] = useState(inputNumber(c?.abs_factor ?? 1));
  const [ecc, setEcc] = useState(inputNumber(c?.eccentricity ?? 0));
  const [error, setError] = useState("");
  const setRow = (i: number, patch: Partial<LoadRow>) => setRows(rows.map((r, k) => (k === i ? { ...r, ...patch } : r)));

  const save = () => {
    const loads: SpectrumLoad[] = [];
    for (const r of rows) {
      const scale = U.read(r.scale, "acceleration");
      const angle = parseNumber(r.angle, true);
      if (scale === null || angle === null) return setError(t("err.spectrumScale"));
      loads.push({ dir: r.dir, function: r.fn, scale, ...(angle ? { angle } : {}) });
    }
    const [d, af, e] = [parseNumber(damping), parseNumber(absFactor), parseNumber(ecc, true)];
    if (d === null || af === null || e === null) return setError(t("dlg.enterNumbers"));
    const next: SpectrumCase = { id: id.trim(), loads, damping: d, modal_combination: modalRule, directional_combination: dirRule, abs_factor: af, eccentricity: e };
    if (useStore.getState().edit((m) => setSpectrumCase(m, c?.id ?? null, next))) {
      setError("");
      onSaved(next.id);
    }
  };
  const remove = () => {
    if (c && useStore.getState().edit((m) => deleteSpectrumCase(m, c.id))) onSaved(null);
  };

  return (
    <div className="define-form combo-form">
      <h4>{c ? t("rs.case", { id: c.id }) : t("df.newLoadCase")}</h4>
      {typeSelect}
      <label className="row">
        <span>{t("dlg.name")}</span>
        <input aria-label={t("dlg.name")} value={id} onChange={(e) => setId(e.target.value)} style={{ width: 140 }} />
      </label>
      {functions.length === 0 && <p className="error">{t("rs.noFunctions")}</p>}
      <table className="data combo-rows">
        <thead>
          <tr>
            <th>{t("rs.dir")}</th>
            <th>{t("rs.functionCol")}</th>
            <th>{t("rs.scale", { unit: U.label("acceleration") })}</th>
            <th>{t("rs.angle")}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td>
                <select aria-label={`${t("rs.dir")} ${i + 1}`} value={r.dir} onChange={(e) => setRow(i, { dir: e.target.value as LoadRow["dir"] })}>
                  {SPECTRUM_DIRS.map((d) => (
                    <option key={d} value={d}>
                      {d}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <select aria-label={`${t("rs.functionCol")} ${i + 1}`} value={r.fn} onChange={(e) => setRow(i, { fn: e.target.value })}>
                  {functions.map((f) => (
                    <option key={f.id} value={f.id}>
                      {f.id}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <input aria-label={`${t("rs.scaleShort")} ${i + 1}`} value={r.scale} onChange={(e) => setRow(i, { scale: e.target.value })} style={{ width: 80 }} />
              </td>
              <td>
                <input aria-label={`${t("rs.angle")} ${i + 1}`} value={r.angle} onChange={(e) => setRow(i, { angle: e.target.value })} style={{ width: 50 }} disabled={r.dir === "Z"} />
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
        <button onClick={() => setRows([...rows, { dir: "Y", fn: firstFn, scale: U.show(G, "acceleration"), angle: "0" }])} disabled={!firstFn}>
          {t("df.comboAddRow")}
        </button>
      </div>
      <div className="field-grid">
        <NumberField label={t("rs.damping")} value={damping} onChange={setDamping} width={70} />
        <label className="field">
          <span className="field-label">{t("rs.modalRule")}</span>
          <select aria-label={t("rs.modalRule")} value={modalRule} onChange={(e) => setModalRule(e.target.value as SpectrumCase["modal_combination"])}>
            {MODAL_COMBINATIONS.map((k) => (
              <option key={k} value={k}>
                {t(`rs.rule.${k}` as Key)}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field-label">{t("rs.dirRule")}</span>
          <select aria-label={t("rs.dirRule")} value={dirRule} onChange={(e) => setDirRule(e.target.value as SpectrumCase["directional_combination"])}>
            {DIRECTIONAL_COMBINATIONS.map((k) => (
              <option key={k} value={k}>
                {t(`rs.rule.${k}` as Key)}
              </option>
            ))}
          </select>
        </label>
        {dirRule === "absolute" && <NumberField label={t("rs.absFactor")} value={absFactor} onChange={setAbsFactor} width={70} />}
        <NumberField label={t("rs.eccentricity")} value={ecc} onChange={setEcc} width={70} />
      </div>
      <p className="hint">{t("rs.caseHint", { g: U.show(G, "acceleration"), unit: U.label("acceleration") })}</p>
      {!model.modal && <p className="hint note">{t("rs.modalOn", { n: DEFAULT_MODES })}</p>}
      {error && <p className="error">{error}</p>}
      <div className="buttons">
        <button className="primary" onClick={save}>
          {c ? t("dlg.apply") : t("dlg.add")}
        </button>
        {c && <button onClick={remove}>{t("dlg.delete")}</button>}
      </div>
    </div>
  );
}
