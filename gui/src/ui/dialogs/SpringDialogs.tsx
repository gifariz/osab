// Define > Spring Properties (line springs, area springs) and Assign > Frame > Line Springs / Assign >
// Shell > Area Springs (model/springs.ts).

import { useState } from "react";

import { t, tn, type Key } from "../../i18n";
import { EditError } from "../../model/ops";
import { SPRING_KEYS, assignAreaSpring, assignLineSpring, deleteSpringProp, springUsers, upsertSpringProp, type SpringKind } from "../../model/springs";
import type { Id } from "../../model/types";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import type { Quantity } from "../../units/units";
import { Dialog, NumberField } from "../Dialog";
import { ListPicker } from "./DefineDialogs";

const close = () => useStore.getState().openDialog(null);

/** The unit of each stiffness of a spring property. */
const quantity = (kind: SpringKind, key: string): Quantity => (kind === "area" ? "areaSpring" : key === "r1" ? "lineRotSpring" : "lineSpring");

/** Define > Spring Properties > Line Springs or Area Springs: add, change, rename and delete properties. */
export function SpringPropsDialog({ kind }: { kind: SpringKind }) {
  const model = useStore((s) => s.model);
  const edit = useStore((s) => s.edit);
  const U = useUnits();
  const list = (kind === "line" ? model.line_spring_props : model.area_spring_props) ?? [];
  const ids = list.map((p) => p.id);
  const item = useStore((s) => s.dialogItem);
  const [selected, setSelected] = useState<Id | null>(item && ids.includes(item) ? item : (ids[0] ?? null));
  const current = list.find((p) => p.id === selected);
  const blank = () => Object.fromEntries(SPRING_KEYS[kind].map((k) => [k, U.show(0, quantity(kind, k))])) as Record<string, string>;
  const fromProp = (p: (typeof list)[number] | undefined) =>
    p ? (Object.fromEntries(SPRING_KEYS[kind].map((k) => [k, U.show((p as unknown as Record<string, number>)[k] ?? 0, quantity(kind, k))])) as Record<string, string>) : blank();
  const [name, setName] = useState(current?.id ?? "");
  const [values, setValues] = useState<Record<string, string>>(() => fromProp(current));
  const pick = (id: Id | null) => {
    setSelected(id);
    const p = list.find((x) => x.id === id);
    setName(p?.id ?? "");
    setValues(fromProp(p));
  };
  const save = () => {
    const prop: Record<string, unknown> = { id: name };
    for (const k of SPRING_KEYS[kind]) {
      const v = U.read(values[k], quantity(kind, k), true);
      if (v === null || v < 0) {
        useStore.getState().notify(t("spr.errValues"), "error");
        return;
      }
      prop[k] = v;
    }
    if (edit((m) => upsertSpringProp(m, kind, selected, prop as never))) setSelected(name.trim());
  };
  const users = selected ? springUsers(model, kind, selected).length : 0;
  const labels: Record<string, string> = { u1: "U1", u2: "U2", u3: "U3", r1: "R1" };
  return (
    <Dialog title={t(kind === "line" ? "spr.lineTitle" : "spr.areaTitle")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("spr.list")} items={ids} selected={selected} onSelect={pick} />
        <div className="define-form">
          <h4>{selected ? t("spr.one", { id: selected }) : t("spr.new")}</h4>
          {selected && <p>{tn(kind === "line" ? "spr.usedFrames" : "spr.usedAreas", users)}</p>}
          <NumberField label={t("dlg.name")} value={name} onChange={setName} width={140} />
          {SPRING_KEYS[kind].map((k) => (
            <NumberField key={k} label={labels[k]} value={values[k]} onChange={(v) => setValues({ ...values, [k]: v })} unit={U.label(quantity(kind, k))} width={90} />
          ))}
          <div className="buttons">
            <button className="primary" onClick={save}>
              {t(selected ? "spr.change" : "dlg.add")}
            </button>
            {selected && <button onClick={() => edit((m) => deleteSpringProp(m, kind, selected)) && pick(null)}>{t("dlg.delete")}</button>}
          </div>
          <p className="hint">{t(kind === "line" ? "spr.lineHint" : "spr.areaHint")}</p>
        </div>
      </div>
    </Dialog>
  );
}

/** Assign > Frame > Line Springs or Assign > Shell > Area Springs: a property (or none) on the selection. */
export function AssignSpringDialog({ kind }: { kind: SpringKind }) {
  const model = useStore((s) => s.model);
  const sel = useStore((s) => (kind === "line" ? s.selection.frames : s.selection.areas));
  const list = (kind === "line" ? model.line_spring_props : model.area_spring_props) ?? [];
  const own = sel.map((id) => (kind === "line" ? model.frames.find((f) => f.id === id)?.line_spring : model.areas.find((a) => a.id === id)?.area_spring) ?? "");
  const [value, setValue] = useState(own[0] || list[0]?.id || "");
  const apply = (ok: boolean) => {
    const done = useStore.getState().edit((m) => {
      if (value && !list.some((p) => p.id === value)) throw new EditError(t("spr.errUnknown", { id: value }));
      return kind === "line" ? assignLineSpring(m, sel, value || null) : assignAreaSpring(m, sel, value || null);
    });
    if (done && ok) close();
  };
  const title: Key = kind === "line" ? "spr.assignLine" : "spr.assignArea";
  return (
    <Dialog
      title={tn(title, sel.length)}
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
        <span>{t("spr.property")}</span>
        <select aria-label={t("spr.property")} value={value} onChange={(e) => setValue(e.target.value)}>
          <option value="">{t("spr.none")}</option>
          {list.map((p) => (
            <option key={p.id} value={p.id}>
              {p.id}
            </option>
          ))}
        </select>
      </label>
      {!list.length && <p className="hint note">{t("spr.defineFirst")}</p>}
      {new Set(own).size > 1 && <p className="hint note">{t("spr.mixed")}</p>}
      <p className="hint">{t(kind === "line" ? "spr.assignLineHint" : "spr.assignAreaHint")}</p>
    </Dialog>
  );
}
