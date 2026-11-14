// Grid: X lines A, B, C…, Y lines 1, 2, 3…, stories Base, Story1, Story2…

import type { Grid, Vec3 } from "./types";

/** A, B, …, Z, AA, AB, … */
export function letters(i: number): string {
  let s = "";
  let n = i + 1;
  while (n > 0) {
    const r = (n - 1) % 26;
    s = String.fromCharCode(65 + r) + s;
    n = Math.floor((n - 1) / 26);
  }
  return s;
}

/**
 * Reads spacings such as "6, 4" or "3*4, 6" (3 spacings of 4). Returns null if the text is not
 * a list of positive numbers. An empty text gives an empty list.
 */
export function parseSpacings(text: string): number[] | null {
  const out: number[] = [];
  for (const part of text.split(/[,;\s]+/).filter((p) => p.length > 0)) {
    const m = /^(?:(\d+)\*)?([0-9]*\.?[0-9]+(?:e[-+]?\d+)?)$/i.exec(part);
    if (!m) return null;
    const count = m[1] ? parseInt(m[1], 10) : 1;
    const v = parseFloat(m[2]);
    if (!(v > 0) || count < 1 || count > 1000) return null;
    for (let i = 0; i < count; i++) out.push(v);
  }
  return out;
}

const cumulative = (spacings: number[]) => {
  const c = [0];
  for (const s of spacings) c.push(round(c[c.length - 1] + s));
  return c;
};

/** Rounds away binary noise from sums like 0.1 + 0.2. */
export const round = (v: number) => Math.round(v * 1e9) / 1e9;

export function makeGrid(xSpacings: number[], ySpacings: number[], storyHeights: number[]): Grid {
  return {
    x: cumulative(xSpacings).map((coord, i) => ({ label: letters(i), coord })),
    y: cumulative(ySpacings).map((coord, i) => ({ label: String(i + 1), coord })),
    stories: cumulative(storyHeights).map((z, i) => ({ label: i === 0 ? "Base" : `Story${i}`, z })),
  };
}

/** Every grid intersection on every story. */
export function gridPoints(grid: Grid): Vec3[] {
  const pts: Vec3[] = [];
  for (const s of grid.stories) for (const x of grid.x) for (const y of grid.y) pts.push([x.coord, y.coord, s.z]);
  return pts;
}

/** Spacings between consecutive coordinates, as text for the grid dialog ("6, 4"). */
export function spacingText(coords: number[]): string {
  const out: number[] = [];
  for (let i = 1; i < coords.length; i++) out.push(round(coords[i] - coords[i - 1]));
  return out.join(", ");
}
