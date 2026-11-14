// Define > Groups (also Select > Select by Group): the groups with their name, colour and objects; the
// selection put in or taken out; a group's objects selected, deselected, shown only, hidden or shown.

import { useState } from "react";

import { t } from "../../i18n";
import { ALL, GROUP_KEYS, assignGroup, deleteGroup, groupMembers, memberCount, setGroup, type Members } from "../../model/groups";
import { selectionCount, useStore } from "../../state/store";
import { Dialog, NumberField } from "../Dialog";
import { ListPicker } from "./DefineDialogs";

const close = () => useStore.getState().openDialog(null);

export function GroupsDialog() {
  const model = useStore((s) => s.model);
  const names = [ALL, ...(model.groups ?? []).map((g) => g.id)];
  const [selected, setSelected] = useState<string | null>(() => {
    const item = useStore.getState().dialogItem;
    return item && names.includes(item) ? item : (model.groups?.[0]?.id ?? ALL);
  });
  return (
    <Dialog title={t("grp.title")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("grp.list")} items={names} selected={selected} onSelect={setSelected} />
        <GroupForm key={selected ?? "(new)"} id={selected} onSaved={setSelected} />
      </div>
    </Dialog>
  );
}

function GroupForm({ id, onSaved }: { id: string | null; onSaved(id: string | null): void }) {
  const model = useStore((s) => s.model);
  const selection = useStore((s) => s.selection);
  const group = model.groups?.find((g) => g.id === id);
  const all = id === ALL;
  const [name, setName] = useState(id ?? "");
  const [color, setColor] = useState(group?.color ?? "");
  const members: Members = id ? groupMembers(model, id) : { nodes: [], frames: [], links: [], areas: [] };
  const st = () => useStore.getState();
  const save = () => {
    if (st().edit((m) => setGroup(m, group ? group.id : null, { id: name, color: color || undefined }))) onSaved(name.trim());
  };
  const remove = () => {
    if (group && st().edit((m) => deleteGroup(m, group.id))) onSaved(ALL);
  };
  const assign = (out: boolean) => {
    if (group) st().edit((m) => assignGroup(m, group.id, selection, out));
  };
  const select = (mode: "replace" | "add" | "remove") => {
    const s = st().selection;
    const hidden = st().hidden;
    const next = Object.fromEntries(
      GROUP_KEYS.map((k) => {
        const g = members[k].filter((o) => !hidden[k].includes(o));
        if (mode === "replace") return [k, g];
        if (mode === "add") return [k, [...new Set([...s[k], ...g])]];
        return [k, s[k].filter((o) => !members[k].includes(o))];
      }),
    ) as Members;
    useStore.setState({ selection: next });
  };
  const counts = GROUP_KEYS.map((k) => t(`grp.count.${k}`, { n: members[k].length })).join(", ");
  const has = memberCount(members) > 0;
  const picked = selectionCount(selection) > 0;
  return (
    <div className="define-form">
      <h4>{id === null ? t("grp.new") : t("grp.group", { id })}</h4>
      <NumberField label={t("dlg.name")} value={name} onChange={setName} width={160} disabled={all} />
      {!all && (
        <label className="row">
          <span>{t("grp.color")}</span>
          <input type="color" aria-label={t("grp.color")} value={color || "#7f8c8d"} onChange={(e) => setColor(e.target.value)} />
        </label>
      )}
      {id !== null && <p data-testid="group-counts">{counts}</p>}
      {!all && (
        <div className="buttons">
          <button className="primary" onClick={save}>
            {group ? t("dlg.apply") : t("dlg.add")}
          </button>
          {group && <button onClick={remove}>{t("dlg.delete")}</button>}
        </div>
      )}
      {group && (
        <>
          <p className="hint">{t("grp.selectionHint")}</p>
          <div className="buttons">
            <button disabled={!picked} onClick={() => assign(false)}>
              {t("grp.addSelected")}
            </button>
            <button disabled={!picked} onClick={() => assign(true)}>
              {t("grp.removeSelected")}
            </button>
          </div>
        </>
      )}
      {id !== null && (
        <>
          <p className="hint">{t("grp.objectsHint")}</p>
          <div className="buttons">
            <button disabled={!has} onClick={() => select("replace")}>
              {t("grp.select")}
            </button>
            <button disabled={!has} onClick={() => select("add")}>
              {t("grp.addToSelection")}
            </button>
            <button disabled={!has} onClick={() => select("remove")}>
              {t("grp.deselect")}
            </button>
            <button disabled={!has} onClick={() => st().showOnly(members)}>
              {t("grp.showOnly")}
            </button>
            <button disabled={!has} onClick={() => st().hide(members)}>
              {t("grp.hide")}
            </button>
            <button disabled={!has} onClick={() => st().show(members)}>
              {t("grp.show")}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
