// Values on screen in a model's display units: input boxes, labels and tables. Model values are in
// the base units (kN, m, °C, t, s); `show` and `fmt` convert them for display, `read` converts typed text back.

import { fmt as fmtNumber, inputNumber } from "../scene/format";
import type { Component, Model, ShellView } from "../model/types";
import { fromDisplay, toDisplay, unitLabel, unitSystemOf, type Quantity, type UnitSystem } from "./units";

export interface UnitFormat {
  system: UnitSystem;
  /** The unit label, e.g. "kN/m²". */
  label(q: Quantity): string;
  /** A base value as the text of an input box, in display units ("" for undefined). */
  show(v: number | undefined, q: Quantity): string;
  /** Typed text in display units as a base value; null when it is not a number (blank: 0 with `blankZero`). */
  read(text: string, q: Quantity, blankZero?: boolean): number | null;
  /** A base value as a short number in display units (4 significant digits; "0" for round-off next to `ref`). */
  fmt(v: number, q: Quantity, ref?: number, digits?: number): string;
  /** A base value in display units. */
  value(v: number, q: Quantity): number;
}

export function unitFormat(system: UnitSystem): UnitFormat {
  return {
    system,
    label: (q) => unitLabel(q, system),
    show: (v, q) => (v === undefined || !Number.isFinite(v) ? "" : inputNumber(toDisplay(v, q, system))),
    read(text, q, blankZero = false) {
      const s = text.trim();
      if (s === "") return blankZero ? 0 : null;
      const v = Number(s);
      return Number.isFinite(v) ? fromDisplay(v, q, system) : null;
    },
    fmt: (v, q, ref = 0, digits = 4) => fmtNumber(toDisplay(v, q, system), toDisplay(ref, q, system), digits),
    value: (v, q) => toDisplay(v, q, system),
  };
}

/** The display units of a model (its saved setting, or SI). */
export function modelUnits(m: Model): UnitFormat {
  return unitFormat(unitSystemOf(m.gui?.units));
}

/** The quantity of a frame force component: P V2 V3 forces, T M2 M3 moments. */
export function frameQuantity(c: Component): Quantity {
  return c === "T" || c === "M2" || c === "M3" ? "moment" : "force";
}

/** The quantity of a shell force component: moments per length (M…) or forces per length. */
export function shellQuantity(c: ShellView): Quantity {
  return c.startsWith("M") ? "momentPerLength" : "forcePerLength";
}
