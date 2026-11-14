// Assign > Frame End Length Offsets: end offsets (automatic from connectivity, given lengths or
// none) and rigid-zone factor of the selected members.

import { useState } from "react";

import { t, tn } from "../../i18n";
import { EditError, setFrameOffsets } from "../../model/ops";
import type { EndOffsets, Frame } from "../../model/types";
import { inputNumber } from "../../scene/format";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import { Dialog, NumberField, parseNumber } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

type Mode = "auto" | "lengths" | "none";

const modeOf = (f?: Frame): Mode => (!f?.end_offsets ? "none" : f.end_offsets === "auto" ? "auto" : "lengths");

export function OffsetsDialog() {
  const model = useStore((s) => s.model);
  const frames = useStore((s) => s.selection.frames);
  const own = frames.map((id) => model.frames.find((e) => e.id === id));
  const key = (f?: Frame) => JSON.stringify([f?.end_offsets ?? null, f?.rigid_zone ?? 0]);
  const mixed = new Set(own.map(key)).size > 1;
  const first = own[0];
  const [mode, setMode] = useState<Mode>(() => modeOf(first));
  const given = first?.end_offsets && first.end_offsets !== "auto" ? first.end_offsets : { i: 0, j: 0 };
  const U = useUnits();
  const [li, setLi] = useState(U.show(given.i, "distance"));
  const [lj, setLj] = useState(U.show(given.j, "distance"));
  const [rz, setRz] = useState(inputNumber(first?.rigid_zone ?? 0));
  const u = U.label("distance");
  const apply = (ok: boolean) => {
    const done = useStore.getState().edit((m) => {
      const factor = parseNumber(rz, true);
      if (factor === null) throw new EditError(t("err.rigidZone"));
      let offsets: EndOffsets | null = null;
      if (mode === "auto") offsets = "auto";
      if (mode === "lengths") {
        const [i, j] = [U.read(li, "distance", true), U.read(lj, "distance", true)];
        if (i === null || j === null) throw new EditError(t("err.offsetNegative"));
        offsets = { i, j };
      }
      return setFrameOffsets(m, frames, offsets, factor);
    });
    if (done && ok) close();
  };
  const choices: [Mode, string][] = [
    ["auto", t("off.automatic")],
    ["lengths", t("off.lengths")],
    ["none", t("off.none")],
  ];
  return (
    <Dialog
      title={tn("off.title", frames.length)}
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
      <div className="radio-list">
        {choices.map(([m, label]) => (
          <label key={m}>
            <input type="radio" name="offset-mode" checked={mode === m} onChange={() => setMode(m)} /> {label}
          </label>
        ))}
      </div>
      {mode === "lengths" && (
        <>
          <NumberField label={t("off.i")} value={li} onChange={setLi} unit={u} />
          <NumberField label={t("off.j")} value={lj} onChange={setLj} unit={u} />
        </>
      )}
      {mode !== "none" && <NumberField label={t("off.rigid")} value={rz} onChange={setRz} />}
      {mixed && <p className="hint note">{t("off.mixed")}</p>}
      <p className="hint">{t("off.hint")}</p>
    </Dialog>
  );
}
