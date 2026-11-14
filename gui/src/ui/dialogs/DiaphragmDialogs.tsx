// Define > Diaphragms (rigid or semi-rigid) and Assign > Diaphragm (selected joints and areas).

import { useState } from "react";

import { t } from "../../i18n";
import { addDiaphragm, deleteDiaphragm, setDiaphragm, setDiaphragmSemiRigid } from "../../model/ops";
import { useStore } from "../../state/store";
import { Dialog, NumberField } from "../Dialog";
import { ListPicker } from "./DefineDialogs";

const close = () => useStore.getState().openDialog(null);

export function DiaphragmsDialog() {
  const model = useStore((s) => s.model);
  const ids = (model.diaphragms ?? []).map((d) => d.id);
  const [selected, setSelected] = useState<string | null>(ids[0] ?? null);
  const [name, setName] = useState("");
  const [semi, setSemi] = useState(false);
  const edit = useStore((s) => s.edit);
  const current = (model.diaphragms ?? []).find((d) => d.id === selected);
  const users = selected ? [...model.nodes, ...model.areas].filter((o) => o.diaphragm === selected).length : 0;
  return (
    <Dialog title={t("dia.title")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("dia.list")} items={ids} selected={selected} onSelect={setSelected} />
        <div className="define-form">
          {selected ? (
            <>
              <h4>{t("dia.one", { id: selected })}</h4>
              <p>{t("dia.used", { n: users })}</p>
              <label className="check">
                <input type="checkbox" checked={!!current?.semi_rigid} onChange={(e) => edit((m) => setDiaphragmSemiRigid(m, selected, e.target.checked))} /> {t("dia.semiRigid")}
              </label>
              <div className="buttons">
                <button onClick={() => edit((m) => deleteDiaphragm(m, selected)) && setSelected(null)}>{t("dlg.delete")}</button>
              </div>
            </>
          ) : (
            <>
              <h4>{t("dia.new")}</h4>
              <NumberField label={t("dlg.name")} value={name} onChange={setName} width={140} />
              <label className="check">
                <input type="checkbox" checked={semi} onChange={(e) => setSemi(e.target.checked)} /> {t("dia.semiRigid")}
              </label>
              <div className="buttons">
                <button
                  className="primary"
                  onClick={() => {
                    if (edit((m) => addDiaphragm(m, name, semi))) {
                      setSelected(name.trim());
                      setName("");
                    }
                  }}
                >
                  {t("dlg.add")}
                </button>
              </div>
            </>
          )}
          <p className="hint">{t("dia.hint")}</p>
        </div>
      </div>
    </Dialog>
  );
}

export function AssignDiaphragmDialog() {
  const model = useStore((s) => s.model);
  const sel = useStore((s) => s.selection);
  const ids = (model.diaphragms ?? []).map((d) => d.id);
  const own = [...model.nodes.filter((n) => sel.nodes.includes(n.id)), ...model.areas.filter((a) => sel.areas.includes(a.id))].map((o) => o.diaphragm ?? "");
  const [value, setValue] = useState(own[0] ?? ids[0] ?? "");
  const apply = (ok: boolean) => {
    if (useStore.getState().edit((m) => setDiaphragm(m, sel.nodes, sel.areas, value || null)) && ok) close();
  };
  return (
    <Dialog
      title={t("dia.assignTitle", { joints: sel.nodes.length, areas: sel.areas.length })}
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
        <span>{t("dia.diaphragm")}</span>
        <select aria-label={t("dia.diaphragm")} value={value} onChange={(e) => setValue(e.target.value)}>
          <option value="">{t("dia.none")}</option>
          {ids.map((id) => (
            <option key={id} value={id}>
              {id}
            </option>
          ))}
        </select>
      </label>
      {new Set(own).size > 1 && <p className="hint note">{t("dia.mixed")}</p>}
      <p className="hint">{t("dia.assignHint")}</p>
    </Dialog>
  );
}
