// What the model view shows (View > Set Display Options, Ctrl+W): texts and symbols on each object
// kind, the object types drawn, colours and special effects. Saved in the model file's `gui.viewOptions`
// so a model reopens looking the same; missing or unknown entries read as the defaults.

/** Colours of members and areas: by section, by object type or by material. */
export type ColorBy = "sections" | "types" | "materials";
export const COLOR_BY: ColorBy[] = ["sections", "types", "materials"];

export interface ViewOptions {
  colorBy: ColorBy;
  /** Object types not drawn ("joint", "link", "frame:column", "area:opening"…). */
  hiddenTypes: string[];
  /** Joints drawn without their dot (unless selected or under the mouse): still picked, their symbols, loads and texts still drawn. */
  jointsInvisible: boolean;
  /** Members and areas drawn a little smaller, so their ends and edges stand apart. */
  shrink: boolean;
  /** Areas drawn with a see-through face (off: edges only). */
  fill: boolean;
  /** Members drawn with their section. */
  extrudeFrames: boolean;
  /** Areas drawn with their thickness. */
  extrudeAreas: boolean;
  /** Support and spring symbols at the joints. */
  supports: boolean;
  /** Rigid diaphragms: their joints coloured, lines from the outer joints to the centre, the name there. */
  diaphragms: boolean;
  /** Grid lines, bubbles and story levels. */
  grid: boolean;
  /** The analysis mesh drawn on the areas. */
  mesh: boolean;
  /** The 3D view in parallel projection (no perspective). */
  parallel: boolean;
  joint: { labels: boolean; ids: boolean; masses: boolean };
  frame: { labels: boolean; ids: boolean; sections: boolean; modifiers: boolean; releases: boolean; offsets: boolean; insertion: boolean; springs: boolean; piers: boolean; spandrels: boolean; localAxes: boolean };
  area: { labels: boolean; ids: boolean; sections: boolean; modifiers: boolean; insertion: boolean; springs: boolean; piers: boolean; spandrels: boolean; localAxes: boolean };
  link: { labels: boolean; ids: boolean; properties: boolean; localAxes: boolean };
}

export type ObjectKind = "joint" | "frame" | "area" | "link";
export const OBJECT_KINDS: ObjectKind[] = ["joint", "frame", "area", "link"];

/** Off switches of each object kind's items. */
const off = <K extends string>(keys: readonly K[]) => Object.fromEntries(keys.map((k) => [k, false])) as Record<K, boolean>;

export const OBJECT_ITEMS = {
  joint: ["labels", "ids", "masses"],
  frame: ["labels", "ids", "sections", "modifiers", "releases", "offsets", "insertion", "springs", "piers", "spandrels", "localAxes"],
  area: ["labels", "ids", "sections", "modifiers", "insertion", "springs", "piers", "spandrels", "localAxes"],
  link: ["labels", "ids", "properties", "localAxes"],
} as const satisfies { [K in ObjectKind]: readonly (keyof ViewOptions[K])[] };

/** Switches that are not per object kind, in dialog order. */
export const GENERAL_ITEMS = ["jointsInvisible", "shrink", "fill", "extrudeFrames", "extrudeAreas", "supports", "diaphragms", "grid", "mesh"] as const;
export type GeneralItem = (typeof GENERAL_ITEMS)[number];

/** A new view: every object type drawn, no texts or assignment symbols, areas filled, supports on, the grid off. */
export function defaultViewOptions(): ViewOptions {
  return {
    colorBy: "sections",
    hiddenTypes: [],
    jointsInvisible: false,
    shrink: false,
    fill: true,
    extrudeFrames: false,
    extrudeAreas: false,
    supports: true,
    diaphragms: false,
    grid: false,
    mesh: false,
    parallel: false,
    joint: off(OBJECT_ITEMS.joint),
    frame: off(OBJECT_ITEMS.frame),
    area: off(OBJECT_ITEMS.area),
    link: off(OBJECT_ITEMS.link),
  };
}

/** View options read from a file's `gui.viewOptions`; anything missing or of the wrong kind is the default. */
export function viewOptionsOf(raw: unknown): ViewOptions {
  const o = defaultViewOptions();
  if (!raw || typeof raw !== "object") return o;
  const r = raw as Record<string, unknown>;
  if (COLOR_BY.includes(r.colorBy as ColorBy)) o.colorBy = r.colorBy as ColorBy;
  if (Array.isArray(r.hiddenTypes)) o.hiddenTypes = r.hiddenTypes.filter((x): x is string => typeof x === "string");
  for (const k of GENERAL_ITEMS) if (typeof r[k] === "boolean") o[k] = r[k];
  if (typeof r.parallel === "boolean") o.parallel = r.parallel;
  for (const kind of OBJECT_KINDS) {
    const src = r[kind];
    if (!src || typeof src !== "object") continue;
    const items = o[kind] as Record<string, boolean>;
    for (const k of OBJECT_ITEMS[kind]) {
      const v = (src as Record<string, unknown>)[k];
      if (typeof v === "boolean") items[k] = v;
    }
  }
  return o;
}

/** A copy of `o` with one object kind's items changed. */
export function withItems<K extends ObjectKind>(o: ViewOptions, kind: K, patch: Partial<ViewOptions[K]>): ViewOptions {
  return { ...o, [kind]: { ...o[kind], ...patch } };
}

/** Kinds that have `item` (labels, ids, localAxes…). */
const kindsWith = (item: string) => OBJECT_KINDS.filter((k) => (OBJECT_ITEMS[k] as readonly string[]).includes(item));

/** Whether `item` is on for every kind that has it (View > Show Labels / Show Local Axes are checked). */
export function allOn(o: ViewOptions, item: "labels" | "localAxes"): boolean {
  return kindsWith(item).every((k) => (o[k] as Record<string, boolean>)[item]);
}

/** `item` set on or off for every kind that has it. */
export function setAll(o: ViewOptions, item: "labels" | "localAxes", value: boolean): ViewOptions {
  let next = o;
  for (const k of kindsWith(item)) next = withItems(next, k, { [item]: value });
  return next;
}
