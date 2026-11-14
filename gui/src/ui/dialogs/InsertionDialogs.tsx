// Assign > Frame > Insertion Point and Assign > Shell > Insertion Point: where a frame's section or a
// shell sits relative to its joints, and whether that changes the stiffness (model/insertion.ts).

import { useState } from "react";

import { t, tn, type Key } from "../../i18n";
import { setAreaInsertion, setFrameInsertion } from "../../model/insertion";
import { EditError } from "../../model/ops";
import type { AreaInsertion, FrameInsertion, Vec3 } from "../../model/types";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import { Dialog, NumberField } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

function OkApplyCancel({ apply }: { apply(ok: boolean): void }) {
  return (
    <>
      <button className="primary" onClick={() => apply(true)}>
        {t("dlg.ok")}
      </button>
      <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
      <button onClick={close}>{t("dlg.cancel")}</button>
    </>
  );
}

const POINTS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

/** Assign > Frame > Insertion Point. */
export function FrameInsertionDialog() {
  const model = useStore((s) => s.model);
  const frames = useStore((s) => s.selection.frames);
  const own = frames.map((id) => model.frames.find((f) => f.id === id));
  const mixed = new Set(own.map((f) => JSON.stringify(f?.insertion ?? null))).size > 1;
  const first: FrameInsertion = own[0]?.insertion ?? { point: 10 };
  const U = useUnits();
  const [point, setPoint] = useState(first.point);
  const [mirror2, setMirror2] = useState(!!first.mirror2);
  const [mirror3, setMirror3] = useState(!!first.mirror3);
  const [global, setGlobal] = useState(!!first.global);
  const [transform, setTransform] = useState(!!first.transform);
  const show = (o: Vec3 | undefined) => [0, 1, 2].map((k) => U.show(o?.[k] ?? 0, "distance"));
  const [oi, setOi] = useState(show(first.offset_i));
  const [oj, setOj] = useState(show(first.offset_j));
  const axes = global ? ["X", "Y", "Z"] : ["1", "2", "3"];
  const apply = (ok: boolean) => {
    const done = useStore.getState().edit((m) => {
      const read = (v: string[]) => v.map((x) => U.read(x, "distance", true));
      const [i, j] = [read(oi), read(oj)];
      if ([...i, ...j].some((x) => x === null)) throw new EditError(t("ins.numbers"));
      return setFrameInsertion(m, frames, { point, mirror2, mirror3, offset_i: i as Vec3, offset_j: j as Vec3, global, transform });
    });
    if (done && ok) close();
  };
  const offsetRow = (label: Key, v: string[], set: (v: string[]) => void) => (
    <div className="row">
      <span className="field-label">{t(label)}</span>
      {v.map((x, k) => (
        <NumberField key={k} label={`${t(label)} ${axes[k]}`} value={x} onChange={(s) => set(v.map((y, n) => (n === k ? s : y)))} unit={U.label("distance")} width={64} />
      ))}
    </div>
  );
  return (
    <Dialog title={tn("ins.frameTitle", frames.length)} wide buttons={<OkApplyCancel apply={apply} />}>
      <label className="field">
        <span className="field-label">{t("ins.point")}</span>
        <select aria-label={t("ins.point")} value={point} onChange={(e) => setPoint(Number(e.target.value))}>
          {POINTS.map((p) => (
            <option key={p} value={p}>
              {p} {t(`ins.p${p}` as Key)}
            </option>
          ))}
        </select>
      </label>
      <label className="check">
        <input type="checkbox" checked={mirror2} onChange={(e) => setMirror2(e.target.checked)} /> {t("ins.mirror2")}
      </label>
      <label className="check">
        <input type="checkbox" checked={mirror3} onChange={(e) => setMirror3(e.target.checked)} /> {t("ins.mirror3")}
      </label>
      <h4>{t("ins.offsets")}</h4>
      <label className="field">
        <span className="field-label">{t("ins.axes")}</span>
        <select aria-label={t("ins.axes")} value={global ? "global" : "local"} onChange={(e) => setGlobal(e.target.value === "global")}>
          <option value="local">{t("ins.local")}</option>
          <option value="global">{t("ins.global")}</option>
        </select>
      </label>
      {offsetRow("ins.endI", oi, setOi)}
      {offsetRow("ins.endJ", oj, setOj)}
      <label className="check">
        <input type="checkbox" checked={transform} onChange={(e) => setTransform(e.target.checked)} /> {t("ins.transform")}
      </label>
      {mixed && <p className="hint note">{t("ins.mixed")}</p>}
      <p className="hint">{t("ins.frameHint")}</p>
    </Dialog>
  );
}

/** Assign > Shell > Insertion Point. */
export function AreaInsertionDialog() {
  const model = useStore((s) => s.model);
  const areas = useStore((s) => s.selection.areas);
  const own = areas.map((id) => model.areas.find((a) => a.id === id));
  const mixed = new Set(own.map((a) => JSON.stringify(a?.insertion ?? null))).size > 1;
  const first: AreaInsertion = own[0]?.insertion ?? { point: "middle" };
  const [point, setPoint] = useState<AreaInsertion["point"]>(first.point);
  const [transform, setTransform] = useState(!!first.transform);
  const apply = (ok: boolean) => {
    if (useStore.getState().edit((m) => setAreaInsertion(m, areas, { point, transform })) && ok) close();
  };
  return (
    <Dialog title={tn("ins.areaTitle", areas.length)} buttons={<OkApplyCancel apply={apply} />}>
      <div className="radio-list">
        {(["top", "middle", "bottom"] as const).map((p) => (
          <label key={p}>
            <input type="radio" name="area-insertion" checked={point === p} onChange={() => setPoint(p)} /> {t(`ins.${p}`)}
          </label>
        ))}
      </div>
      <label className="check">
        <input type="checkbox" checked={transform} onChange={(e) => setTransform(e.target.checked)} /> {t("ins.transform")}
      </label>
      {mixed && <p className="hint note">{t("ins.mixed")}</p>}
      <p className="hint">{t("ins.areaHint")}</p>
    </Dialog>
  );
}
