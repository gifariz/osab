// Results against a benchmark's reference values, by `osab bench`'s rule. Used by the click tests
// (helpers.ts) and by the Vitest test of the WebAssembly solver.

import type { Results } from "../src/model/types";

export interface RefValue {
  case: string;
  kind: string;
  node?: string;
  dof?: string;
  element?: string;
  area?: string;
  /** "max" or "min" for a combination with both. */
  step?: string;
  at?: string | number;
  comp?: string;
  /** Modal values (`period`, `mass_ratio`, `mode_shape`): the mode. */
  mode?: number;
  /** Diaphragm centres (`centre`): the row, with `comp` one of mass_x, xcm, cum_mass_x, xccm, xcr and the y ones. */
  story?: string;
  diaphragm?: string;
  /** Drifts: "x" or "y". */
  dir?: string;
  /** Pier and spandrel forces (`pier`, `spandrel`): the label, with `story`, `at` (top, bottom, left, right) and `comp`. */
  label?: string;
  value: number;
}

/** Quantity type for the tolerance floor (the same classes as `osab bench`). */
function quantityClass(v: RefValue): string {
  if (v.kind === "period") return "period";
  if (v.kind === "centre") return v.comp!.includes("mass") ? "diaphragm mass" : "diaphragm centre";
  if (v.kind === "mass_ratio") return "mass ratio";
  if (v.kind === "mode_shape") return v.dof!.startsWith("u") ? "mode shape translation" : "mode shape rotation";
  if (v.kind === "disp") return v.dof!.startsWith("u") ? "translation" : "rotation";
  if (v.kind === "reaction" || v.kind === "base_reaction") return v.dof!.startsWith("f") ? "force" : "moment";
  if (v.kind === "joint_drift" || v.kind === "story_drift") return "drift";
  if (v.kind === "story_force") return v.comp!.startsWith("M") || v.comp === "T" ? "story moment" : "story force";
  if (v.kind === "pier" || v.kind === "spandrel") return v.comp!.startsWith("M") || v.comp === "T" ? "wall moment" : "wall force";
  if (v.kind === "shell") return v.comp!.startsWith("F") ? "membrane force" : v.comp!.startsWith("M") ? "plate moment" : "plate shear";
  return v.comp!.startsWith("M") || v.comp === "T" ? "moment" : "force";
}

/** Maps a benchmark id to the app's id (joints and members drawn by clicking get their own numbers). */
export interface IdMap {
  node?: (id: string) => string;
  element?: (id: string) => string;
  area?: (id: string) => string;
  /** A mesh element (`F1-3`) of a meshed area. */
  shell?: (id: string) => string;
}

export interface Check {
  label: string;
  got: number;
  want: number;
  /** Largest allowed |got - want|. */
  tolerance: number;
}

/**
 * Each reference value with the matching result (same rule as `osab bench`: 0.1% plus a floor of
 * 1e-5 times the largest value of the same quantity type). `element` names a member or a link; `shell`
 * values are an area's corner forces at joint `node`.
 */
export function referenceChecks(results: Results, refs: RefValue[], ids: IdMap = {}): Check[] {
  const same = (id: string) => id;
  const [node, element, area] = [ids.node ?? same, ids.element ?? same, ids.area ?? same];
  const largest = new Map<string, number>();
  for (const v of refs) largest.set(quantityClass(v), Math.max(largest.get(quantityClass(v)) ?? 0, Math.abs(v.value)));
  const dofs = ["ux", "uy", "uz", "rx", "ry", "rz"];
  // Lookups by id, built once (a building has tens of thousands of reference values).
  const memo = <K, V>(make: (k: K) => V) => {
    const m = new Map<K, V>();
    return (k: K) => {
      if (!m.has(k)) m.set(k, make(k));
      return m.get(k)!;
    };
  };
  const byKey = <T>(list: T[], key: (t: T) => string) => new Map(list.map((t) => [key(t), t]));
  const modeOf = (k: number) => results.modal?.modes.find((x) => x.mode === k);
  const shapes = memo((k: number) => byKey(modeOf(k)?.shape ?? [], (d) => d.node));
  const shapeAt = (v: RefValue) => {
    const d = shapes(v.mode!).get(node(v.node!)) as unknown as Record<string, number> | undefined;
    return d?.[v.dof!] ?? NaN;
  };
  type Case = (typeof results.cases)[number];
  const tables = memo((c: Case) => ({
    disp: byKey(c.displacements, (d) => d.node),
    react: byKey(c.reactions, (d) => d.node),
    members: byKey([...c.frames, ...(c.links ?? [])], (e) => e.element),
    shellEl: byKey(c.shells ?? [], (x) => x.element),
    shellArea: byKey(c.shells ?? [], (x) => x.area),
  }));
  // A mode shape's sign is free: each mode with the sign that fits the reference best.
  const dot = new Map<number, number>();
  for (const v of refs) if (v.kind === "mode_shape") dot.set(v.mode!, (dot.get(v.mode!) ?? 0) + shapeAt(v) * v.value);
  return refs.map((v) => {
    if (v.kind === "period" || v.kind === "mass_ratio" || v.kind === "mode_shape") {
      const md = modeOf(v.mode!);
      const got =
        v.kind === "period" ? (md?.period ?? NaN) : v.kind === "mass_ratio" ? (md?.mass_ratio[dofs.indexOf(v.dof!)] ?? NaN) : shapeAt(v) * ((dot.get(v.mode!) ?? 0) < 0 ? -1 : 1);
      return { label: `mode ${v.mode} ${v.kind} ${v.node ?? ""} ${v.dof ?? ""}`, got, want: v.value, tolerance: 1e-3 * Math.abs(v.value) + 1e-5 * largest.get(quantityClass(v))! + 1e-9 };
    }
    if (v.kind === "centre") {
      const row = results.diaphragm_centers?.find((x) => x.story === v.story && x.diaphragm === v.diaphragm) as unknown as Record<string, number | null> | undefined;
      return { label: `centres ${v.story} ${v.diaphragm} ${v.comp}`, got: row?.[v.comp!] ?? NaN, want: v.value, tolerance: 1e-3 * Math.abs(v.value) + 1e-5 * largest.get(quantityClass(v))! + 1e-9 };
    }
    const c = results.cases.find((x) => x.id === v.case && (x.step ?? null) === (v.step ?? null))!;
    const tb = tables(c);
    let got: number;
    const d = v.dir?.toLowerCase() === "y" ? 1 : 0;
    if (v.kind === "base_reaction") got = c.base_reaction?.[["fx", "fy", "fz", "mx", "my", "mz"].indexOf(v.dof!)] ?? NaN;
    // Joint drifts by size (a spectrum case's are sizes, a static case's signed).
    else if (v.kind === "joint_drift") got = Math.abs(c.joint_drifts?.find((j) => j.node === node(v.node!))?.drift[d] ?? NaN);
    else if (v.kind === "story_drift") got = c.story_drifts?.find((s) => s.story === v.story && s.dir.toLowerCase() === v.dir!.toLowerCase())?.drift ?? NaN;
    else if (v.kind === "story_force") {
      const f = c.story_forces?.find((s) => s.story === v.story);
      got = (v.at === "top" ? f?.top : f?.bottom)?.[["P", "VX", "VY", "T", "MX", "MY"].indexOf(v.comp!)] ?? NaN;
    } else if (v.kind === "pier" || v.kind === "spandrel") {
      const i = ["P", "V2", "V3", "T", "M2", "M3"].indexOf(v.comp!);
      const p = c.pier_forces?.find((x) => x.story === v.story && x.pier === v.label);
      const s = c.spandrel_forces?.find((x) => x.story === v.story && x.spandrel === v.label);
      const ends: Record<string, number[] | undefined> = { top: p?.top, bottom: p?.bottom, left: s?.left, right: s?.right };
      got = ends[String(v.at)]?.[i] ?? NaN;
    } else if (v.kind === "disp") got = (tb.disp.get(node(v.node!)) as unknown as Record<string, number>)[v.dof!];
    else if (v.kind === "reaction") got = (tb.react.get(node(v.node!)) as unknown as Record<string, number>)[v.dof!];
    else if (v.kind === "shell") {
      // A meshed area's values belong to one of its elements; an area analysed as one element is that element.
      const sh = v.element ? tb.shellEl.get((ids.shell ?? same)(v.element!))! : tb.shellArea.get(area(v.area!))!;
      got = (sh.corners.find((k) => k.node === node(v.node!)) as unknown as Record<string, number>)[v.comp!];
    } else {
      const id = element(v.element!);
      const er = tb.members.get(id)!;
      // "i" / "j": the end stations; a number: the station there (members with end offsets: the clear-length ends).
      const s =
        v.at === "i"
          ? er.stations[0]
          : v.at === "j"
            ? er.stations[er.stations.length - 1]
            : er.stations.reduce((best, st) => (Math.abs(st.x - Number(v.at)) < Math.abs(best.x - Number(v.at)) ? st : best));
      got = (s as unknown as Record<string, number>)[v.comp!];
    }
    const label = `${v.case}${v.step ? ` (${v.step})` : ""} ${v.kind} ${v.label ?? ""} ${v.story ?? ""} ${v.at ?? ""} ${v.area ?? ""} ${v.element ?? ""} ${v.node ?? ""} ${v.dof ?? v.comp ?? v.dir}`;
    return { label, got, want: v.value, tolerance: 1e-3 * Math.abs(v.value) + 1e-5 * largest.get(quantityClass(v))! + 1e-9 };
  });
}
