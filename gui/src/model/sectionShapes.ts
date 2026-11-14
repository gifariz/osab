// Frame section shapes as the model stores them (`section.shape`, the solver's `osab_mesh::section::Shape`):
// which dimensions each shape has (for Define > Frame Sections) and its outline (for drawing). The numbers
// (A, I, J, shear areas) come from the solver (command `section`); the mesh step works them out again
// from the shape before every run.

import type { Key } from "../i18n";
import type { Id, Model, Section, SectionShape } from "./types";

export type ShapeKind =
  | "rectangle"
  | "i"
  | "channel"
  | "tee"
  | "angle"
  | "double_angle"
  | "double_channel"
  | "box"
  | "pipe"
  | "circle"
  | "concrete_tee"
  | "concrete_l"
  | "concrete_pipe"
  | "cross"
  | "trapezoid"
  | "filled_tube"
  | "filled_pipe"
  | "encased_rectangle"
  | "encased_circle"
  | "aisc";

/** A section shape: lengths in the model's unit, mirror flags, the materials of a composite shape. */
export interface ShapeSpec {
  type: ShapeKind;
  depth?: number;
  width?: number;
  flange?: number;
  web?: number;
  bottom_width?: number;
  bottom_flange?: number;
  gap?: number;
  web_tip?: number;
  diameter?: number;
  wall?: number;
  mirror?: boolean;
  mirror2?: boolean;
  mirror3?: boolean;
  /** Composite: the steel (the section is transformed to it; frames with it use this material). */
  material?: Id;
  fill?: Id;
  encasement?: Id;
  /** Encased: the steel I-section inside. */
  core?: ShapeSpec;
  /** A steel list section: its AISC name, and its plain shape (dimensions in the model's unit) for drawing. */
  name?: string;
  like?: ShapeSpec;
}

export type LengthKey = "depth" | "width" | "flange" | "web" | "bottom_width" | "bottom_flange" | "gap" | "web_tip" | "diameter" | "wall";
export type FlagKey = "mirror" | "mirror2" | "mirror3";

/** Each shape's name, its length fields (with their labels), its mirror flags and composite materials. */
export const SHAPES: Record<Exclude<ShapeKind, "aisc">, { label: Key; lengths: [LengthKey, Key][]; flags: [FlagKey, Key][]; fill?: "fill" | "encasement" }> = {
  rectangle: { label: "sh.rectangle", lengths: [["depth", "df.depth2"], ["width", "df.width3"]], flags: [] },
  i: {
    label: "sh.i",
    lengths: [
      ["depth", "sh.depth"],
      ["width", "sh.topWidth"],
      ["flange", "sh.topFlange"],
      ["web", "sh.web"],
      ["bottom_width", "sh.bottomWidth"],
      ["bottom_flange", "sh.bottomFlange"],
    ],
    flags: [],
  },
  channel: { label: "sh.channel", lengths: [["depth", "sh.depth"], ["width", "sh.flangeWidth"], ["flange", "sh.flange"], ["web", "sh.web"]], flags: [["mirror", "sh.mirror2"]] },
  tee: { label: "sh.tee", lengths: [["depth", "sh.depth"], ["width", "sh.flangeWidth"], ["flange", "sh.flange"], ["web", "sh.web"]], flags: [["mirror", "sh.mirror3"]] },
  angle: {
    label: "sh.angle",
    lengths: [["depth", "sh.vertLeg"], ["width", "sh.horizLeg"], ["flange", "sh.horizThick"], ["web", "sh.vertThick"]],
    flags: [
      ["mirror2", "sh.mirror2"],
      ["mirror3", "sh.mirror3"],
    ],
  },
  double_angle: {
    label: "sh.doubleAngle",
    lengths: [["depth", "sh.vertLeg"], ["width", "sh.totalWidth"], ["flange", "sh.horizThick"], ["web", "sh.vertThick"], ["gap", "sh.gap"]],
    flags: [["mirror", "sh.mirror3"]],
  },
  double_channel: {
    label: "sh.doubleChannel",
    lengths: [["depth", "sh.depth"], ["width", "sh.totalWidth"], ["flange", "sh.flange"], ["web", "sh.web"], ["gap", "sh.gap"]],
    flags: [],
  },
  box: { label: "sh.box", lengths: [["depth", "sh.depth"], ["width", "sh.width"], ["flange", "sh.flange"], ["web", "sh.web"]], flags: [] },
  pipe: { label: "sh.pipe", lengths: [["diameter", "sh.diameter"], ["wall", "sh.wall"]], flags: [] },
  circle: { label: "sh.circle", lengths: [["diameter", "sh.diameter"]], flags: [] },
  concrete_tee: {
    label: "sh.concreteTee",
    lengths: [["depth", "sh.depth"], ["width", "sh.flangeWidth"], ["flange", "sh.flange"], ["web", "sh.webAtFlange"], ["web_tip", "sh.webAtTip"]],
    flags: [["mirror", "sh.mirror3"]],
  },
  concrete_l: {
    label: "sh.concreteL",
    lengths: [["depth", "sh.vertLeg"], ["width", "sh.horizLeg"], ["flange", "sh.horizThick"], ["web", "sh.webAtCorner"], ["web_tip", "sh.webAtTip"]],
    flags: [
      ["mirror2", "sh.mirror2"],
      ["mirror3", "sh.mirror3"],
    ],
  },
  concrete_pipe: { label: "sh.concretePipe", lengths: [["diameter", "sh.diameter"], ["wall", "sh.wall"]], flags: [] },
  cross: { label: "sh.cross", lengths: [["depth", "sh.depth"], ["width", "sh.width"], ["flange", "sh.horizBar"], ["web", "sh.vertBar"]], flags: [] },
  trapezoid: { label: "sh.trapezoid", lengths: [["depth", "sh.depth"], ["width", "sh.topWidth"], ["bottom_width", "sh.bottomWidth"]], flags: [] },
  filled_tube: { label: "sh.filledTube", lengths: [["depth", "sh.depth"], ["width", "sh.width"], ["flange", "sh.flange"], ["web", "sh.web"]], flags: [], fill: "fill" },
  filled_pipe: { label: "sh.filledPipe", lengths: [["diameter", "sh.diameter"], ["wall", "sh.wall"]], flags: [], fill: "fill" },
  encased_rectangle: { label: "sh.encasedRectangle", lengths: [["depth", "sh.depth"], ["width", "sh.width"]], flags: [], fill: "encasement" },
  encased_circle: { label: "sh.encasedCircle", lengths: [["diameter", "sh.diameter"]], flags: [], fill: "encasement" },
};

/** Lengths a shape may leave out (they then follow another). */
export const OPTIONAL_LENGTHS: LengthKey[] = ["bottom_width", "bottom_flange", "web_tip"];

/** A starting shape of each kind (lengths in metres), for a new section. */
export function defaultShape(kind: Exclude<ShapeKind, "aisc">): ShapeSpec {
  const I: ShapeSpec = { type: "i", depth: 0.4, width: 0.2, flange: 0.016, web: 0.01 };
  switch (kind) {
    case "rectangle":
      return { type: kind, depth: 0.5, width: 0.3 };
    case "i":
      return I;
    case "channel":
    case "tee":
      return { type: kind, depth: 0.3, width: 0.1, flange: 0.012, web: 0.008 };
    case "angle":
      return { type: kind, depth: 0.15, width: 0.1, flange: 0.012, web: 0.01 };
    case "double_angle":
      return { type: kind, depth: 0.15, width: 0.21, flange: 0.012, web: 0.01, gap: 0.01 };
    case "double_channel":
      return { type: kind, depth: 0.3, width: 0.22, flange: 0.012, web: 0.008, gap: 0.02 };
    case "box":
    case "filled_tube":
      return { type: kind, depth: 0.3, width: 0.2, flange: 0.012, web: 0.01 };
    case "pipe":
    case "filled_pipe":
      return { type: kind, diameter: 0.3, wall: 0.01 };
    case "circle":
      return { type: kind, diameter: 0.6 };
    case "concrete_tee":
      return { type: kind, depth: 0.6, width: 1.0, flange: 0.15, web: 0.3 };
    case "concrete_l":
      return { type: kind, depth: 0.6, width: 0.5, flange: 0.15, web: 0.3 };
    case "concrete_pipe":
      return { type: kind, diameter: 0.8, wall: 0.15 };
    case "cross":
      return { type: kind, depth: 0.8, width: 0.6, flange: 0.2, web: 0.25 };
    case "trapezoid":
      return { type: kind, depth: 0.6, width: 0.4, bottom_width: 0.3 };
    case "encased_rectangle":
      return { type: kind, depth: 0.6, width: 0.5, core: I };
    case "encased_circle":
      return { type: kind, diameter: 0.8, core: I };
  }
}

/** The shape of a section: its own, or the one the app recorded for an older file (rectangle, I). */
export function sectionShape(m: Model, sec: Section): ShapeSpec | undefined {
  if (sec.shape) return sec.shape;
  const old: SectionShape | undefined = m.gui?.sectionShapes?.[sec.id];
  if (!old) return undefined;
  if (old.shape === "rectangle") return { type: "rectangle", depth: old.depth, width: old.width };
  return { type: "i", depth: old.depth, width: old.flangeWidth, flange: old.flangeThickness, web: old.webThickness };
}

export type P2 = [number, number];

const rect = (h0: number, h1: number, v0: number, v1: number): P2[] => [
  [h0, v0],
  [h1, v0],
  [h1, v1],
  [h0, v1],
];

/** Mirrored (h -> -h, v -> -v), kept counterclockwise. */
function mirrored(p: P2[], h?: boolean, v?: boolean): P2[] {
  const q = p.map(([a, b]): P2 => [h ? -a : a, v ? -b : b]);
  return !!h !== !!v ? q.reverse() : q;
}

function circle(r: number, n = 48): P2[] {
  return Array.from({ length: n }, (_, k): P2 => [r * Math.cos((2 * Math.PI * k) / n), r * Math.sin((2 * Math.PI * k) / n)]);
}

function iOutline(s: ShapeSpec): P2[] {
  const d = s.depth ?? 0;
  const [bt, tft, tw] = [s.width ?? 0, s.flange ?? 0, s.web ?? 0];
  const [bb, tfb] = [s.bottom_width ?? bt, s.bottom_flange ?? tft];
  const [y0, y1] = [-d / 2, d / 2];
  return [
    [-bb / 2, y0],
    [bb / 2, y0],
    [bb / 2, y0 + tfb],
    [tw / 2, y0 + tfb],
    [tw / 2, y1 - tft],
    [bt / 2, y1 - tft],
    [bt / 2, y1],
    [-bt / 2, y1],
    [-bt / 2, y1 - tft],
    [-tw / 2, y1 - tft],
    [-tw / 2, y0 + tfb],
    [-bb / 2, y0 + tfb],
  ];
}

function angleOutline(d: number, b: number, tf: number, twc: number, twt: number): P2[] {
  return [
    [-b / 2, -d / 2],
    [b / 2, -d / 2],
    [b / 2, d / 2],
    [b / 2 - twt, d / 2],
    [b / 2 - twc, -d / 2 + tf],
    [-b / 2, -d / 2 + tf],
  ];
}

/**
 * A shape's outline from the middle of its box: [h along local 3, v along local 2] polygons,
 * counterclockwise, holes clockwise. `fill`: a composite shape's concrete, drawn apart.
 */
export function shapeOutline(s: ShapeSpec): { steel: P2[][]; fill: P2[][] } {
  if (s.type === "aisc") return s.like ? shapeOutline(s.like) : { steel: [], fill: [] };
  const d = s.depth ?? 0;
  const b = s.width ?? 0;
  const tf = s.flange ?? 0;
  const tw = s.web ?? 0;
  const hole = (p: P2[]) => [...p].reverse();
  switch (s.type) {
    case "rectangle":
      return { steel: [rect(-b / 2, b / 2, -d / 2, d / 2)], fill: [] };
    case "i":
      return { steel: [iOutline(s)], fill: [] };
    case "channel":
      return {
        steel: [
          mirrored(
            [
              [-b / 2, -d / 2],
              [b / 2, -d / 2],
              [b / 2, d / 2],
              [-b / 2, d / 2],
              [-b / 2, d / 2 - tf],
              [b / 2 - tw, d / 2 - tf],
              [b / 2 - tw, -d / 2 + tf],
              [-b / 2, -d / 2 + tf],
            ],
            s.mirror,
            false,
          ),
        ],
        fill: [],
      };
    case "tee":
      return {
        steel: [
          mirrored(
            [
              [-tw / 2, -d / 2],
              [tw / 2, -d / 2],
              [tw / 2, d / 2 - tf],
              [b / 2, d / 2 - tf],
              [b / 2, d / 2],
              [-b / 2, d / 2],
              [-b / 2, d / 2 - tf],
              [-tw / 2, d / 2 - tf],
            ],
            false,
            s.mirror,
          ),
        ],
        fill: [],
      };
    case "angle":
      return { steel: [mirrored(angleOutline(d, b, tf, tw, tw), s.mirror2, s.mirror3)], fill: [] };
    case "concrete_l":
      return { steel: [mirrored(angleOutline(d, b, tf, tw, s.web_tip ?? tw), s.mirror2, s.mirror3)], fill: [] };
    case "double_angle": {
      const g = (s.gap ?? 0) / 2;
      const right: P2[] = [
        [g, -d / 2],
        [g + tw, -d / 2],
        [g + tw, d / 2 - tf],
        [b / 2, d / 2 - tf],
        [b / 2, d / 2],
        [g, d / 2],
      ];
      return { steel: [mirrored(right, false, s.mirror), mirrored(mirrored(right, true, false), false, s.mirror)], fill: [] };
    }
    case "double_channel": {
      const g = (s.gap ?? 0) / 2;
      const right: P2[] = [
        [g, -d / 2],
        [b / 2, -d / 2],
        [b / 2, -d / 2 + tf],
        [g + tw, -d / 2 + tf],
        [g + tw, d / 2 - tf],
        [b / 2, d / 2 - tf],
        [b / 2, d / 2],
        [g, d / 2],
      ];
      return { steel: [right, mirrored(right, true, false)], fill: [] };
    }
    case "box":
      return { steel: [rect(-b / 2, b / 2, -d / 2, d / 2), hole(rect(-b / 2 + tw, b / 2 - tw, -d / 2 + tf, d / 2 - tf))], fill: [] };
    case "filled_tube":
      return {
        steel: [rect(-b / 2, b / 2, -d / 2, d / 2), hole(rect(-b / 2 + tw, b / 2 - tw, -d / 2 + tf, d / 2 - tf))],
        fill: [rect(-b / 2 + tw, b / 2 - tw, -d / 2 + tf, d / 2 - tf)],
      };
    case "pipe":
    case "concrete_pipe": {
      const r = (s.diameter ?? 0) / 2;
      return { steel: [circle(r), hole(circle(r - (s.wall ?? 0)))], fill: [] };
    }
    case "filled_pipe": {
      const r = (s.diameter ?? 0) / 2;
      return { steel: [circle(r), hole(circle(r - (s.wall ?? 0)))], fill: [circle(r - (s.wall ?? 0))] };
    }
    case "circle":
      return { steel: [circle((s.diameter ?? 0) / 2)], fill: [] };
    case "concrete_tee": {
      const twt = s.web_tip ?? tw;
      return {
        steel: [
          mirrored(
            [
              [-twt / 2, -d / 2],
              [twt / 2, -d / 2],
              [tw / 2, d / 2 - tf],
              [b / 2, d / 2 - tf],
              [b / 2, d / 2],
              [-b / 2, d / 2],
              [-b / 2, d / 2 - tf],
              [-tw / 2, d / 2 - tf],
            ],
            false,
            s.mirror,
          ),
        ],
        fill: [],
      };
    }
    case "cross": {
      const [h, v, a, c] = [b / 2, d / 2, tw / 2, tf / 2];
      return {
        steel: [
          [
            [-a, -v],
            [a, -v],
            [a, -c],
            [h, -c],
            [h, c],
            [a, c],
            [a, v],
            [-a, v],
            [-a, c],
            [-h, c],
            [-h, -c],
            [-a, -c],
          ],
        ],
        fill: [],
      };
    }
    case "trapezoid": {
      const bb = s.bottom_width ?? b;
      return {
        steel: [
          [
            [-bb / 2, -d / 2],
            [bb / 2, -d / 2],
            [b / 2, d / 2],
            [-b / 2, d / 2],
          ],
        ],
        fill: [],
      };
    }
    case "encased_rectangle":
    case "encased_circle": {
      const core = iOutline(s.core ?? { type: "i" });
      const outer = s.type === "encased_rectangle" ? rect(-b / 2, b / 2, -d / 2, d / 2) : circle((s.diameter ?? 0) / 2);
      return { steel: [core], fill: [outer, hole(core)] };
    }
  }
}

/** Signed area of a polygon (> 0 counterclockwise). */
export function polygonArea(p: P2[]): number {
  let a = 0;
  for (let k = 0; k < p.length; k++) {
    const [x0, y0] = p[k];
    const [x1, y1] = p[(k + 1) % p.length];
    a += x0 * y1 - x1 * y0;
  }
  return a / 2;
}

function inside(pt: P2, poly: P2[]): boolean {
  let c = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i];
    const [xj, yj] = poly[j];
    if (yi > pt[1] !== yj > pt[1] && pt[0] < ((xj - xi) * (pt[1] - yi)) / (yj - yi) + xi) c = !c;
  }
  return c;
}

/** Polygons grouped into faces: each counterclockwise outline with the clockwise holes inside it. */
export function outlineFaces(polys: P2[][]): { outer: P2[]; holes: P2[][] }[] {
  const faces = polys.filter((p) => polygonArea(p) > 0).map((outer) => ({ outer, holes: [] as P2[][] }));
  for (const h of polys.filter((p) => polygonArea(p) < 0)) {
    const probe: P2 = [h.reduce((s, p) => s + p[0], 0) / h.length, h.reduce((s, p) => s + p[1], 0) / h.length];
    faces.find((f) => inside(probe, f.outer))?.holes.push(h);
  }
  return faces;
}
