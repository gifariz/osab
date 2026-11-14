// Define > Load Combinations: name, type (Linear Add, Envelope or SRSS) and rows of
// load case or combination with a scale factor.

import { useState } from "react";

import { t, type Key } from "../../i18n";
import { copyDefinition, deleteCombination, upsertCombination } from "../../model/ops";
import { COMBO_TYPES, type ComboItem, type ComboType, type LoadCombination } from "../../model/types";
import { inputNumber } from "../../scene/format";
import { useStore } from "../../state/store";
import { Dialog, parseNumber } from "../Dialog";
import { ListPicker, initialItem } from "./DefineDialogs";

const close = () => useStore.getState().openDialog(null);

export function CombinationsDialog() {
  const model = useStore((s) => s.model);
  const [selected, setSelected] = useState<string | null>(() => initialItem(model.load_combinations));
  const current = model.load_combinations.find((c) => c.id === selected);
  return (
    <Dialog title={t("df.combos")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("df.combosList")} items={model.load_combinations.map((c) => c.id)} selected={selected} onSelect={setSelected} />
        <ComboForm key={selected ?? "(new)"} combo={current} onSaved={setSelected} />
      </div>
    </Dialog>
  );
}

/** A row being edited: the part as "case:ID" or "combo:ID", and the scale factor as typed. */
interface Row {
  part: string;
  scale: string;
}

const rowOf = (it: ComboItem): Row => ({ part: it.case !== undefined ? `case:${it.case}` : `combo:${it.combo}`, scale: inputNumber(it.scale) });

function ComboForm({ combo, onSaved }: { combo?: LoadCombination; onSaved(id: string | null): void }) {
  const model = useStore((s) => s.model);
  const firstCase = model.load_cases[0]?.id ?? model.spectrum_cases?.[0]?.id;
  const [id, setId] = useState(combo?.id ?? "");
  const [type, setType] = useState<ComboType>(combo?.type ?? "linear-add");
  const [rows, setRows] = useState<Row[]>(combo ? combo.items.map(rowOf) : firstCase ? [{ part: `case:${firstCase}`, scale: "1" }] : []);
  const [error, setError] = useState("");
  const others = model.load_combinations.filter((c) => c.id !== combo?.id);

  const save = () => {
    const items: ComboItem[] = [];
    for (const r of rows) {
      const s = parseNumber(r.scale);
      if (s === null) return setError(t("err.comboScale"));
      const [kind, ...rest] = r.part.split(":");
      const name = rest.join(":");
      items.push(kind === "combo" ? { combo: name, scale: s } : { case: name, scale: s });
    }
    const next: LoadCombination = { id: id.trim(), type, items };
    if (useStore.getState().edit((m) => upsertCombination(m, combo?.id ?? null, next))) {
      setError("");
      onSaved(next.id);
    }
  };
  const remove = () => {
    if (combo && useStore.getState().edit((m) => deleteCombination(m, combo.id))) onSaved(null);
  };
  const copy = () => {
    if (!combo) return;
    let copied = "";
    const ok = useStore.getState().edit((m) => {
      const r = copyDefinition(m, "combo", combo.id);
      copied = r.id;
      return r.model;
    });
    if (ok) onSaved(copied);
  };
  const setRow = (i: number, patch: Partial<Row>) => setRows(rows.map((r, k) => (k === i ? { ...r, ...patch } : r)));

  return (
    <div className="define-form combo-form">
      <h4>{combo ? t("df.combo", { id: combo.id }) : t("df.newCombo")}</h4>
      <label className="row">
        <span>{t("dlg.name")}</span>
        <input aria-label={t("dlg.name")} value={id} onChange={(e) => setId(e.target.value)} style={{ width: 140 }} />
      </label>
      <label className="row">
        <span>{t("df.comboType")}</span>
        <select aria-label={t("df.comboType")} value={type} onChange={(e) => setType(e.target.value as ComboType)}>
          {COMBO_TYPES.map((k) => (
            <option key={k} value={k}>
              {t(`df.combo.${k}` as Key)}
            </option>
          ))}
        </select>
      </label>
      <table className="data combo-rows">
        <thead>
          <tr>
            <th>{t("df.comboPart")}</th>
            <th>{t("df.comboScale")}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td>
                <select aria-label={`${t("df.comboPart")} ${i + 1}`} value={r.part} onChange={(e) => setRow(i, { part: e.target.value })}>
                  <optgroup label={t("df.comboCases")}>
                    {[...model.load_cases, ...(model.spectrum_cases ?? [])].map((c) => (
                      <option key={c.id} value={`case:${c.id}`}>
                        {c.id}
                      </option>
                    ))}
                  </optgroup>
                  {others.length > 0 && (
                    <optgroup label={t("df.comboCombos")}>
                      {others.map((c) => (
                        <option key={c.id} value={`combo:${c.id}`}>
                          {c.id}
                        </option>
                      ))}
                    </optgroup>
                  )}
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
        <button onClick={() => setRows([...rows, { part: firstCase ? `case:${firstCase}` : "", scale: "1" }])} disabled={!firstCase}>
          {t("df.comboAddRow")}
        </button>
      </div>
      <p className="hint">{t("df.combosHint")}</p>
      {error && <p className="error">{error}</p>}
      <div className="buttons">
        <button className="primary" onClick={save}>
          {combo ? t("dlg.apply") : t("dlg.add")}
        </button>
        {combo && <button onClick={copy}>{t("ex.copy")}</button>}
        {combo && <button onClick={remove}>{t("dlg.delete")}</button>}
      </div>
    </div>
  );
}
