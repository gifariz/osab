import { useEffect, useState } from "react";

import { useCursor } from "../actions";
import { isDemo } from "../demo";
import { t, type Key } from "../i18n";
import { backend } from "../ipc/backend";
import { objectTypes, typeName } from "../model/objectTypes";
import { dimensionKey, linkLength, memberLength, nodeById } from "../model/ops";
import { shellValue } from "../scene/areas";
import { shellQuantity, useUnits } from "../units/format";
import { UNIT_SYSTEMS, type UnitSystem } from "../units/units";
import { caseOf, resultLabel } from "../scene/buildResultsScene";
import { viewName } from "../scene/view";
import { selectionCount, useStore } from "../state/store";

export function StatusBar() {
  const model = useStore((s) => s.model);
  const hover = useStore((s) => s.hover);
  const tool = useStore((s) => s.tool);
  const view = useStore((s) => s.view);
  const results = useStore((s) => s.results);
  const caseId = useStore((s) => s.display.caseId);
  const display = useStore((s) => s.display);
  const selection = useStore((s) => s.selection);
  const allStories = useStore((s) => s.allStories);
  const cursor = useCursor((c) => c.p);
  const snapped = useCursor((c) => c.snapped);
  const slowRun = useSlowRun();
  const U = useUnits();

  let info = t(`status.hint.${tool}` as Key);
  const measureStart = useStore((s) => s.measureStart);
  if (tool === "measure" && measureStart && cursor) {
    const d = Math.hypot(cursor[0] - measureStart[0], cursor[1] - measureStart[1], cursor[2] - measureStart[2]);
    info = t("measure.live", { d: U.fmt(d, "distance"), unit: U.label("distance") });
  }
  if (hover?.kind === "node") {
    const n = nodeById(model, hover.id);
    if (n) {
      info = t("status.joint", { id: n.id, x: U.fmt(n.x, "distance"), y: U.fmt(n.y, "distance"), z: U.fmt(n.z, "distance") });
      const d = results && caseOf(results, caseId)?.displacements.find((x) => x.node === n.id);
      if (d) {
        const [tr, rot] = [(v: number) => U.fmt(v, "displacement"), (v: number) => U.fmt(v, "rotation")];
        info += `   ${resultLabel(caseOf(results, caseId)!)}: UX ${tr(d.ux)}  UY ${tr(d.uy)}  UZ ${tr(d.uz)} ${U.label("displacement")}  RX ${rot(d.rx)}  RY ${rot(d.ry)}  RZ ${rot(d.rz)} rad`;
      }
    }
  } else if (hover?.kind === "frame") {
    const e = model.frames.find((x) => x.id === hover.id);
    if (e)
      info = t("status.frame", {
        kind: t(typeName(objectTypes(model).frames.get(e.id) ?? "brace", "frame")) + (e.type === "truss" ? t("ex.truss") : ""),
        id: e.id,
        i: e.nodes[0],
        j: e.nodes[1],
        length: U.fmt(memberLength(model, e.id), "distance"),
        unit: U.label("distance"),
        section: e.section || t("obj.noSection"),
      });
  } else if (hover?.kind === "area") {
    const a = model.areas.find((x) => x.id === hover.id);
    if (a) {
      info = t("status.area", { kind: t(typeName(objectTypes(model).areas.get(a.id) ?? "wall", "area")), id: a.id, joints: a.nodes.join(", "), section: a.section || t("obj.noSection") });
      // Shell forces: the corners of an area analysed as one element, or the range over its mesh elements.
      const shs = results && display.mode === "shells" ? (caseOf(results, caseId)?.shells ?? []).filter((x) => x.area === a.id) : [];
      const comp = display.shellComponent;
      const sq = shellQuantity(comp);
      if (shs.length === 1) info += `   ${comp}: ${shs[0].corners.map((c) => `${c.node} ${U.fmt(shellValue(c, comp), sq)}`).join(", ")} ${U.label(sq)}`;
      else if (shs.length > 1) {
        const all = shs.flatMap((s) => s.corners.map((c) => ({ at: `${s.element} ${c.node}`, v: shellValue(c, comp) })));
        const hi = all.reduce((p, q) => (q.v > p.v ? q : p));
        const lo = all.reduce((p, q) => (q.v < p.v ? q : p));
        info += `   ${comp}: ${t("status.meshRange", { n: shs.length, max: U.fmt(hi.v, sq), maxAt: hi.at, min: U.fmt(lo.v, sq), minAt: lo.at })} ${U.label(sq)}`;
      }
    }
  } else if (hover?.kind === "link") {
    const l = model.links.find((x) => x.id === hover.id);
    if (l) info = t("status.link", { id: l.id, i: l.nodes[0], j: l.nodes[1], length: U.fmt(linkLength(model, l.id), "distance"), unit: U.label("distance") });
  }
  const sel = selectionCount(selection);
  const hidden = useStore((s) => selectionCount(s.hidden));
  return (
    <div className="statusbar">
      <span className="status-info">{info}</span>
      {slowRun && backend.canStop && (
        <button className="status-cell status-button" onClick={() => backend.stop()} title={t("status.stopTitle")}>
          {t("status.stop")}
        </button>
      )}
      {sel > 0 && (
        <span className="status-cell">
          {t("status.selected", {
            joints: selection.nodes.length,
            members: selection.frames.length,
            links: selection.links.length ? t("status.selectedLinks", { n: selection.links.length }) : "",
            areas: selection.areas.length ? t("status.selectedAreas", { n: selection.areas.length }) : "",
          })}
        </span>
      )}
      {hidden > 0 && (
        <button className="status-cell status-button" onClick={() => useStore.getState().showAll()} title={t("status.showAllTitle")}>
          {t("status.hidden", { n: hidden })}
        </button>
      )}
      <span className="status-cell">{viewName(model, view)}</span>
      <span className="status-cell coords">
        {cursor ? `${snapped ? "◎ " : ""}X ${U.fmt(cursor[0], "distance", 0, 7)}  Y ${U.fmt(cursor[1], "distance", 0, 7)}  Z ${U.fmt(cursor[2], "distance", 0, 7)} ${U.label("distance")}` : " "}
      </span>
      <span className="status-cell" title={t("status.dofTitle")}>
        {t(dimensionKey(model))}
      </span>
      <select
        className="status-cell status-units"
        aria-label={t("status.stories")}
        title={t("status.storiesTitle")}
        value={allStories ? "all" : "one"}
        onChange={(e) => useStore.getState().setAllStories(e.target.value === "all")}
      >
        <option value="one">{t("status.storiesOne")}</option>
        <option value="all">{t("status.storiesAll")}</option>
      </select>
      <select
        className="status-cell status-units"
        aria-label={t("units.label")}
        title={t("units.title", { system: U.system })}
        value={U.system}
        onChange={(e) => useStore.getState().setUnitSystem(e.target.value as UnitSystem)}
      >
        {UNIT_SYSTEMS.map((s) => (
          <option key={s} value={s} title={t(`units.${s}` as Key)}>
            {s}
          </option>
        ))}
      </select>
      {isDemo && (
        <button className="status-cell status-button" onClick={() => useStore.getState().openDialog("welcome")} title={t("status.demoTitle")}>
          {t("status.demo")}
        </button>
      )}
    </div>
  );
}

/** True while a run has taken longer than half a second (short runs do not flash the Stop button). */
function useSlowRun() {
  const running = useStore((s) => s.running);
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    if (!running) {
      setSlow(false);
      return;
    }
    const timer = setTimeout(() => setSlow(true), 500);
    return () => clearTimeout(timer);
  }, [running]);
  return slow;
}
