// View > Set Display Options (Ctrl+W): what the model view shows. General: colours, the object types
// drawn, special effects and other items; Object Assignments: the texts and symbols of each object kind.

import { Fragment, useState } from "react";

import { t, type Key } from "../../i18n";
import { COLOR_BY, OBJECT_ITEMS, OBJECT_KINDS, defaultViewOptions, withItems, type ColorBy, type GeneralItem, type ObjectKind, type ViewOptions } from "../../scene/viewOptions";
import { useStore } from "../../state/store";
import { Dialog } from "../Dialog";
import { TYPE_KEYS } from "./MeshDialogs";

const close = () => useStore.getState().openDialog(null);

/** Object types of *Objects present in view*: joints, the frame and area types, links. */
const PRESENT: [string, Key][] = [["joint", "ex.joints"], ...TYPE_KEYS, ["link", "ex.links"]];

const GENERAL_GROUPS: [Key, GeneralItem[]][] = [
  ["vo.effects", ["shrink", "fill", "extrudeFrames", "extrudeAreas"]],
  ["vo.other", ["supports", "diaphragms", "grid", "mesh"]],
];
const KIND_TITLE: Record<ObjectKind, Key> = { joint: "ex.joints", frame: "ex.frames", area: "ex.areas", link: "ex.links" };

function Check(props: { label: string; checked: boolean; sub?: boolean; disabled?: boolean; onChange(v: boolean): void }) {
  return (
    <label className={props.sub ? "check sub" : "check"}>
      <input type="checkbox" checked={props.checked} disabled={props.disabled} onChange={(e) => props.onChange(e.target.checked)} /> {props.label}
    </label>
  );
}

export function DisplayOptionsDialog() {
  const [o, setO] = useState<ViewOptions>(() => useStore.getState().display.options);
  const [tab, setTab] = useState<"general" | "objects">("general");
  const apply = (ok: boolean) => {
    useStore.getState().setDisplay({ options: o });
    if (ok) close();
  };
  const present = (key: string, on: boolean) => setO({ ...o, hiddenTypes: on ? o.hiddenTypes.filter((k) => k !== key) : [...o.hiddenTypes, key] });
  return (
    <Dialog
      title={t("vo.title")}
      wide
      buttons={
        <>
          <button onClick={() => setO(defaultViewOptions())}>{t("vo.defaults")}</button>
          <button className="primary" onClick={() => apply(true)}>
            {t("dlg.ok")}
          </button>
          <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <div className="tabs" role="tablist">
        {(["general", "objects"] as const).map((k) => (
          <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? "tab active" : "tab"} onClick={() => setTab(k)}>
            {t(k === "general" ? "vo.general" : "vo.objects")}
          </button>
        ))}
      </div>
      {tab === "general" && (
        <div className="vo-groups">
          <fieldset>
            <legend>{t("vo.present")}</legend>
            {PRESENT.map(([k, label]) => (
              <Fragment key={k}>
                <Check label={t(label)} checked={!o.hiddenTypes.includes(k)} onChange={(v) => present(k, v)} />
                {/* Inside the joints' tick: drawn without their dots, still picked. */}
                {k === "joint" && (
                  <Check label={t("vo.jointsInvisible")} sub disabled={o.hiddenTypes.includes("joint")} checked={o.jointsInvisible} onChange={(v) => setO({ ...o, jointsInvisible: v })} />
                )}
              </Fragment>
            ))}
          </fieldset>
          <fieldset>
            <legend>{t("vo.colorBy")}</legend>
            <select aria-label={t("vo.colorBy")} value={o.colorBy} onChange={(e) => setO({ ...o, colorBy: e.target.value as ColorBy })}>
              {COLOR_BY.map((c) => (
                <option key={c} value={c}>
                  {t(`vo.color.${c}` as Key)}
                </option>
              ))}
            </select>
          </fieldset>
          {GENERAL_GROUPS.map(([title, items]) => (
            <fieldset key={title}>
              <legend>{t(title)}</legend>
              {items.map((k) => (
                <Check key={k} label={t(`vo.${k}` as Key)} checked={o[k]} onChange={(v) => setO({ ...o, [k]: v })} />
              ))}
            </fieldset>
          ))}
        </div>
      )}
      {tab === "objects" && (
        <div className="vo-groups">
          {OBJECT_KINDS.map((kind) => (
            <fieldset key={kind} aria-label={t(KIND_TITLE[kind])}>
              <legend>{t(KIND_TITLE[kind])}</legend>
              {OBJECT_ITEMS[kind].map((k) => (
                <Check
                  key={k}
                  label={t(`vo.${k}` as Key)}
                  checked={(o[kind] as Record<string, boolean>)[k]}
                  onChange={(v) => setO(withItems(o, kind, { [k]: v }))}
                />
              ))}
            </fieldset>
          ))}
        </div>
      )}
      <p className="hint">{t("vo.hint")}</p>
    </Dialog>
  );
}
