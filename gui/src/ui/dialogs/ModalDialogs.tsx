// Assign > Joint Masses, Define > Mass Source and Modal Case (added mass, mass source and the modal
// case, Eigen or Ritz vectors) and Define > P-Delta Options.

import { useState } from "react";

import { t, tn } from "../../i18n";
import { EditError, massesOf, setJointMasses, setMassSource, setPDelta, type LoadMode } from "../../model/ops";
import { RITZ_ACCELS, type MassSource, type ModalCase, type PDelta, type RitzStart } from "../../model/types";
import { inputNumber } from "../../scene/format";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import type { Quantity } from "../../units/units";
import { Dialog, NumberField, parseNumber } from "../Dialog";
import { ModeRadio } from "./AssignDialogs";

const close = () => useStore.getState().openDialog(null);
const LABELS = ["UX", "UY", "UZ", "RX", "RY", "RZ"];

/** Assign > Joint Masses: six values in global axes, shown for the first joint. */
export function JointMassesDialog() {
  const model = useStore((s) => s.model);
  const nodes = useStore((s) => s.selection.nodes);
  const U = useUnits();
  const massQ = (i: number): Quantity => (i < 3 ? "mass" : "rotationalInertia");
  const [values, setValues] = useState<string[]>(() => (nodes.length ? massesOf(model, nodes[0]) : [0, 0, 0, 0, 0, 0]).map((m, i) => U.show(m, massQ(i))));
  const [mode, setMode] = useState<LoadMode>("replace");
  const [error, setError] = useState("");
  const apply = (ok: boolean) => {
    const v: number[] = [];
    for (const [i, text] of values.entries()) {
      const n = U.read(text, massQ(i), true);
      if (n === null || n < 0) return setError(t("err.massNegative"));
      v.push(n);
    }
    if (useStore.getState().edit((m) => setJointMasses(m, nodes, v, mode)) && ok) close();
  };
  return (
    <Dialog
      title={tn("mass.jointTitle", nodes.length)}
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
      <p className="hint">{t("mass.jointHint")}</p>
      <div className="field-grid">
        {LABELS.map((label, i) => (
          <NumberField
            key={label}
            label={t("mass.dof", { dof: label })}
            value={values[i]}
            onChange={(v) => setValues(values.map((x, k) => (k === i ? v : x)))}
            unit={U.label(massQ(i))}
            width={90}
          />
        ))}
      </div>
      <ModeRadio value={mode} onChange={setMode} noun={t("mass.noun")} />
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

/** Define > Mass Source and Modal Case. */
export function MassSourceDialog() {
  const model = useStore((s) => s.model);
  const src: MassSource = model.mass_source ?? {};
  const [flags, setFlags] = useState({
    elements: src.elements !== false,
    joint_masses: src.joint_masses !== false,
    lateral_only: src.lateral_only !== false,
    lump_at_stories: src.lump_at_stories !== false,
  });
  const [loads, setLoads] = useState<{ pattern: string; scale: string }[]>(() => (src.loads ?? []).map((l) => ({ pattern: l.pattern, scale: inputNumber(l.scale) })));
  const [modal, setModal] = useState(!!model.modal);
  const [modes, setModes] = useState(String(model.modal?.modes ?? 12));
  const [ritz, setRitz] = useState(model.modal?.type === "ritz");
  // Starting loads as "UX", "UY", "UZ" or "P:<pattern>", with their cycles (text; 0 or empty = no limit).
  const [starts, setStarts] = useState<{ load: string; cycles: string }[]>(() =>
    (model.modal?.starts ?? [{ accel: "UX" }, { accel: "UY" }, { accel: "UZ" }]).map((s) => ({
      load: s.accel ?? `P:${s.pattern}`,
      cycles: s.cycles ? String(s.cycles) : "0",
    })),
  );
  const startOptions: [string, string][] = [
    ...RITZ_ACCELS.map((d): [string, string] => [d, t("mass.accel", { d })]),
    ...model.load_patterns.map((p): [string, string] => [`P:${p.id}`, t("mass.patternStart", { id: p.id })]),
  ];
  const move = (i: number, by: number) => {
    const j = i + by;
    if (j < 0 || j >= starts.length) return;
    const next = [...starts];
    [next[i], next[j]] = [next[j], next[i]];
    setStarts(next);
  };
  const flagLabels: [keyof typeof flags, string][] = [
    ["elements", t("mass.elements")],
    ["joint_masses", t("mass.joints")],
    ["lateral_only", t("mass.lateral")],
    ["lump_at_stories", t("mass.lump")],
  ];
  const apply = (ok: boolean) => {
    const done = useStore.getState().edit((m) => {
      const list = loads.map((l) => {
        const s = parseNumber(l.scale);
        if (s === null) throw new EditError(t("err.massScale"));
        return { pattern: l.pattern, scale: s };
      });
      const n = parseNumber(modes);
      let mc: ModalCase | null = null;
      if (modal) {
        mc = { id: m.modal?.id ?? "Modal", modes: n ?? NaN };
        if (ritz) {
          mc.type = "ritz";
          mc.starts = starts.map((s): RitzStart => {
            const c = s.cycles.trim() === "" ? 0 : (parseNumber(s.cycles) ?? NaN);
            const st: RitzStart = s.load.startsWith("P:") ? { pattern: s.load.slice(2) } : { accel: s.load as RitzStart["accel"] };
            if (c !== 0) st.cycles = c;
            return st;
          });
        }
      }
      return setMassSource(m, { ...flags, loads: list }, mc);
    });
    if (done && ok) close();
  };
  return (
    <Dialog
      title={t("mass.title")}
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
      <h4>{t("mass.source")}</h4>
      {flagLabels.map(([k, label]) => (
        <label key={k} className="check">
          <input type="checkbox" checked={flags[k]} onChange={(e) => setFlags({ ...flags, [k]: e.target.checked })} /> {label}
        </label>
      ))}
      <h4>{t("mass.fromPatterns")}</h4>
      {loads.map((l, i) => (
        <div key={i} className="row">
          <select
            aria-label={t("mass.pattern", { n: i + 1 })}
            value={l.pattern}
            onChange={(e) => setLoads(loads.map((x, k) => (k === i ? { ...x, pattern: e.target.value } : x)))}
          >
            {model.load_patterns.map((c) => (
              <option key={c.id}>{c.id}</option>
            ))}
          </select>
          <NumberField label={t("mass.scale", { n: i + 1 })} value={l.scale} onChange={(v) => setLoads(loads.map((x, k) => (k === i ? { ...x, scale: v } : x)))} width={70} />
          <button onClick={() => setLoads(loads.filter((_, k) => k !== i))}>{t("dlg.delete")}</button>
        </div>
      ))}
      <button disabled={!model.load_patterns.length} onClick={() => setLoads([...loads, { pattern: model.load_patterns[0]?.id ?? "", scale: "1" }])}>
        {t("mass.addPattern")}
      </button>
      <h4>{t("mass.modal")}</h4>
      <label className="check">
        <input type="checkbox" checked={modal} onChange={(e) => setModal(e.target.checked)} /> {t("mass.runModal")}
      </label>
      {modal && (
        <>
          <div className="radio-list">
            <label>
              <input type="radio" name="modaltype" checked={!ritz} onChange={() => setRitz(false)} /> {t("mass.eigen")}
            </label>
            <label>
              <input type="radio" name="modaltype" checked={ritz} onChange={() => setRitz(true)} /> {t("mass.ritz")}
            </label>
          </div>
          <NumberField label={t("mass.modes")} value={modes} onChange={setModes} width={70} />
          {ritz && (
            <>
              <h4>{t("mass.starts")}</h4>
              {starts.map((s, i) => (
                <div key={i} className="row">
                  <select aria-label={t("mass.start", { n: i + 1 })} value={s.load} onChange={(e) => setStarts(starts.map((x, k) => (k === i ? { ...x, load: e.target.value } : x)))}>
                    {startOptions.map(([v, label]) => (
                      <option key={v} value={v}>
                        {label}
                      </option>
                    ))}
                  </select>
                  <NumberField label={t("mass.cycles", { n: i + 1 })} value={s.cycles} onChange={(v) => setStarts(starts.map((x, k) => (k === i ? { ...x, cycles: v } : x)))} width={50} />
                  <button aria-label={t("mass.up", { n: i + 1 })} disabled={i === 0} onClick={() => move(i, -1)}>
                    ↑
                  </button>
                  <button aria-label={t("mass.down", { n: i + 1 })} disabled={i === starts.length - 1} onClick={() => move(i, 1)}>
                    ↓
                  </button>
                  <button onClick={() => setStarts(starts.filter((_, k) => k !== i))}>{t("dlg.delete")}</button>
                </div>
              ))}
              <button onClick={() => setStarts([...starts, { load: "UX", cycles: "0" }])}>{t("mass.addStart")}</button>
              <p className="hint">{t("mass.ritzHint")}</p>
            </>
          )}
        </>
      )}
      <p className="hint">{t("mass.hint")}</p>
    </Dialog>
  );
}

/** Define > P-Delta Options: none, from the mass, or iterative from a load (load patterns and factors). */
export function PDeltaDialog() {
  const model = useStore((s) => s.model);
  const [method, setMethod] = useState<PDelta["method"]>(model.p_delta?.method ?? "none");
  const [loads, setLoads] = useState<{ pattern: string; scale: string }[]>(() => {
    const rows = (model.p_delta?.loads ?? []).map((l) => ({ pattern: l.pattern, scale: inputNumber(l.scale) }));
    return rows.length || !model.load_patterns.length ? rows : [{ pattern: model.load_patterns[0].id, scale: "1" }];
  });
  const methods: [PDelta["method"], string][] = [
    ["none", t("pd.none")],
    ["mass", t("pd.mass")],
    ["loads", t("pd.loads")],
  ];
  const apply = (ok: boolean) => {
    const done = useStore.getState().edit((m) => {
      const list =
        method === "loads"
          ? loads.map((l) => {
              const s = parseNumber(l.scale);
              if (s === null) throw new EditError(t("err.pDeltaScale"));
              return { pattern: l.pattern, scale: s };
            })
          : [];
      return setPDelta(m, { method, loads: list });
    });
    if (done && ok) close();
  };
  return (
    <Dialog
      title={t("pd.title")}
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
      <h4>{t("pd.method")}</h4>
      <div className="radio-list">
        {methods.map(([k, label]) => (
          <label key={k}>
            <input type="radio" name="p-delta-method" checked={method === k} onChange={() => setMethod(k)} /> {label}
          </label>
        ))}
      </div>
      {method === "loads" && (
        <>
          <h4>{t("pd.load")}</h4>
          {loads.map((l, i) => (
            <div key={i} className="row">
              <select
                aria-label={t("pd.pattern", { n: i + 1 })}
                value={l.pattern}
                onChange={(e) => setLoads(loads.map((x, k) => (k === i ? { ...x, pattern: e.target.value } : x)))}
              >
                {model.load_patterns.map((c) => (
                  <option key={c.id}>{c.id}</option>
                ))}
              </select>
              <NumberField label={t("pd.scale", { n: i + 1 })} value={l.scale} onChange={(v) => setLoads(loads.map((x, k) => (k === i ? { ...x, scale: v } : x)))} width={70} />
              <button onClick={() => setLoads(loads.filter((_, k) => k !== i))}>{t("dlg.delete")}</button>
            </div>
          ))}
          <button disabled={!model.load_patterns.length} onClick={() => setLoads([...loads, { pattern: model.load_patterns[0]?.id ?? "", scale: "1" }])}>
            {t("mass.addPattern")}
          </button>
        </>
      )}
      <p className="hint">{t("pd.hint")}</p>
    </Dialog>
  );
}
