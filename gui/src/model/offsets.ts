// End length offsets (the solver's Model::end_offsets, docs/phases/PHASE4_PLAN.md probe
// findings 2a): given lengths, or the automatic ones from the connected columns and beams. Used to draw
// the offsets and list them in the tables; the analysis uses the solver's own copy of the rule.

import { t } from "../i18n";
import { pos, sub } from "./geometry";
import { angleLimitsOf } from "./objectTypes";
import type { Frame, Id, Model } from "./types";

type Kind = "column" | "beam" | "other";

/**
 * End offset lengths of every frame at end i and end j, by id:
 * - a beam end where a column ends from below (the column's upper end): (t3/2) cos^2 a + (t2/2) sin^2 a of
 *   that column (the largest of several), a = angle between the beam and the column's local 2 axis;
 * - a column's upper end: the largest, over the beams there, of the beam's depth t3 x max(cos^2 a, sin^2 a);
 *   a column's lower end: 0;
 * - braces, trusses, null frames: 0.
 */
export function endOffsets(m: Model): Map<Id, [number, number]> {
  const nodes = new Map(m.nodes.map((n) => [n.id, n]));
  const sections = new Map(m.sections.map((s) => [s.id, s]));
  const lim = angleLimitsOf(m);
  const dims = (f: Frame): [number, number] => {
    const s = sections.get(f.section);
    return [s?.depth ?? 0, s?.width ?? 0];
  };
  const info = new Map<Id, { kind: Kind; upper: number; dir: [number, number] }>();
  for (const f of m.frames) {
    const a = nodes.get(f.nodes[0]);
    const b = nodes.get(f.nodes[1]);
    if (!a || !b) continue;
    const d = sub(pos(b), pos(a));
    const h = Math.hypot(d[0], d[1]);
    const tilt = (Math.atan2(Math.abs(d[2]), h) * 180) / Math.PI;
    const kind: Kind =
      !f.section || f.type === "truss" ? "other" : 90 - tilt <= lim.column + 1e-9 ? "column" : tilt <= lim.beam_floor + 1e-9 ? "beam" : "other";
    info.set(f.id, { kind, upper: d[2] >= 0 ? 1 : 0, dir: h > 0 ? [d[0] / h, d[1] / h] : [0, 0] });
  }
  // cos^2 of the angle between a horizontal direction and a column's local 2 axis.
  const cos2 = (dir: [number, number], col: Frame) => {
    const a = ((col.angle ?? 0) * Math.PI) / 180;
    const c = dir[0] * Math.cos(a) + dir[1] * Math.sin(a);
    return Math.min(c * c, 1);
  };
  const at = new Map<Id, Frame[]>();
  for (const f of m.frames) for (const n of f.nodes) at.set(n, [...(at.get(n) ?? []), f]);
  const out = new Map<Id, [number, number]>();
  for (const f of m.frames) {
    const o = f.end_offsets;
    const me = info.get(f.id);
    if (!o || !me) {
      out.set(f.id, [0, 0]);
      continue;
    }
    if (o !== "auto") {
      out.set(f.id, [o.i, o.j]);
      continue;
    }
    const length = (end: 0 | 1) => {
      const joint = f.nodes[end];
      let best = 0;
      for (const c of at.get(joint) ?? []) {
        const ci = info.get(c.id);
        if (!ci) continue;
        if (me.kind === "beam" && ci.kind === "column" && c.nodes[ci.upper] === joint) {
          // Pointing from this end into the beam.
          const d: [number, number] = end === 0 ? me.dir : [-me.dir[0], -me.dir[1]];
          const [t3, t2] = dims(c);
          const k = cos2(d, c);
          best = Math.max(best, (t3 / 2) * k + (t2 / 2) * (1 - k));
        } else if (me.kind === "column" && end === me.upper && ci.kind === "beam" && c.nodes.includes(joint)) {
          const k = cos2(ci.dir, f);
          best = Math.max(best, dims(c)[0] * Math.max(k, 1 - k));
        }
      }
      return best;
    };
    out.set(f.id, [length(0), length(1)]);
  }
  return out;
}

/** A frame's end offsets for the tables: "Auto: 0.25 / 0.25, RZ 0.5", "0.4 / 0.4" or ""; `show` turns a length into display units. */
export function offsetText(f: Frame, lengths: [number, number] | undefined, show: (v: number) => number = (v) => v): string {
  if (!f.end_offsets) return "";
  const [i, j] = (lengths ?? [0, 0]).map((v) => +show(v).toFixed(6));
  const rz = f.rigid_zone ? `, ${t("off.rz")} ${f.rigid_zone}` : "";
  return `${f.end_offsets === "auto" ? `${t("off.auto")}: ` : ""}${i} / ${j}${rz}`;
}
