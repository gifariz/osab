// Right panel: diagrams along one member.

import { useId, useState } from "react";

import { t } from "../i18n";
import { dot } from "../model/geometry";
import type { Component, ElementResult, Station } from "../model/types";
import { caseOf } from "../scene/buildResultsScene";
import { fmt } from "../scene/format";
import { useStore } from "../state/store";
import { frameQuantity, useUnits } from "../units/format";

type Plane = "12" | "13" | "axial";

const W = 300;
const H = 110;
const PAD = { l: 8, r: 8, t: 10, b: 10 };

function localDisp(er: ElementResult, s: Station, k: 0 | 1 | 2) {
  return dot([s.ux, s.uy, s.uz], er.axes[k]);
}

function Chart(props: { title: string; unit: string; xs: number[]; values: number[]; length: number; cursor: number | null; onCursor(i: number | null): void }) {
  const { xs, values, length } = props;
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const max = Math.max(...values.map(Math.abs), 1e-30);
  const sx = (x: number) => PAD.l + (x / length) * (W - PAD.l - PAD.r);
  const sy = (v: number) => PAD.t + (H - PAD.t - PAD.b) * (0.5 - v / (2 * max));
  const pts = xs.map((x, i) => `${sx(x)},${sy(values[i])}`).join(" ");
  const area = `${sx(0)},${sy(0)} ${pts} ${sx(xs[xs.length - 1])},${sy(0)}`;
  let iMax = 0;
  let iMin = 0;
  values.forEach((v, i) => {
    if (v > values[iMax]) iMax = i;
    if (v < values[iMin]) iMin = i;
  });
  const near = (clientX: number, rect: DOMRect) => {
    const x = ((clientX - rect.left) / rect.width) * W;
    let best = 0;
    xs.forEach((xx, i) => {
      if (Math.abs(sx(xx) - x) < Math.abs(sx(xs[best]) - x)) best = i;
    });
    return best;
  };
  const c = props.cursor;
  return (
    <div className="chart">
      <div className="chart-title">
        <span>{props.title}</span>
        <span className="chart-value">{c !== null ? `x = ${fmt(xs[c])}: ${fmt(values[c], max)} ${props.unit}` : props.unit}</span>
      </div>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        onMouseMove={(e) => props.onCursor(near(e.clientX, e.currentTarget.getBoundingClientRect()))}
        onMouseLeave={() => props.onCursor(null)}
      >
        <defs>
          <clipPath id={`pos-${id}`}>
            <rect x={0} y={0} width={W} height={sy(0)} />
          </clipPath>
          <clipPath id={`neg-${id}`}>
            <rect x={0} y={sy(0)} width={W} height={H} />
          </clipPath>
        </defs>
        <polygon points={area} className="area-pos" clipPath={`url(#pos-${id})`} />
        <polygon points={area} className="area-neg" clipPath={`url(#neg-${id})`} />
        <line x1={sx(0)} y1={sy(0)} x2={sx(length)} y2={sy(0)} className="axis" />
        <polyline points={pts} className="curve" />
        {c !== null && <line x1={sx(xs[c])} y1={PAD.t / 2} x2={sx(xs[c])} y2={H - PAD.b / 2} className="cursor" />}
      </svg>
      <div className="chart-extremes">
        {t("md.extremes", { max: fmt(values[iMax], max), xmax: fmt(xs[iMax]), min: fmt(values[iMin], max), xmin: fmt(xs[iMin]) })}
      </div>
    </div>
  );
}

export function MemberDiagram() {
  const id = useStore((s) => s.memberPanel);
  const results = useStore((s) => s.results);
  const caseId = useStore((s) => s.display.caseId);
  const U = useUnits();
  const [plane, setPlane] = useState<Plane>("12");
  const [cursor, setCursor] = useState<number | null>(null);
  const c = results ? caseOf(results, caseId) : undefined;
  const er = c?.frames.find((e) => e.element === id) ?? c?.links?.find((e) => e.element === id);
  if (!er || !c) return null;
  // Charts plot values in display units, along the member in display distances.
  const xs = er.stations.map((s) => U.value(s.x, "distance"));
  const length = U.value(er.length, "distance");
  const fu = U.label("force");
  const mu = U.label("moment");
  const du = U.label("displacement");
  const comp = (k: Component) => er.stations.map((s) => U.value(s[k], frameQuantity(k)));
  const defl = (k: 0 | 1 | 2) => er.stations.map((s) => U.value(localDisp(er, s, k), "displacement"));
  const charts: { title: string; unit: string; values: number[] }[] =
    plane === "12"
      ? [
          { title: t("md.shearV2"), unit: fu, values: comp("V2") },
          { title: t("md.momentM3"), unit: mu, values: comp("M3") },
          { title: t("md.defl2"), unit: du, values: defl(1) },
        ]
      : plane === "13"
        ? [
            { title: t("md.shearV3"), unit: fu, values: comp("V3") },
            { title: t("md.momentM2"), unit: mu, values: comp("M2") },
            { title: t("md.defl3"), unit: du, values: defl(2) },
          ]
        : [
            { title: t("md.axialP"), unit: fu, values: comp("P") },
            { title: t("md.torsionT"), unit: mu, values: comp("T") },
            { title: t("md.defl1"), unit: du, values: defl(0) },
          ];
  return (
    <aside className="member-panel" data-testid="member-panel">
      <div className="member-head">
        <b>
          {er.element} — {c.id}
        </b>
        <button className="dialog-x" onClick={() => useStore.getState().setMemberPanel(null)} aria-label={t("md.close")}>
          ×
        </button>
      </div>
      <p className="hint note">
        {t("md.info", { i: er.nodes[0], j: er.nodes[1], length: U.fmt(er.length, "distance"), unit: U.label("distance") })}
      </p>
      <div className="tabs">
        <button className={plane === "12" ? "tab active" : "tab"} onClick={() => setPlane("12")}>
          {t("md.plane12")}
        </button>
        <button className={plane === "13" ? "tab active" : "tab"} onClick={() => setPlane("13")}>
          {t("md.plane13")}
        </button>
        <button className={plane === "axial" ? "tab active" : "tab"} onClick={() => setPlane("axial")}>
          {t("md.axial")}
        </button>
      </div>
      {charts.map((ch) => (
        <Chart key={ch.title} title={ch.title} unit={ch.unit} xs={xs} values={ch.values} length={length} cursor={cursor} onCursor={setCursor} />
      ))}
    </aside>
  );
}
