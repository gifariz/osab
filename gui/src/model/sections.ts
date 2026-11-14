// Section property calculators. Local 2 is the depth direction, local 3 the width direction.
// Checked against reference values: A and I exact; rectangle shear areas exact and
// J within 0.2%; I-section J within 0.2% and shear areas within 5% (the reference uses another method).

import { t } from "../i18n";
import type { Model, Section, SectionShape } from "./types";

export type SectionProps = Pick<Section, "A" | "I22" | "I33" | "J" | "As2" | "As3">;

/** Solid rectangle: depth (along local 2) and width (along local 3). */
export function rectangle(depth: number, width: number): SectionProps {
  const [a, b] = depth >= width ? [depth, width] : [width, depth];
  const area = depth * width;
  return {
    A: area,
    I33: (width * depth ** 3) / 12,
    I22: (depth * width ** 3) / 12,
    J: plateTorsion(a, b),
    As2: (5 / 6) * area,
    As3: (5 / 6) * area,
  };
}

/** Torsion constant of a solid rectangle, long side a, short side b (Roark). */
function plateTorsion(a: number, b: number): number {
  return a * b ** 3 * (1 / 3 - 0.21 * (b / a) * (1 - b ** 4 / (12 * a ** 4)));
}

/** Doubly symmetric I-section: total depth, flange width, flange thickness, web thickness. */
export function iSection(depth: number, flangeWidth: number, flangeThickness: number, webThickness: number): SectionProps {
  const [d, bf, tf, tw] = [depth, flangeWidth, flangeThickness, webThickness];
  const hw = d - 2 * tf;
  const I33 = (bf * d ** 3 - (bf - tw) * hw ** 3) / 12;
  const I22 = (2 * tf * bf ** 3 + hw * tw ** 3) / 12;
  // Torsion: two flanges, the web and the two flange-web junctions (AISC Design Guide 9, no fillets).
  const [t, t1] = [Math.min(tf, tw), Math.max(tf, tw)];
  const D = (tf ** 2 + tw ** 2 / 4) / tf;
  const J = 2 * plateTorsion(bf, tf) + (hw * tw ** 3) / 3 + 2 * (0.15 * t / t1) * D ** 4;
  return {
    A: 2 * bf * tf + hw * tw,
    I33,
    I22,
    J,
    // Shear areas from the shear-flow (energy) definition As = I^2 / ∫ Q^2 / b.
    As2: shearArea(I33, d / 2, (y) => (y > d / 2 - tf ? bf : tw)),
    As3: shearArea(I22, bf / 2, (z) => (z < tw / 2 ? d : 2 * tf)),
  };
}

/**
 * Shear area of a section symmetric about its centroid: I^2 / ∫ Q(y)^2 / b(y) dy over -c..c,
 * where b(y) is the width at y (given for y >= 0) and Q(y) the first moment of the area beyond y.
 */
function shearArea(I: number, c: number, width: (y: number) => number): number {
  const n = 4000;
  const h = c / n;
  // Q from the outer edge inwards (trapezoid on b*y, exact for piecewise-constant widths with fine steps).
  const q = new Float64Array(n + 1);
  for (let i = n - 1; i >= 0; i--) {
    const y0 = i * h;
    const y1 = y0 + h;
    q[i] = q[i + 1] + width((y0 + y1) / 2) * (y1 * y1 - y0 * y0) / 2;
  }
  let integral = 0;
  for (let i = 0; i < n; i++) {
    const qm = (q[i] + q[i + 1]) / 2;
    integral += (qm * qm) / width((i + 0.5) * h) * h;
  }
  return (I * I) / (2 * integral);
}

/** A rectangle of a section drawing: y along local 2 (depth), z along local 3 (width), from the centroid. */
export interface SectionRect {
  y0: number;
  y1: number;
  z0: number;
  z1: number;
}

const centred = (depth: number, width: number, y = 0): SectionRect => ({ y0: y - depth / 2, y1: y + depth / 2, z0: -width / 2, z1: width / 2 });

/** Rectangles that draw a section shape. */
export function shapeRects(s: SectionShape): SectionRect[] {
  if (s.shape === "rectangle") return [centred(s.depth, s.width)];
  const web = s.depth - 2 * s.flangeThickness;
  const yf = (s.depth - s.flangeThickness) / 2;
  return [centred(s.flangeThickness, s.flangeWidth, yf), centred(web, s.webThickness), centred(s.flangeThickness, s.flangeWidth, -yf)];
}

/**
 * Rectangles that draw a frame section: its shape when it was made from dimensions, otherwise the
 * rectangle with the same A and I33.
 */
export function sectionRects(m: Model, sec: Section): SectionRect[] {
  const shape = m.gui?.sectionShapes?.[sec.id];
  if (shape) return shapeRects(shape);
  let depth = Math.sqrt((12 * sec.I33) / sec.A);
  if (!(depth > 0) || !Number.isFinite(depth)) depth = Math.sqrt(Math.max(sec.A, 0));
  return [centred(depth, depth > 0 ? sec.A / depth : 0)];
}

/** Reasons the shape dimensions are not usable, or null when fine. */
export function iSectionProblem(depth: number, flangeWidth: number, flangeThickness: number, webThickness: number): string | null {
  if (![depth, flangeWidth, flangeThickness, webThickness].every((v) => v > 0)) return t("df.iDimsPositive");
  if (2 * flangeThickness >= depth) return t("df.iFlangesThick");
  if (webThickness > flangeWidth) return t("df.iWebWide");
  return null;
}
