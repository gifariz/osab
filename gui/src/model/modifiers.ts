// Property modifiers on frame and area sections and objects (docs/MODEL_FORMAT.md): factors on
// stiffness terms, mass and weight; absent = 1; a section's and an object's multiply. Same rules as the
// solver (solver/src/model.rs, FrameModifiers and AreaModifiers).

import { AREA_MODIFIERS, FRAME_MODIFIERS, type AreaModifiers, type AreaSection, type FrameModifiers } from "./types";

/** How many of the leading names are stiffness factors (> 0); the rest (mass, weight) may be 0. */
const STIFFNESS = { frame: 6, area: 8 } as const;

/** What the Stiff slab type uses instead of the section's modifiers: bending x 100, no mass or weight. */
export const STIFF_MODIFIERS: AreaModifiers = { m11: 100, m22: 100, m12: 100, mass: 0, weight: 0 };

/** The numeric modifiers of `raw` that differ from 1, or undefined when there are none. */
export function readModifiers<K extends string>(names: readonly K[], raw: unknown): Partial<Record<K, number>> | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const out: Partial<Record<K, number>> = {};
  for (const k of names) {
    const v = (raw as Record<string, unknown>)[k];
    if (v === undefined || v === null) continue;
    const n = typeof v === "number" ? v : Number(v);
    if (n !== 1) out[k] = n;
  }
  return Object.keys(out).length ? out : undefined;
}

/** The first modifier with a value the solver refuses, with the rule it breaks, or null. */
export function modifierProblem(kind: "frame" | "area", mods: FrameModifiers | AreaModifiers | undefined): { name: string; zeroAllowed: boolean } | null {
  const names: readonly string[] = kind === "frame" ? FRAME_MODIFIERS : AREA_MODIFIERS;
  for (let k = 0; k < names.length; k++) {
    const v = (mods as Record<string, number> | undefined)?.[names[k]];
    if (v === undefined) continue;
    const zeroAllowed = k >= STIFFNESS[kind];
    if (!Number.isFinite(v) || (zeroAllowed ? v < 0 : v <= 0)) return { name: names[k], zeroAllowed };
  }
  return null;
}

/** `base` with the values of `set` written over it (undefined = keep), cleaned of 1s. */
export function withModifiers<K extends string>(names: readonly K[], base: Partial<Record<K, number>> | undefined, set: Partial<Record<K, number | undefined>>) {
  const out: Partial<Record<K, number>> = { ...(base ?? {}) };
  for (const k of names) if (set[k] !== undefined) out[k] = set[k];
  return readModifiers(names, out);
}

/** An area section's own modifiers as the analysis uses them (a Stiff section: fixed values). */
export function sectionAreaModifiers(s: AreaSection | undefined): AreaModifiers {
  if (s?.slab_type === "stiff") return STIFF_MODIFIERS;
  return s?.modifiers ?? {};
}

/** Text for a list of modifiers, e.g. "I33 0.35, J 0.1" ("" when all are 1). */
export function modifiersText(names: readonly string[], mods: Record<string, number> | undefined): string {
  return names
    .filter((k) => mods?.[k] !== undefined && mods[k] !== 1)
    .map((k) => `${k} ${Number(mods![k].toPrecision(6))}`)
    .join(", ");
}
