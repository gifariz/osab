// Deck area sections: concrete on a steel deck (filled), the steel deck alone (unfilled) or a solid slab
// analysed as a deck. A deck is always a one-way Membrane floor spanning along local axis 1. The same rules
// as the solver's `Deck` (solver/src/model.rs): weight = concrete per area x the fill's unit weight + the
// deck's unit weight; in-plane stiffness from the concrete above the ribs, the slab, or the shear thickness.

import type { Key } from "../i18n";
import type { AreaPropertyType, AreaSection, Deck, DeckType, Model } from "./types";

export const DECK_TYPES: [DeckType, Key][] = [
  ["filled", "df.deckFilled"],
  ["unfilled", "df.deckUnfilled"],
  ["solid", "df.deckSolid"],
];

/** The label of each property type. */
export const PROPERTY_TYPE_KEYS: Record<AreaPropertyType, Key> = {
  slab: "df.propertySlab",
  deck: "df.propertyDeck",
  wall: "df.propertyWall",
};

/** A new deck's values (kN, m): a filled deck of 65 mm concrete on 75 mm ribs. */
export function newDeck(steel: string): Deck {
  return {
    type: "filled",
    steel,
    slab_depth: 0.065,
    rib_depth: 0.075,
    rib_width_top: 0.15,
    rib_width_bottom: 0.12,
    rib_spacing: 0.3,
    shear_thickness: 0.001,
    unit_weight: 0.1,
  };
}

/** Concrete volume per unit area: the slab above the ribs plus the ribs; none on an unfilled deck. */
export function deckConcrete(d: Deck): number {
  if (d.type === "unfilled") return 0;
  if (d.type === "solid") return d.slab_depth;
  return d.slab_depth + (d.rib_depth * 0.5 * (d.rib_width_top + d.rib_width_bottom)) / d.rib_spacing;
}

/** Total depth: slab depth + rib depth (a solid slab: its depth). */
export function deckDepth(d: Deck): number {
  return d.type === "solid" ? d.slab_depth : d.slab_depth + d.rib_depth;
}

/** A section's weight per unit area (force / area): material weight x thickness, or the deck rule. */
export function sectionWeight(m: Model, s: AreaSection): number {
  const w = m.materials.find((x) => x.id === s.material)?.weight ?? 0;
  return s.deck ? w * deckConcrete(s.deck) + s.deck.unit_weight : w * s.thickness;
}

/** What is wrong with the deck's values (a text key), or null. */
export function deckProblem(d: Deck): Key | null {
  const values = [d.slab_depth, d.rib_depth, d.rib_width_top, d.rib_width_bottom, d.rib_spacing, d.shear_thickness, d.unit_weight];
  if (values.some((v) => !(Number.isFinite(v) && v >= 0))) return "err.deckNegative";
  if (d.type === "filled" && !(d.slab_depth > 0 && d.rib_spacing > 0)) return "err.deckFilled";
  if (d.type === "solid" && !(d.slab_depth > 0)) return "err.deckSolid";
  if (d.type === "unfilled" && !(d.shear_thickness > 0)) return "err.deckUnfilled";
  return null;
}

/** A deck read from a file, or null if it is not one. Missing lengths are 0. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function readDeck(raw: any): Deck | null {
  if (!raw || typeof raw !== "object" || !DECK_TYPES.some(([k]) => k === raw.type)) return null;
  const num = (v: unknown) => (Number.isFinite(Number(v)) ? Number(v) : 0);
  return {
    type: raw.type,
    steel: String(raw.steel ?? ""),
    slab_depth: num(raw.slab_depth),
    rib_depth: num(raw.rib_depth),
    rib_width_top: num(raw.rib_width_top),
    rib_width_bottom: num(raw.rib_width_bottom),
    rib_spacing: num(raw.rib_spacing),
    shear_thickness: num(raw.shear_thickness),
    unit_weight: num(raw.unit_weight),
  };
}
