// Display units. Every model is stored and solved in one base unit system: kN, m, °C, with mass in t
// (kN·s²/m) and time in s. The app shows and reads values in one of three display systems, SI, MKS
// or US, each a fixed unit per quantity (docs/phases/UNITS_PLAN.md; the sets follow
// common defaults). Values are converted only on the way to and
// from the screen, so switching systems never changes the model.

export type UnitSystem = "SI" | "MKS" | "US";
export const UNIT_SYSTEMS: readonly UnitSystem[] = ["SI", "MKS", "US"];
export const DEFAULT_UNIT_SYSTEM: UnitSystem = "SI";

/** What a value measures. Each quantity has one unit in each display system. */
export type Quantity =
  | "distance" // coordinates, grid lines, story heights, stations, offsets, mesh sizes
  | "structureArea"
  | "angle"
  | "sectionLength" // section dimensions, slab and wall thickness
  | "sectionArea" // A, As2, As3
  | "length3"
  | "length4" // I22, I33, J
  | "length6"
  | "displacement"
  | "rotation"
  | "force"
  | "forcePerLength" // frame line loads; shell F11 F22 F12 V13 V23
  | "forcePerArea" // area loads
  | "moment"
  | "momentPerLength" // shell M11 M22 M12
  | "temperature"
  | "temperatureChange"
  | "stress"
  | "strain"
  | "modulus" // E, G
  | "transStiffness"
  | "rotStiffness"
  | "lineSpring" // line spring: force/length per length
  | "lineRotSpring" // line spring about the axis: force·length/rad per length
  | "areaSpring" // area spring (subgrade modulus): force/length per area
  | "acceleration" // spectrum case scale factors
  | "period"
  | "frequency"
  | "circularFrequency"
  | "mass"
  | "massPerVolume"
  | "weight"
  | "weightPerLength"
  | "weightPerArea"
  | "weightPerVolume" // material weight per volume
  | "rotationalInertia" // mass moments of inertia
  | "thermalCoefficient";

export const QUANTITIES: readonly Quantity[] = [
  "distance", "structureArea", "angle", "sectionLength", "sectionArea", "length3", "length4", "length6",
  "displacement", "rotation", "force", "forcePerLength", "forcePerArea", "moment", "momentPerLength",
  "temperature", "temperatureChange", "stress", "strain", "modulus", "transStiffness", "rotStiffness",
  "lineSpring", "lineRotSpring", "areaSpring", "acceleration", "period", "frequency", "circularFrequency", "mass", "massPerVolume", "weight", "weightPerLength",
  "weightPerArea", "weightPerVolume", "rotationalInertia", "thermalCoefficient",
];

/** A display unit: base value = shown value x factor + offset (the offset is only for °F). */
export interface Unit {
  label: string;
  factor: number;
  offset?: number;
  /** Decimal places shown by default (the common defaults of each system). */
  decimals: number;
}

// Exact definitions in base units (m, kN).
const m = 1, mm = 0.001, cm = 0.01, ft = 0.3048, inch = 0.0254;
const kN = 1, N = 0.001, kgf = 9.80665e-3, tonf = 9.80665, lb = 4.4482216152605e-3, kip = 4.4482216152605;

const u = (label: string, factor: number, decimals: number, offset?: number): Unit =>
  offset === undefined ? { label, factor, decimals } : { label, factor, decimals, offset };

const SETS: Record<UnitSystem, Record<Quantity, Unit>> = {
  SI: {
    distance: u("m", m, 4),
    structureArea: u("m²", m * m, 2),
    angle: u("°", 1, 3),
    sectionLength: u("mm", mm, 1),
    sectionArea: u("cm²", cm ** 2, 1),
    length3: u("cm³", cm ** 3, 1),
    length4: u("cm⁴", cm ** 4, 1),
    length6: u("cm⁶", cm ** 6, 1),
    displacement: u("mm", mm, 3),
    rotation: u("rad", 1, 6),
    force: u("kN", kN, 4),
    forcePerLength: u("kN/m", kN / m, 3),
    forcePerArea: u("kN/m²", kN / m ** 2, 3),
    moment: u("kN·m", kN * m, 4),
    momentPerLength: u("kN·m/m", kN, 3),
    temperature: u("°C", 1, 3),
    temperatureChange: u("°C", 1, 3),
    stress: u("MPa", 1000, 2),
    strain: u("mm/mm", 1, 6),
    modulus: u("MPa", 1000, 2),
    transStiffness: u("kN/m", kN / m, 3),
    rotStiffness: u("kN·m/rad", kN * m, 4),
    lineSpring: u("kN/m/m", kN / m ** 2, 3),
    lineRotSpring: u("kN·m/rad/m", kN, 3),
    areaSpring: u("kN/m³", kN / m ** 3, 3),
    acceleration: u("m/s²", m, 5),
    period: u("s", 1, 3),
    frequency: u("cyc/s", 1, 3),
    circularFrequency: u("rad/s", 1, 3),
    mass: u("kg", 0.001, 2),
    massPerVolume: u("kg/m³", 0.001, 3),
    weight: u("kN", kN, 4),
    weightPerLength: u("N/m", N / m, 2),
    weightPerArea: u("kN/m²", kN / m ** 2, 4),
    weightPerVolume: u("kN/m³", kN / m ** 3, 4),
    rotationalInertia: u("t·m²", 1, 4),
    thermalCoefficient: u("1/°C", 1, 8),
  },
  MKS: {
    distance: u("m", m, 4),
    structureArea: u("m²", m * m, 2),
    angle: u("°", 1, 3),
    sectionLength: u("mm", mm, 1),
    sectionArea: u("mm²", mm ** 2, 1),
    length3: u("mm³", mm ** 3, 1),
    length4: u("mm⁴", mm ** 4, 1),
    length6: u("mm⁶", mm ** 6, 1),
    displacement: u("mm", mm, 3),
    rotation: u("rad", 1, 6),
    force: u("tonf", tonf, 4),
    forcePerLength: u("tonf/m", tonf / m, 3),
    forcePerArea: u("kgf/m²", kgf / m ** 2, 2),
    moment: u("tonf·m", tonf * m, 4),
    momentPerLength: u("tonf·m/m", tonf, 4),
    temperature: u("°C", 1, 3),
    temperatureChange: u("°C", 1, 3),
    stress: u("kgf/mm²", kgf / mm ** 2, 2),
    strain: u("mm/mm", 1, 6),
    modulus: u("kgf/mm²", kgf / mm ** 2, 2),
    transStiffness: u("tonf/mm", tonf / mm, 5),
    rotStiffness: u("tonf·mm/rad", tonf * mm, 2),
    lineSpring: u("tonf/m/m", tonf / m ** 2, 3),
    lineRotSpring: u("tonf·m/rad/m", tonf, 3),
    areaSpring: u("kgf/cm³", kgf / cm ** 3, 4),
    acceleration: u("m/s²", m, 5),
    period: u("s", 1, 3),
    frequency: u("cyc/s", 1, 3),
    circularFrequency: u("rad/s", 1, 3),
    mass: u("kg", 0.001, 2),
    massPerVolume: u("kg/m³", 0.001, 3),
    weight: u("tonf", tonf, 5),
    weightPerLength: u("kgf/m", kgf / m, 2),
    weightPerArea: u("kgf/m²", kgf / m ** 2, 2),
    weightPerVolume: u("kgf/m³", kgf / m ** 3, 2),
    rotationalInertia: u("tonf·m·s²", tonf * m, 4),
    thermalCoefficient: u("1/°C", 1, 8),
  },
  US: {
    distance: u("ft", ft, 4),
    structureArea: u("ft²", ft ** 2, 2),
    angle: u("°", 1, 3),
    sectionLength: u("in", inch, 4),
    sectionArea: u("in²", inch ** 2, 2),
    length3: u("in³", inch ** 3, 2),
    length4: u("in⁴", inch ** 4, 2),
    length6: u("in⁶", inch ** 6, 2),
    displacement: u("in", inch, 6),
    rotation: u("rad", 1, 6),
    force: u("kip", kip, 3),
    forcePerLength: u("kip/ft", kip / ft, 3),
    forcePerArea: u("lb/ft²", lb / ft ** 2, 2),
    moment: u("kip·ft", kip * ft, 4),
    momentPerLength: u("kip·ft/ft", kip, 3),
    temperature: u("°F", 5 / 9, 3, -32 * (5 / 9)),
    temperatureChange: u("°F", 5 / 9, 3),
    stress: u("lb/in²", lb / inch ** 2, 2),
    strain: u("in/in", 1, 6),
    modulus: u("lb/in²", lb / inch ** 2, 2),
    transStiffness: u("kip/in", kip / inch, 4),
    rotStiffness: u("kip·in/rad", kip * inch, 3),
    lineSpring: u("kip/in/in", kip / inch ** 2, 4),
    lineRotSpring: u("kip·in/rad/in", kip, 3),
    areaSpring: u("lb/in³", lb / inch ** 3, 3),
    acceleration: u("in/s²", inch, 3),
    period: u("s", 1, 3),
    frequency: u("cyc/s", 1, 3),
    circularFrequency: u("rad/s", 1, 3),
    mass: u("lb·s²/ft", lb / ft, 2),
    massPerVolume: u("lb·s²/ft⁴", lb / ft ** 4, 3),
    weight: u("kip", kip, 4),
    weightPerLength: u("lb/ft", lb / ft, 2),
    weightPerArea: u("lb/ft²", lb / ft ** 2, 2),
    weightPerVolume: u("lb/ft³", lb / ft ** 3, 2),
    rotationalInertia: u("kip·ft·s²", kip * ft, 4),
    thermalCoefficient: u("1/°F", 1.8, 8),
  },
};

/** The display unit of a quantity in a system. */
export function unitOf(q: Quantity, system: UnitSystem): Unit {
  return SETS[system][q];
}

/** The unit label shown next to a value, e.g. "kN/m²". */
export function unitLabel(q: Quantity, system: UnitSystem): string {
  return SETS[system][q].label;
}

/** A base value (kN, m, °C, t, s) in the display unit of `q`. */
export function toDisplay(value: number, q: Quantity, system: UnitSystem): number {
  const unit = SETS[system][q];
  return (value - (unit.offset ?? 0)) / unit.factor;
}

/** A value typed in the display unit of `q`, in base units. */
export function fromDisplay(value: number, q: Quantity, system: UnitSystem): number {
  const unit = SETS[system][q];
  return value * unit.factor + (unit.offset ?? 0);
}

/** A model's display system from its saved setting (missing or unknown: SI). */
export function unitSystemOf(saved: unknown): UnitSystem {
  return UNIT_SYSTEMS.includes(saved as UnitSystem) ? (saved as UnitSystem) : DEFAULT_UNIT_SYSTEM;
}
