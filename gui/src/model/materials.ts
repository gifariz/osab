// Material types: Steel, Concrete or Other. The type is a label with its own strengths (steel Fy and Fu,
// concrete f'c, none for other materials); the analysis uses E, G or ν and the unit weight only, so the type
// and strengths change no result. They are kept for exchanging models and picking sensible defaults.

import type { Key } from "../i18n";
import type { Material, MaterialType, Model } from "./types";

export const MATERIAL_TYPES: [MaterialType, Key][] = [
  ["steel", "df.matSteel"],
  ["concrete", "df.matConcrete"],
  ["other", "df.matOther"],
];

/** The strengths each type holds, in the order shown. */
export const STRENGTHS: Record<MaterialType, ("fy" | "fu" | "fc")[]> = {
  steel: ["fy", "fu"],
  concrete: ["fc"],
  other: [],
};

/**
 * The type of a material from a file written before material types (kN, m): E of 150 GPa or more is steel,
 * 10 to 60 GPa concrete, anything else other.
 */
export function guessMaterialType(E: number): MaterialType {
  if (E >= 150e6) return "steel";
  if (E >= 10e6 && E <= 60e6) return "concrete";
  return "other";
}

/** A material's type (missing in older files: guessed from E). */
export function materialType(m: Material): MaterialType {
  return m.type ?? guessMaterialType(m.E);
}

/** The first material of a type, by name order in the model, or undefined. */
export function firstOfType(m: Model, type: MaterialType): string | undefined {
  return m.materials.find((x) => materialType(x) === type)?.id;
}
