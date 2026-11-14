// Mesh and object-type dialogs: Analyze > Mesh Settings, Options > Tolerances (type angle limits),
// Assign > Area Mesh Options, Assign > Floor Meshing Option (*Include in mesh*), Select > Object
// Type and View > Show Object Types.

import { useState } from "react";

import { t, tn, type Key } from "../../i18n";
import { setAreaMesh } from "../../model/areaOps";
import { AREA_TYPES, DEFAULT_ANGLE_LIMITS, FRAME_TYPES, angleLimitsOf, idsOfType, type AreaType, type FrameType } from "../../model/objectTypes";
import { defaultMeshSize, setAngleLimits, setInMesh, setMeshSettings, setSpecial } from "../../model/ops";
import type { AreaMesh, FloorMesh, MeshSettings } from "../../model/types";
import { inputNumber } from "../../scene/format";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import { Dialog, NumberField, parseNumber } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

function OkCancel({ ok }: { ok(): void }) {
  return (
    <>
      <button className="primary" onClick={ok}>
        {t("dlg.ok")}
      </button>
      <button onClick={close}>{t("dlg.cancel")}</button>
    </>
  );
}

/** Analyze > Mesh Settings: floor mesh type and maximum sizes (areas analysed as one element are set per area). */
export function MeshSettingsDialog() {
  const model = useStore((s) => s.model);
  const s: MeshSettings = model.mesh ?? {};
  const U = useUnits();
  const size0 = defaultMeshSize(model.units.length);
  const [floor, setFloor] = useState<FloorMesh>(s.floor ?? "general");
  const [floorSize, setFloorSize] = useState(U.show(s.floor_size ?? size0, "distance"));
  const [wallSize, setWallSize] = useState(U.show(s.wall_size ?? size0, "distance"));
  const [refine, setRefine] = useState(s.refine !== false);
  const [error, setError] = useState("");
  const ok = () => {
    const [fs, ws] = [U.read(floorSize, "distance"), U.read(wallSize, "distance")];
    if (fs === null || ws === null || !(fs > 0) || !(ws > 0)) return setError(t("err.meshSize"));
    const out: MeshSettings = {};
    if (floor !== "general") out.floor = floor;
    // A size typed in other units comes back within round-off of the default.
    const isDefault = (v: number) => Math.abs(v - size0) <= 1e-9 * size0;
    if (!isDefault(fs)) out.floor_size = fs;
    if (!isDefault(ws)) out.wall_size = ws;
    if (!refine) out.refine = false;
    // Defaults need no settings in the file.
    const mesh = Object.keys(out).length ? out : null;
    const same = JSON.stringify(mesh ?? undefined) === JSON.stringify(model.mesh);
    if (same || useStore.getState().edit((m) => setMeshSettings(m, mesh))) close();
  };
  const u = U.label("distance");
  return (
    <Dialog title={t("mesh.title")} buttons={<OkCancel ok={ok} />}>
      <label className="row">
        <span>{t("mesh.floorType")}</span>
        <select aria-label={t("mesh.floorType")} value={floor} onChange={(e) => setFloor(e.target.value as FloorMesh)}>
          <option value="general">{t("mesh.general")}</option>
          <option value="rectangular">{t("mesh.rectangular")}</option>
        </select>
      </label>
      {floor === "general" && (
        <label className="check">
          <input type="checkbox" checked={refine} onChange={(e) => setRefine(e.target.checked)} /> {t("mesh.refine")}
        </label>
      )}
      <NumberField label={t("mesh.floorSize")} value={floorSize} onChange={setFloorSize} unit={u} width={80} />
      <NumberField label={t("mesh.wallSize")} value={wallSize} onChange={setWallSize} unit={u} width={80} />
      <p className="hint">{t("mesh.hint", { size: U.fmt(size0, "distance"), unit: u })}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

/** Options > Tolerances: two angle limits for object types (a model setting). */
export function AngleLimitsDialog() {
  const model = useStore((s) => s.model);
  const lim = angleLimitsOf(model);
  const [column, setColumn] = useState(inputNumber(lim.column));
  const [beam, setBeam] = useState(inputNumber(lim.beam_floor));
  const [error, setError] = useState("");
  const ok = () => {
    const [c, b] = [parseNumber(column), parseNumber(beam)];
    if (c === null || b === null) return setError(t("dlg.enterNumbers"));
    if (c === lim.column && b === lim.beam_floor) return close();
    if (useStore.getState().edit((m) => setAngleLimits(m, { column: c, beam_floor: b }))) close();
  };
  return (
    <Dialog title={t("tol.title")} buttons={<OkCancel ok={ok} />}>
      <NumberField label={t("tol.column")} value={column} onChange={setColumn} unit="°" width={70} />
      <NumberField label={t("tol.beamFloor")} value={beam} onChange={setBeam} unit="°" width={70} />
      <p className="hint">{t("tol.hint", { column: DEFAULT_ANGLE_LIMITS.column, beam: DEFAULT_ANGLE_LIMITS.beam_floor })}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

type Tri = "default" | "yes" | "no";

/** Assign > Area Mesh Options: the model's settings, an own maximum size, or one element; edge restraints. */
export function AreaMeshDialog() {
  const model = useStore((s) => s.model);
  const areas = useStore((s) => s.selection.areas);
  const first = model.areas.find((a) => a.id === areas[0]);
  const U = useUnits();
  const [kind, setKind] = useState<"default" | "size" | "none">(first?.mesh === "none" ? "none" : first?.mesh ? "size" : "default");
  const [size, setSize] = useState(U.show(first?.mesh && first.mesh !== "none" ? first.mesh.size : defaultMeshSize(model.units.length), "distance"));
  const [edge, setEdge] = useState<Tri>(first?.edge_restraints === undefined ? "default" : first.edge_restraints ? "yes" : "no");
  const [error, setError] = useState("");
  const ok = () => {
    let mesh: "default" | AreaMesh = kind === "none" ? "none" : "default";
    if (kind === "size") {
      const v = U.read(size, "distance");
      if (v === null || !(v > 0)) return setError(t("err.meshSize"));
      mesh = { size: v };
    }
    const e = edge === "default" ? undefined : edge === "yes";
    if (useStore.getState().edit((m) => setAreaMesh(m, areas, mesh, e))) close();
  };
  return (
    <Dialog title={tn("amesh.title", areas.length)} buttons={<OkCancel ok={ok} />}>
      <div className="radio-list">
        <label>
          <input type="radio" name="areamesh" checked={kind === "default"} onChange={() => setKind("default")} /> {t("amesh.default")}
        </label>
        <label>
          <input type="radio" name="areamesh" checked={kind === "size"} onChange={() => setKind("size")} /> {t("amesh.size")}
        </label>
        <label>
          <input type="radio" name="areamesh" checked={kind === "none"} onChange={() => setKind("none")} /> {t("amesh.none")}
        </label>
      </div>
      {kind === "size" && <NumberField label={t("amesh.maxSize")} value={size} onChange={setSize} unit={U.label("distance")} width={80} />}
      <label className="row">
        <span>{t("amesh.edge")}</span>
        <select aria-label={t("amesh.edge")} value={edge} onChange={(e) => setEdge(e.target.value as Tri)}>
          <option value="default">{t("amesh.edgeDefault")}</option>
          <option value="yes">{t("dlg.yes")}</option>
          <option value="no">{t("dlg.no")}</option>
        </select>
      </label>
      <p className="hint">{t("amesh.hint")}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

/** Assign > Floor Meshing Option: *Include in mesh* for the selected joints and frames. */
export function FloorMeshingDialog() {
  const sel = useStore((s) => s.selection);
  const model = useStore((s) => s.model);
  const node = model.nodes.find((n) => n.id === sel.nodes[0]);
  const frame = model.frames.find((f) => f.id === sel.frames[0]);
  const start: Tri = node?.in_mesh ? "yes" : frame?.in_mesh === false ? "no" : "default";
  const [value, setValue] = useState<Tri>(start);
  const ok = () => {
    const v = value === "default" ? undefined : value === "yes";
    if (useStore.getState().edit((m) => setInMesh(m, sel.nodes, sel.frames, v))) close();
  };
  return (
    <Dialog title={t("fmesh.title", { joints: sel.nodes.length, frames: sel.frames.length })} buttons={<OkCancel ok={ok} />}>
      <div className="radio-list">
        {(
          [
            ["default", "fmesh.default"],
            ["yes", "fmesh.include"],
            ["no", "fmesh.exclude"],
          ] as [Tri, Key][]
        ).map(([k, label]) => (
          <label key={k}>
            <input type="radio" name="inmesh" checked={value === k} onChange={() => setValue(k)} /> {t(label)}
          </label>
        ))}
      </div>
      <p className="hint">{t("fmesh.hint")}</p>
    </Dialog>
  );
}

/** Assign > Special Joint: special joints stay when the objects attached to them are deleted or moved. */
export function SpecialJointDialog() {
  const sel = useStore((s) => s.selection.nodes);
  const model = useStore((s) => s.model);
  const [value, setValue] = useState(!!model.nodes.find((n) => n.id === sel[0])?.special);
  const ok = () => {
    if (useStore.getState().edit((m) => setSpecial(m, sel, value))) close();
  };
  return (
    <Dialog title={tn("special.title", sel.length)} buttons={<OkCancel ok={ok} />}>
      <label className="check">
        <input type="checkbox" checked={value} onChange={(e) => setValue(e.target.checked)} /> {t("prop.special")}
      </label>
      <p className="hint">{t("special.hint")}</p>
    </Dialog>
  );
}

/** Group names of the object types, as in the Model Explorer. */
const FRAME_GROUP: Record<FrameType, Key> = { column: "ex.columns", beam: "ex.beams", brace: "ex.braces", null: "ex.nullFrames" };
const AREA_GROUP: Record<AreaType, Key> = { floor: "ex.floors", wall: "ex.walls", opening: "ex.openings", null: "ex.nullAreas" };

/** Every object type as "frame:column", "area:floor"…, with its group name. */
export const TYPE_KEYS: [string, Key][] = [
  ...FRAME_TYPES.map((ty): [string, Key] => [`frame:${ty}`, FRAME_GROUP[ty]]),
  ...AREA_TYPES.map((ty): [string, Key] => [`area:${ty}`, AREA_GROUP[ty]]),
];

/** The frames and areas of the given type keys ("frame:column"…). */
export function objectsOfTypeKeys(keys: string[]) {
  const m = useStore.getState().model;
  const pick = (of: string) => keys.filter((k) => k.startsWith(of)).map((k) => k.slice(of.length)) as (FrameType | AreaType)[];
  return { frames: idsOfType(m, pick("frame:")).frames, areas: idsOfType(m, pick("area:")).areas };
}

function TypeChecks({ value, onChange }: { value: string[]; onChange(v: string[]): void }) {
  return (
    <div className="checks type-checks">
      {TYPE_KEYS.map(([k, label]) => (
        <label key={k} className="check">
          <input type="checkbox" checked={value.includes(k)} onChange={(e) => onChange(e.target.checked ? [...value, k] : value.filter((x) => x !== k))} /> {t(label)}
        </label>
      ))}
    </div>
  );
}

/** Select > Object Type: select or deselect every object of the ticked types. */
export function SelectTypeDialog() {
  const [types, setTypes] = useState<string[]>([]);
  const act = (add: boolean) => {
    const o = objectsOfTypeKeys(types);
    const s = useStore.getState();
    if (add) s.select([...o.frames.map((id) => ({ kind: "frame" as const, id })), ...o.areas.map((id) => ({ kind: "area" as const, id }))], "add");
    else s.deselect(o);
    close();
  };
  return (
    <Dialog
      title={t("sel.title")}
      buttons={
        <>
          <button className="primary" onClick={() => act(true)}>
            {t("sel.select")}
          </button>
          <button onClick={() => act(false)}>{t("sel.deselect")}</button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <TypeChecks value={types} onChange={setTypes} />
    </Dialog>
  );
}
