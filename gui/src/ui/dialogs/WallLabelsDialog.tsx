// Define / Assign > Pier Labels and Spandrel Labels: the labels with their name and colour (a spandrel's
// multi-story switch too), given to the selected frames and areas or taken from them.

import { useState } from "react";

import { t } from "../../i18n";
import { assignWallLabel, deleteWallLabel, labelUsers, labelsOf, setWallLabel, type WallKind } from "../../model/wallLabels";
import { useStore } from "../../state/store";
import { Dialog, NumberField } from "../Dialog";
import { ListPicker } from "./DefineDialogs";

const close = () => useStore.getState().openDialog(null);

export function WallLabelsDialog({ kind }: { kind: WallKind }) {
  const model = useStore((s) => s.model);
  const names = labelsOf(model, kind).map((l) => l.id);
  const [selected, setSelected] = useState<string | null>(() => names[0] ?? null);
  return (
    <Dialog title={t(`wl.title.${kind}`)} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t(`wl.list.${kind}`)} items={names} selected={selected} onSelect={setSelected} />
        <LabelForm key={selected ?? "(new)"} kind={kind} id={selected} onSaved={setSelected} />
      </div>
    </Dialog>
  );
}

function LabelForm({ kind, id, onSaved }: { kind: WallKind; id: string | null; onSaved(id: string | null): void }) {
  const model = useStore((s) => s.model);
  const selection = useStore((s) => s.selection);
  const label = labelsOf(model, kind).find((l) => l.id === id);
  const [name, setName] = useState(id ?? "");
  const [color, setColor] = useState(label?.color ?? "");
  const [multi, setMulti] = useState(!!label?.multi_story);
  const st = () => useStore.getState();
  const save = () => {
    if (st().edit((m) => setWallLabel(m, kind, label ? label.id : null, { id: name, color: color || undefined, multi_story: multi }))) onSaved(name.trim());
  };
  const remove = () => {
    if (label && st().edit((m) => deleteWallLabel(m, kind, label.id))) onSaved(labelsOf(st().model, kind)[0]?.id ?? null);
  };
  const picked = selection.frames.length + selection.areas.length > 0;
  const assign = (to: string | null) => st().edit((m) => assignWallLabel(m, kind, to, selection));
  return (
    <div className="define-form">
      <h4>{label ? t(`wl.label.${kind}`, { id: label.id }) : t(`wl.new.${kind}`)}</h4>
      <NumberField label={t("dlg.name")} value={name} onChange={setName} width={160} />
      <label className="row">
        <span>{t("wl.color")}</span>
        <input type="color" aria-label={t("wl.color")} value={color || "#7f8c8d"} onChange={(e) => setColor(e.target.value)} />
      </label>
      {kind === "spandrel" && (
        <label className="row">
          <input type="checkbox" checked={multi} onChange={(e) => setMulti(e.target.checked)} /> {t("wl.multiStory")}
        </label>
      )}
      {label && <p data-testid="label-users">{t("wl.users", { n: labelUsers(model, kind, label.id) })}</p>}
      <div className="buttons">
        <button className="primary" onClick={save}>
          {label ? t("dlg.apply") : t("dlg.add")}
        </button>
        {label && <button onClick={remove}>{t("dlg.delete")}</button>}
      </div>
      <p className="hint">{t(`wl.hint.${kind}`)}</p>
      <div className="buttons">
        <button disabled={!label || !picked} onClick={() => label && assign(label.id)}>
          {t("wl.assign")}
        </button>
        <button disabled={!picked} onClick={() => assign(null)}>
          {t("wl.clear")}
        </button>
      </div>
    </div>
  );
}
