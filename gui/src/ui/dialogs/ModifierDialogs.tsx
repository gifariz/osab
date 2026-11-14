// Property modifiers: Assign > Frame Property Modifiers and Assign > Area Property Modifiers for
// the selected objects, and the Property Modifiers window of the Frame Sections and Area Sections forms.

import { useState } from "react";

import { t, tn, type Key } from "../../i18n";
import { setAreaModifiers } from "../../model/areaOps";
import { setFrameModifiers } from "../../model/ops";
import { AREA_MODIFIERS, FRAME_MODIFIERS, type AreaModifiers, type FrameModifiers } from "../../model/types";
import { inputNumber } from "../../scene/format";
import { useStore } from "../../state/store";
import { Dialog, NumberField, parseNumber } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

type Mods = Record<string, number | undefined>;

/** Rows of related modifiers. */
const ROWS: Record<number, string[][]> = {
  8: [
    ["A", "As2", "As3"],
    ["J", "I22", "I33"],
    ["mass", "weight"],
  ],
  10: [
    ["f11", "f22", "f12"],
    ["m11", "m22", "m12"],
    ["v13", "v23"],
    ["mass", "weight"],
  ],
};

/** One labelled box per modifier, a row per group. */
export function ModifierFields({ names, values, onChange }: { names: readonly string[]; values: Record<string, string>; onChange(v: Record<string, string>): void }) {
  const rows = ROWS[names.length] ?? [names as string[]];
  return (
    <div className="field-grid modifier-grid">
      {rows.flatMap((row) =>
        [0, 1, 2].map((c) =>
          row[c] ? (
            <NumberField key={row[c]} label={modifierLabel(row[c])} title={t(`mod.${row[c]}` as Key)} value={values[row[c]] ?? ""} onChange={(v) => onChange({ ...values, [row[c]]: v })} width={70} />
          ) : (
            <span key={`${row[0]}-${c}`} />
          ),
        ),
      )}
    </div>
  );
}

/** Box texts of a set of modifiers (absent = 1). */
export function modifierTexts(names: readonly string[], mods: Mods | undefined): Record<string, string> {
  return Object.fromEntries(names.map((k) => [k, inputNumber(mods?.[k] ?? 1)]));
}

/** The modifiers typed in the boxes; blank boxes are undefined ("keep") when `blankKeeps`, else refused. */
export function readModifierTexts(names: readonly string[], texts: Record<string, string>, blankKeeps: boolean): Mods | null {
  const out: Mods = {};
  for (const k of names) {
    const text = texts[k] ?? "";
    if (text.trim() === "" && blankKeeps) continue;
    const v = parseNumber(text);
    if (v === null) return null;
    out[k] = v;
  }
  return out;
}

/** Short text of a section's modifiers: the ones not 1, or "none". */
/** The factors that are not 1 ("m11 0.25, mass 0"); empty when all are 1. */
export function modifierSummary(names: readonly string[], values: Record<string, string>): string {
  return names
    .filter((k) => parseNumber(values[k] ?? "") !== 1)
    .map((k) => `${k} ${values[k]?.trim() || "?"}`)
    .join(", ");
}

/** A modifier's label: its usual symbol (f11, I33...); mass and weight in words. The long name is the tooltip. */
function modifierLabel(k: string): string {
  return k === "mass" || k === "weight" ? t(`mod.${k}`) : k;
}

/**
 * The section part of the Frame Sections and Area Sections forms: a button that opens the Property
 * Modifiers window and a line with the factors that are not 1. The window's OK hands the values back to
 * the form; the form's Apply or Add saves them with the rest of the section.
 */
export function SectionModifiers(props: { names: readonly string[]; values: Record<string, string>; onChange(v: Record<string, string>): void; stiff?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="section-modifiers">
      <button onClick={() => setOpen(true)} disabled={props.stiff}>
        {t("mod.open")}
      </button>
      <span className="hint note">{props.stiff ? t("mod.stiffHint") : modifierSummary(props.names, props.values)}</span>
      {open && (
        <SectionModifiersWindow
          names={props.names}
          initial={props.values}
          onOk={(v) => {
            props.onChange(v);
            setOpen(false);
          }}
          onClose={() => setOpen(false)}
        />
      )}
    </div>
  );
}

function SectionModifiersWindow(props: { names: readonly string[]; initial: Record<string, string>; onOk(v: Record<string, string>): void; onClose(): void }) {
  const [values, setValues] = useState(props.initial);
  const [error, setError] = useState("");
  const ok = () => {
    if (!readModifierTexts(props.names, values, false)) return setError(t("mod.enterAll"));
    props.onOk(values);
  };
  return (
    <Dialog
      title={t("mod.sectionTitle")}
      onClose={props.onClose}
      buttons={
        <>
          <button onClick={() => setValues(Object.fromEntries(props.names.map((k) => [k, "1"])))}>{t("mod.reset")}</button>
          <button className="primary" onClick={ok}>
            {t("dlg.ok")}
          </button>
          <button onClick={props.onClose}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <ModifierFields names={props.names} values={values} onChange={setValues} />
      <p className="hint">{t("mod.sectionHint")}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

/** The boxes start at the selection's common value, blank where the objects differ. */
function common(names: readonly string[], list: (Mods | undefined)[]): Record<string, string> {
  return Object.fromEntries(
    names.map((k) => {
      const vals = new Set(list.map((m) => m?.[k] ?? 1));
      return [k, vals.size === 1 ? inputNumber([...vals][0]) : ""];
    }),
  );
}

function ObjectModifiersDialog(props: {
  title: string;
  hint: string;
  names: readonly string[];
  initial: Record<string, string>;
  apply(values: Mods): boolean;
}) {
  const [values, setValues] = useState(props.initial);
  const [error, setError] = useState("");
  const apply = (ok: boolean) => {
    const v = readModifierTexts(props.names, values, true);
    if (!v) return setError(t("mod.enterNumbers"));
    setError("");
    if (props.apply(v) && ok) close();
  };
  return (
    <Dialog
      title={props.title}
      buttons={
        <>
          <button onClick={() => setValues(Object.fromEntries(props.names.map((k) => [k, "1"])))}>{t("mod.reset")}</button>
          <button className="primary" onClick={() => apply(true)}>
            {t("dlg.ok")}
          </button>
          <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <ModifierFields names={props.names} values={values} onChange={setValues} />
      <p className="hint">{props.hint}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

export function FrameModifiersDialog() {
  const model = useStore((s) => s.model);
  const frames = useStore((s) => s.selection.frames);
  const own = frames.map((id) => model.frames.find((e) => e.id === id)?.modifiers as Mods | undefined);
  return (
    <ObjectModifiersDialog
      title={tn("mod.frameTitle", frames.length)}
      hint={t("mod.frameHint")}
      names={FRAME_MODIFIERS}
      initial={common(FRAME_MODIFIERS, own)}
      apply={(v) => useStore.getState().edit((m) => setFrameModifiers(m, frames, v as FrameModifiers))}
    />
  );
}

export function AreaModifiersDialog() {
  const model = useStore((s) => s.model);
  const areas = useStore((s) => s.selection.areas);
  const own = areas.map((id) => model.areas.find((a) => a.id === id)?.modifiers as Mods | undefined);
  return (
    <ObjectModifiersDialog
      title={tn("mod.areaTitle", areas.length)}
      hint={t("mod.areaHint")}
      names={AREA_MODIFIERS}
      initial={common(AREA_MODIFIERS, own)}
      apply={(v) => useStore.getState().edit((m) => setAreaModifiers(m, areas, v as AreaModifiers))}
    />
  );
}
