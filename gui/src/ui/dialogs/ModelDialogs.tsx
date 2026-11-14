// File, grid and joint dialogs: New Model, Edit Grid, Joint Coordinates, Check Model, About.

import { useState, type MouseEvent } from "react";

import { LANGUAGES, t, type Key, type Lang } from "../../i18n";

import { createModel, deleteCaseResults, moveJoint, runAnalysis } from "../../actions";
import { checkModel } from "../../model/check";
import { letters, makeGrid, parseSpacings, round, spacingText } from "../../model/grid";
import { addNode, combosNotRun, finishedCases, gridOf, nodeById, runCaseIds, setDimension, setGrid } from "../../model/ops";
import type { Dimension, Grid } from "../../model/types";
import { DEFAULT_PREFS, usePrefs, type ThemeChoice } from "../../state/prefs";
import { useStore } from "../../state/store";
import { unitFormat, useUnits } from "../../units/format";
import { UNIT_SYSTEMS, unitSystemOf, type UnitSystem } from "../../units/units";
import { Dialog, NumberField, parseNumber } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

export function NewModelDialog() {
  const [units, setUnits] = useState<UnitSystem>(() => unitSystemOf(useStore.getState().model.gui?.units));
  const [xs, setXs] = useState("6");
  const [ys, setYs] = useState("6");
  const [zs, setZs] = useState("4");
  const [error, setError] = useState("");
  const U = unitFormat(units);
  const ok = async () => {
    // Spacings are typed in the chosen display units; the model keeps metres.
    const inBase = (v: number[] | null) => v && v.map((s) => U.read(String(s), "distance") ?? NaN);
    const x = inBase(parseSpacings(xs));
    const y = inBase(parseSpacings(ys));
    const z = inBase(parseSpacings(zs));
    if (!x || !y || !z) return setError(t("md2.spacingsError"));
    if (await createModel({ xSpacings: x, ySpacings: y, storyHeights: z, units })) close();
  };
  return (
    <Dialog
      title={t("md2.newModel")}
      buttons={
        <>
          <button className="primary" onClick={ok}>
            {t("dlg.ok")}
          </button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <UnitSystemSelect value={units} onChange={setUnits} />
      <p className="hint">{t("md2.newHint", { unit: U.label("distance") })}</p>
      <NumberField label={t("md2.xSpacings")} value={xs} onChange={setXs} width={160} />
      <NumberField label={t("md2.ySpacings")} value={ys} onChange={setYs} width={160} />
      <NumberField label={t("md2.storyHeights")} value={zs} onChange={setZs} width={160} />
      <p className="hint">{t("md2.newHint2")}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

/** The display unit system: SI, MKS or US. */
export function UnitSystemSelect(props: { value: UnitSystem; onChange(v: UnitSystem): void }) {
  return (
    <label className="row">
      <span>{t("units.label")}</span>
      <select aria-label={t("units.label")} value={props.value} onChange={(e) => props.onChange(e.target.value as UnitSystem)}>
        {UNIT_SYSTEMS.map((s) => (
          <option key={s} value={s}>
            {t(`units.${s}` as Key)}
          </option>
        ))}
      </select>
    </label>
  );
}

function coordsText(v: number[], U: ReturnType<typeof useUnits>) {
  return v.map((x) => U.show(x, "distance")).join(", ");
}

function parseCoords(text: string, U: ReturnType<typeof useUnits>): number[] | null {
  const parts = text.split(/[,;\s]+/).filter(Boolean);
  const v = parts.map((p) => U.read(p, "distance") ?? NaN);
  if (!v.length || v.some((x) => !Number.isFinite(x))) return null;
  return [...new Set(v.map(round))].sort((a, b) => a - b);
}

export function GridDialog() {
  const model = useStore((s) => s.model);
  const g = gridOf(model);
  const U = useUnits();
  const [xs, setXs] = useState(coordsText(g.x.map((l) => l.coord), U));
  const [ys, setYs] = useState(coordsText(g.y.map((l) => l.coord), U));
  const [error, setError] = useState("");
  const ok = () => {
    const x = parseCoords(xs, U);
    const y = parseCoords(ys, U);
    if (!x || !y) return setError(t("md2.gridError"));
    const grid: Grid = makeGrid([], [], []);
    grid.x = x.map((coord, i) => ({ label: letters(i), coord }));
    grid.y = y.map((coord, i) => ({ label: String(i + 1), coord }));
    // Story levels stay (Define > Story Data edits them).
    grid.stories = g.stories.map((s) => ({ ...s }));
    if (useStore.getState().edit((m) => setGrid(m, grid))) close();
  };
  return (
    <Dialog
      title={t("md2.editGrid")}
      buttons={
        <>
          <button className="primary" onClick={ok}>
            {t("dlg.ok")}
          </button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <p className="hint">
        {t("md2.gridHint", {
          unit: U.label("distance"),
          x: spacingText(g.x.map((l) => U.value(l.coord, "distance"))) || "-",
        })}
      </p>
      <NumberField label={t("md2.xLines")} value={xs} onChange={setXs} width={220} />
      <NumberField label={t("md2.yLines")} value={ys} onChange={setYs} width={220} />
      <p className="hint">{t("md2.gridHint2")}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

export function JointCoordsDialog() {
  const model = useStore((s) => s.model);
  const sel = useStore((s) => s.selection);
  const one = sel.nodes.length === 1 ? nodeById(model, sel.nodes[0]) : undefined;
  const U = useUnits();
  const [x, setX] = useState(U.show(one?.x ?? 0, "distance"));
  const [y, setY] = useState(U.show(one?.y ?? 0, "distance"));
  const [z, setZ] = useState(U.show(one?.z ?? 0, "distance"));
  const [error, setError] = useState("");
  const point = () => {
    const p = [U.read(x, "distance", true), U.read(y, "distance", true), U.read(z, "distance", true)];
    if (p.some((v) => v === null)) {
      setError(t("md2.coordsError"));
      return null;
    }
    return p as [number, number, number];
  };
  const add = () => {
    const p = point();
    if (!p) return;
    let id = "";
    const ok = useStore.getState().edit((m) => {
      const r = addNode(m, p, { free: true });
      id = r.id;
      return r.model;
    });
    if (ok) {
      useStore.getState().notify(t("md2.jointAdded", { id, p: p.map((v) => U.fmt(v, "distance")).join(", ") }));
      setError("");
    }
  };
  const move = async () => {
    const p = point();
    if (!p || !one) return;
    await moveJoint(one.id, p);
    close();
  };
  return (
    <Dialog
      title={t("md2.jointCoords")}
      buttons={
        <>
          <button className="primary" onClick={add}>
            {t("md2.addJoint")}
          </button>
          {one && <button onClick={move}>{t("md2.moveJoint", { id: one.id })}</button>}
          <button onClick={close}>{t("dlg.close")}</button>
        </>
      }
    >
      <p className="hint">
        {t("md2.coordsHint", { unit: U.label("distance") })}
        {one ? t("md2.coordsHintSelected", { id: one.id }) : ""}
      </p>
      <div className="field-row">
        <NumberField label="X" value={x} onChange={setX} width={90} />
        <NumberField label="Y" value={y} onChange={setY} width={90} />
        <NumberField label="Z" value={z} onChange={setZ} width={90} />
      </div>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

export function CheckDialog() {
  const model = useStore((s) => s.model);
  const issues = checkModel(model);
  const errors = issues.filter((i) => i.level === "error");
  const warnings = issues.filter((i) => i.level === "warning");
  return (
    <Dialog
      title={t("md2.checkModel")}
      buttons={
        <>
          {errors.length === 0 && (
            <button
              className="primary"
              onClick={() => {
                close();
                void runAnalysis(true);
              }}
            >
              {warnings.length ? t("md2.runAnyway") : t("md2.runAnalysis")}
            </button>
          )}
          <button onClick={close}>{t("dlg.close")}</button>
        </>
      }
    >
      {issues.length === 0 ? (
        <p>{t("md2.noProblems")}</p>
      ) : (
        <ul className="issues">
          {issues.map((i, k) => (
            <li key={k} className={i.level}>
              <b>{i.level === "error" ? t("md2.error") : i.level === "warning" ? t("md2.warning") : t("md2.note")}:</b> {i.message}
            </li>
          ))}
        </ul>
      )}
      {issues.some((i) => i.nodes?.length || i.frames?.length) && <p className="hint note">{t("md2.shownRed")}</p>}
    </Dialog>
  );
}

export function AboutDialog() {
  return (
    <Dialog title={t("md2.about")} buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <p>{t("md2.aboutLine1", { name: "OSAB" })}</p>
      <p>{t("md2.aboutLine2")}</p>
      <p>{t("md2.aboutWarning")}</p>
      <p className="hint note">{t("md2.aboutLicense")}</p>
    </Dialog>
  );
}

export function AnalysisOptionsDialog() {
  const model = useStore((s) => s.model);
  const [dim, setDim] = useState<Dimension>(model.dimension);
  const ok = () => {
    if (dim === model.dimension || useStore.getState().edit((m) => setDimension(m, dim))) close();
  };
  return (
    <Dialog
      title={t("md2.analysisOptions")}
      buttons={
        <>
          <button className="primary" onClick={ok}>
            {t("dlg.ok")}
          </button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <p>{t("md2.activeDof")}</p>
      <div className="radio-list">
        {(
          [
            ["3D", "md2.space"],
            ["2D", "md2.plane"],
            ["grid", "md2.grid"],
          ] as [Dimension, Key][]
        ).map(([d, label]) => (
          <label key={d}>
            <input type="radio" name="dof" checked={dim === d} onChange={() => setDim(d)} /> {t(label)}
          </label>
        ))}
      </div>
      <p className="hint">{t("md2.planeHint")}</p>
      <p className="hint">{t("md2.planeGridHint")}</p>
    </Dialog>
  );
}

/**
 * Analyze > Load Cases to Run: which cases a run includes, each case's status, and deleting a case's
 * results. Rows are selected by clicking (Ctrl adds, Shift takes a range); the buttons act on them.
 * OK keeps the settings, Run Now keeps them and runs; deleted results are gone either way.
 */
export function RunCasesDialog() {
  const model = useStore((s) => s.model);
  const results = useStore((s) => s.results);
  const ids = runCaseIds(model);
  const done = finishedCases(model, results);
  const [off, setOff] = useState<string[]>(() => (model.not_run ?? []).filter((id) => ids.includes(id)));
  const [picked, setPicked] = useState<string[]>([]);
  const [anchor, setAnchor] = useState<string | null>(null);
  const type = (id: string) =>
    id === model.modal?.id ? t("lc.type.modal") : model.spectrum_cases?.some((c) => c.id === id) ? t("lc.type.spectrum") : t("lc.type.static");
  const toggle = (id: string, run: boolean) => setOff((o) => (run ? o.filter((x) => x !== id) : [...o, id]));
  /** All of `list` to run, or (when all of them run) none of them. */
  const flip = (list: string[]) => setOff((o) => (list.some((id) => o.includes(id)) ? o.filter((id) => !list.includes(id)) : [...new Set([...o, ...list])]));
  const pick = (id: string, e: MouseEvent) => {
    if (e.shiftKey && anchor) {
      const [i, j] = [ids.indexOf(anchor), ids.indexOf(id)].sort((x, y) => x - y);
      const range = ids.slice(i, j + 1);
      setPicked((p) => (e.ctrlKey || e.metaKey ? [...new Set([...p, ...range])] : range));
      return;
    }
    setAnchor(id);
    setPicked((p) => (e.ctrlKey || e.metaKey ? (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]) : [id]));
  };
  const spectrumRuns = (model.spectrum_cases ?? []).some((c) => !off.includes(c.id));
  const modalAnyway = model.modal !== undefined && off.includes(model.modal.id) && spectrumRuns;
  const combos = combosNotRun(model, off);
  const keep = () => useStore.getState().setRunFlags(off);
  const ok = () => {
    if (keep()) close();
  };
  const runNow = () => {
    if (!keep()) return;
    close();
    void runAnalysis();
  };
  return (
    <Dialog
      title={t("rc.title")}
      buttons={
        <>
          <button onClick={runNow}>{t("rc.runNow")}</button>
          <button className="primary" onClick={ok}>
            {t("dlg.ok")}
          </button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <table className="data combo-rows run-cases">
        <thead>
          <tr>
            <th>{t("rc.case")}</th>
            <th>{t("rc.type")}</th>
            <th>{t("rc.status")}</th>
            <th>{t("rc.run")}</th>
          </tr>
        </thead>
        <tbody>
          {ids.map((id) => (
            <tr key={id} aria-selected={picked.includes(id)} className={picked.includes(id) ? "picked" : undefined} onClick={(e) => pick(id, e)}>
              <td>{id}</td>
              <td>{type(id)}</td>
              <td>{done.has(id) ? t("rc.finished") : t("rc.notRun")}</td>
              <td>
                <input
                  type="checkbox"
                  aria-label={t("rc.runCase", { id })}
                  checked={!off.includes(id)}
                  onClick={(e) => e.stopPropagation()}
                  onChange={(e) => toggle(id, e.target.checked)}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="row">
        <button disabled={!picked.length} onClick={() => flip(picked)}>
          {t("rc.flipPicked")}
        </button>
        <button disabled={!picked.some((id) => done.has(id))} onClick={() => void deleteCaseResults(picked)}>
          {t("rc.deletePicked")}
        </button>
        <button onClick={() => flip(ids)}>{t("rc.flipAll")}</button>
        <button disabled={!done.size} onClick={() => void deleteCaseResults(ids)}>
          {t("rc.deleteAll")}
        </button>
      </div>
      <p className="hint">{t("rc.pickHint")}</p>
      {modalAnyway && <p className="hint note">{t("rc.modalAnyway", { id: model.modal!.id })}</p>}
      {combos.length > 0 && <p className="hint note">{t("rc.combos", { list: combos.join(", ") })}</p>}
    </Dialog>
  );
}

export function PreferencesDialog() {
  const snapMm = usePrefs((p) => p.snapMm);
  const [text, setText] = useState(String(snapMm));
  const [lang, setLang] = useState<Lang>(usePrefs.getState().lang);
  const [theme, setTheme] = useState<ThemeChoice>(usePrefs.getState().theme);
  const [units, setUnits] = useState<UnitSystem>(() => unitSystemOf(useStore.getState().model.gui?.units));
  const [error, setError] = useState("");
  const ok = () => {
    const v = parseNumber(text);
    if (v === null || !(v > 0)) return setError(t("md2.snapError"));
    usePrefs.getState().set({ snapMm: v, lang, theme });
    useStore.getState().setUnitSystem(units);
    close();
  };
  return (
    <Dialog
      title={t("md2.preferences")}
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
        <span>{t("md2.language")}</span>
        <select aria-label={t("md2.language")} value={lang} onChange={(e) => setLang(e.target.value as Lang)}>
          {LANGUAGES.map((l) => (
            <option key={l.code} value={l.code}>
              {l.name}
            </option>
          ))}
        </select>
      </label>
      <p className="hint">{t("md2.languageHint")}</p>
      <label className="row">
        <span>{t("md2.theme")}</span>
        <select aria-label={t("md2.theme")} value={theme} onChange={(e) => setTheme(e.target.value as ThemeChoice)}>
          <option value="system">{t("md2.themeSystem")}</option>
          <option value="light">{t("md2.themeLight")}</option>
          <option value="dark">{t("md2.themeDark")}</option>
        </select>
      </label>
      <UnitSystemSelect value={units} onChange={setUnits} />
      <p className="hint">{t("units.hint")}</p>
      <NumberField label={t("md2.snap")} value={text} onChange={setText} unit="mm" width={90} />
      <p className="hint">{t("md2.snapHint", { mm: DEFAULT_PREFS.snapMm })}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}
