// Edit > Move and Edit > Replicate (model/edit.ts). Offsets, axes and mirror planes are typed or picked
// in the view (the dialog hides while points are picked).

import { useState } from "react";

import { t } from "../../i18n";
import { DEFAULT_REPLICATE_OPTIONS, moveObjects, replicateObjects, type MoveResult, type ReplicateResult, type ReplicateSpec } from "../../model/edit";
import { storyLevels } from "../../model/stories";
import type { Vec3 } from "../../model/types";
import type { PickRef } from "../../scene/types";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import { Dialog, NumberField, parseNumber } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

/** The selection's counts, shown at the top of both dialogs. */
function SelectionLine() {
  const sel = useStore((s) => s.selection);
  return <p className="hint note">{t("edit.selected", { joints: sel.nodes.length, frames: sel.frames.length, links: sel.links.length, areas: sel.areas.length })}</p>;
}

function PickButton({ need, onPicked }: { need: 1 | 2; onPicked(points: Vec3[]): void }) {
  return (
    <button title={t("pick.title")} onClick={() => useStore.getState().startPick(need, onPicked)}>
      {t("pick.points")}
    </button>
  );
}

/** Three distance fields (display units). */
function Offset({ value, onChange, labels = ["dX", "dY", "dZ"] }: { value: string[]; onChange(v: string[]): void; labels?: string[] }) {
  const U = useUnits();
  return (
    <div className="row">
      {labels.map((label, i) => (
        <NumberField key={label} label={label} value={value[i]} unit={U.label("distance")} width={80} onChange={(v) => onChange(value.map((x, k) => (k === i ? v : x)))} />
      ))}
    </div>
  );
}

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

/** Edit > Move (Ctrl+M): moves the selection by an offset. */
export function MoveDialog() {
  const U = useUnits();
  const [d, setD] = useState(["0", "0", "0"]);
  const [error, setError] = useState("");
  const apply = (ok: boolean) => {
    const v = d.map((x) => U.read(x, "distance", true));
    if (v.some((x) => x === null)) return setError(t("edit.numbers"));
    setError("");
    const s = useStore.getState();
    let r: MoveResult | null = null;
    if (!s.edit((m) => (r = moveObjects(m, s.selection, v as Vec3)).model)) return;
    const res = r as MoveResult | null;
    const notes = [res?.merged ? t("edit.merged", { n: res.merged }) : "", res?.meshesCleared.length ? t("edit.meshesCleared", { list: res.meshesCleared.join(", ") }) : ""].filter(Boolean);
    if (notes.length) s.notify(notes.join(" "));
    if (ok) close();
  };
  return (
    <Dialog title={t("edit.moveTitle")} buttons={<OkApplyCancel apply={apply} />}>
      <SelectionLine />
      <h4>{t("edit.offset")}</h4>
      <Offset value={d} onChange={setD} />
      <PickButton need={2} onPicked={([a, b]) => setD([0, 1, 2].map((k) => U.show(b[k] - a[k], "distance")))} />
      <p className="hint">{t("edit.moveHint")}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

type Kind = ReplicateSpec["kind"];
const KINDS: [Kind, "edit.linear" | "edit.radial" | "edit.mirror" | "edit.story"][] = [
  ["linear", "edit.linear"],
  ["radial", "edit.radial"],
  ["mirror", "edit.mirror"],
  ["story", "edit.story"],
];

/** Edit > Replicate (Ctrl+R): copies of the selection, on four tabs. */
export function ReplicateDialog() {
  const U = useUnits();
  const model = useStore((s) => s.model);
  const stories = storyLevels(model).slice(1).reverse();
  const [kind, setKind] = useState<Kind>("linear");
  const [offset, setOffset] = useState(["0", "0", "0"]);
  const [copies, setCopies] = useState("1");
  const [center, setCenter] = useState(["0", "0"]);
  const [angle, setAngle] = useState("90");
  const [rotateAxes, setRotateAxes] = useState(false);
  const [plane, setPlane] = useState(["0", "0", "0", "0"]);
  const [targets, setTargets] = useState<string[]>([]);
  const [opts, setOpts] = useState(DEFAULT_REPLICATE_OPTIONS);
  const [error, setError] = useState("");
  const dist = (v: number) => U.show(v, "distance");

  const spec = (): ReplicateSpec | null => {
    const read = (xs: string[]) => xs.map((x) => U.read(x, "distance", true));
    const n = parseNumber(copies);
    if (kind === "linear") {
      const v = read(offset);
      return v.some((x) => x === null) || n === null ? null : { kind, offset: v as Vec3, copies: n };
    }
    if (kind === "radial") {
      const c = read(center);
      const a = parseNumber(angle);
      return c.some((x) => x === null) || a === null || n === null ? null : { kind, center: c as [number, number], angle: a, copies: n, rotateAxes };
    }
    if (kind === "mirror") {
      const p = read(plane);
      return p.some((x) => x === null) ? null : { kind, a: [p[0]!, p[1]!], b: [p[2]!, p[3]!] };
    }
    return { kind, stories: targets };
  };
  const apply = (ok: boolean) => {
    const sp = spec();
    if (!sp) return setError(t("edit.numbers"));
    setError("");
    const s = useStore.getState();
    let r: ReplicateResult | null = null;
    if (!s.edit((m) => (r = replicateObjects(m, s.selection, sp, opts)).model)) return;
    const res = r as unknown as ReplicateResult;
    const c = res.created;
    const picks: PickRef[] = [
      ...c.nodes.map((id) => ({ kind: "node" as const, id })),
      ...c.frames.map((id) => ({ kind: "frame" as const, id })),
      ...c.links.map((id) => ({ kind: "link" as const, id })),
      ...c.areas.map((id) => ({ kind: "area" as const, id })),
    ];
    s.select(picks, "replace");
    s.notify([t("edit.replicated", { n: picks.length }), res.skipped ? t("edit.skipped", { n: res.skipped }) : ""].filter(Boolean).join(" "));
    if (ok) close();
  };
  const tick = (key: keyof typeof opts, label: Parameters<typeof t>[0]) => (
    <label className="check">
      <input type="checkbox" checked={opts[key]} onChange={(e) => setOpts({ ...opts, [key]: e.target.checked })} /> {t(label)}
    </label>
  );

  return (
    <Dialog title={t("edit.replicateTitle")} wide buttons={<OkApplyCancel apply={apply} />}>
      <SelectionLine />
      <div className="tabs" role="tablist">
        {KINDS.map(([k, label]) => (
          <button key={k} role="tab" aria-selected={kind === k} className={kind === k ? "tab active" : "tab"} onClick={() => setKind(k)}>
            {t(label)}
          </button>
        ))}
      </div>
      {kind === "linear" && (
        <>
          <h4>{t("edit.offset")}</h4>
          <Offset value={offset} onChange={setOffset} />
          <PickButton need={2} onPicked={([a, b]) => setOffset([0, 1, 2].map((k) => dist(b[k] - a[k])))} />
          <NumberField label={t("edit.copies")} value={copies} onChange={setCopies} width={60} />
        </>
      )}
      {kind === "radial" && (
        <>
          <h4>{t("edit.center")}</h4>
          <Offset value={center} onChange={setCenter} labels={["X", "Y"]} />
          <PickButton need={1} onPicked={([p]) => setCenter([dist(p[0]), dist(p[1])])} />
          <NumberField label={t("edit.angle")} value={angle} onChange={setAngle} unit="°" width={60} />
          <NumberField label={t("edit.copies")} value={copies} onChange={setCopies} width={60} />
          <label className="check">
            <input type="checkbox" checked={rotateAxes} onChange={(e) => setRotateAxes(e.target.checked)} /> {t("edit.rotateAxes")}
          </label>
        </>
      )}
      {kind === "mirror" && (
        <>
          <p className="hint">{t("edit.mirrorHint")}</p>
          <h4>{t("edit.point1")}</h4>
          <Offset value={plane.slice(0, 2)} onChange={(v) => setPlane([...v, ...plane.slice(2)])} labels={["X1", "Y1"]} />
          <h4>{t("edit.point2")}</h4>
          <Offset value={plane.slice(2)} onChange={(v) => setPlane([...plane.slice(0, 2), ...v])} labels={["X2", "Y2"]} />
          <PickButton need={2} onPicked={([a, b]) => setPlane([dist(a[0]), dist(a[1]), dist(b[0]), dist(b[1])])} />
        </>
      )}
      {kind === "story" && (
        <>
          <h4>{t("edit.stories")}</h4>
          <div className="checks">
            {stories.map((st) => (
              <label key={st.id} className="check">
                <input type="checkbox" checked={targets.includes(st.id)} onChange={(e) => setTargets(e.target.checked ? [...targets, st.id] : targets.filter((x) => x !== st.id))} /> {st.id}
              </label>
            ))}
          </div>
        </>
      )}
      <h4>{t("edit.options")}</h4>
      {tick("supports", "edit.optSupports")}
      {tick("loads", "edit.optLoads")}
      {tick("diaphragms", "edit.optDiaphragms")}
      {tick("deleteOriginals", "edit.deleteOriginals")}
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}
