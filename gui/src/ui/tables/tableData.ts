// Display > Tables: what each table holds. Model tables come from the model; result tables from the
// chosen cases (a Case column first when a table has a set of rows per case).

import { t, type Key } from "../../i18n";
import { DECK_TYPES, PROPERTY_TYPE_KEYS, sectionWeight } from "../../model/deck";
import { MATERIAL_TYPES, materialType } from "../../model/materials";
import { objectTypes, typeName } from "../../model/objectTypes";
import { endOffsets, offsetText } from "../../model/offsets";
import { springsOf } from "../../model/ops";
import { sectionAreaModifiers } from "../../model/modifiers";
import { AREA_MODIFIERS, FRAME_MODIFIERS, type Asce716, type CaseResult, type Group, type Id, type Material, type Model, type Results } from "../../model/types";
import { areaMeshText } from "../../model/storedMesh";
import { insertionText } from "../../model/insertion";
import { resultLabel } from "../../scene/buildResultsScene";
import { SHAPES, sectionShape } from "../../model/sectionShapes";
import type { UnitFormat } from "../../units/display";
import type { Quantity } from "../../units/units";

export type TableId =
  | "displacements"
  | "reactions"
  | "forces"
  | "linkForces"
  | "areaForces"
  | "joints"
  | "groups"
  | "groupAssignments"
  | "materials"
  | "frameSections"
  | "frames"
  | "springs"
  | "links"
  | "areas"
  | "areaSections"
  | "deckSections"
  | "springProps"
  | "areaLoads"
  | "loadPatterns"
  | "loadCases"
  | "windLoads"
  | "frameModifiers"
  | "areaModifiers"
  | "meshJoints"
  | "meshElements"
  | "modalPeriods"
  | "modalRatios"
  | "modalLoadParticipation"
  | "centres"
  | "spectrumFunctions"
  | "spectrumCases"
  | "baseReactions"
  | "storyDrifts"
  | "jointDrifts"
  | "storyForces"
  | "pierLabels"
  | "spandrelLabels"
  | "wallLabelAssignments"
  | "pierForces"
  | "spandrelForces"
  | "pierSections"
  | "spandrelSections";

/** A group of the tables tree: its label and its tables or sub-groups. */
export interface TableGroup {
  label: Key;
  items: (TableId | TableGroup)[];
}

/** The tables tree of Display > Tables. */
export const TABLE_TREE: TableGroup[] = [
  {
    label: "tbl.group.model",
    items: [
      { label: "tbl.group.properties", items: ["materials", "frameSections", "areaSections", "deckSections", "springProps", "frameModifiers", "areaModifiers", "spectrumFunctions", "pierLabels", "spandrelLabels"] },
      { label: "tbl.group.connectivity", items: ["joints", "frames", "areas", "links", "springs", "groups", "groupAssignments", "wallLabelAssignments"] },
      { label: "tbl.group.loads", items: ["loadPatterns", "loadCases", "spectrumCases", "areaLoads", "windLoads"] },
    ],
  },
  {
    label: "tbl.group.results",
    items: [
      { label: "tbl.group.joints", items: ["displacements", "reactions", "jointDrifts"] },
      { label: "tbl.group.elements", items: ["forces", "linkForces", "areaForces"] },
      { label: "tbl.group.structure", items: ["baseReactions", "storyDrifts", "storyForces", "centres"] },
      { label: "tbl.group.walls", items: ["pierForces", "spandrelForces", "pierSections", "spandrelSections"] },
      { label: "tbl.group.modal", items: ["modalPeriods", "modalRatios", "modalLoadParticipation"] },
      { label: "tbl.group.mesh", items: ["meshJoints", "meshElements"] },
    ],
  },
];

/** Each table's name. */
export const TABLE_LABEL: Record<TableId, Key> = {
  joints: "tbl.joints",
  frameSections: "tbl.frameSections",
  groups: "tbl.groups",
  groupAssignments: "tbl.groupAssignments",
  frames: "tbl.frames",
  springs: "tbl.springs",
  links: "tbl.links",
  areas: "tbl.areas",
  materials: "tbl.materials",
  areaSections: "tbl.areaSections",
  deckSections: "tbl.deckSections",
  springProps: "tbl.springProps",
  areaLoads: "tbl.areaLoads",
  loadPatterns: "tbl.loadPatterns",
  loadCases: "tbl.loadCases",
  windLoads: "tbl.windLoads",
  frameModifiers: "tbl.frameModifiers",
  areaModifiers: "tbl.areaModifiers",
  spectrumFunctions: "tbl.spectrumFunctions",
  spectrumCases: "tbl.spectrumCases",
  displacements: "tbl.displacements",
  reactions: "tbl.reactions",
  forces: "tbl.forces",
  linkForces: "tbl.linkForces",
  areaForces: "tbl.areaForces",
  baseReactions: "tbl.baseReactions",
  storyDrifts: "tbl.storyDrifts",
  jointDrifts: "tbl.jointDrifts",
  storyForces: "tbl.storyForces",
  centres: "tbl.centres",
  modalPeriods: "tbl.modalPeriods",
  modalRatios: "tbl.modalRatios",
  modalLoadParticipation: "tbl.modalLoadParticipation",
  meshJoints: "tbl.meshJoints",
  meshElements: "tbl.meshElements",
  pierLabels: "tbl.pierLabels",
  spandrelLabels: "tbl.spandrelLabels",
  wallLabelAssignments: "tbl.wallLabelAssignments",
  pierForces: "tbl.pierForces",
  spandrelForces: "tbl.spandrelForces",
  pierSections: "tbl.pierSections",
  spandrelSections: "tbl.spandrelSections",
};

/** Result tables with a set of rows per chosen case. */
export const CASE_TABLES: TableId[] = ["displacements", "reactions", "forces", "linkForces", "areaForces", "baseReactions", "storyDrifts", "jointDrifts", "storyForces", "pierForces", "spandrelForces"];

/** Result tables that need the cases' values (fetched on OK); the others read the cases' summaries. */
export const VALUE_TABLES: TableId[] = ["displacements", "reactions", "forces", "linkForces", "areaForces", "jointDrifts"];

/** Tables filtered by the chosen load patterns. */
export const PATTERN_TABLES: TableId[] = ["loadPatterns", "areaLoads", "windLoads"];

/** Whether a table has anything to show for this model and these results (the tree lists only these). */
export function hasTable(id: TableId, m: Model, r: Results | null): boolean {
  const hasAreas = m.areas.length > 0 || m.area_sections.length > 0;
  switch (id) {
    case "spectrumFunctions":
      return !!m.spectrum_functions?.length;
    case "spectrumCases":
      return !!m.spectrum_cases?.length;
    case "windLoads":
      return m.load_patterns.some((p) => p.wind_loads?.length);
    case "areas":
    case "areaSections":
    case "areaModifiers":
    case "areaLoads":
      return hasAreas;
    case "deckSections":
      return m.area_sections.some((s) => s.deck);
    case "springs":
      return m.springs.length > 0;
    case "groups":
    case "groupAssignments":
      return !!m.groups?.length;
    case "springProps":
      return !!(m.line_spring_props?.length || m.area_spring_props?.length);
    case "links":
      return m.links.length > 0;
    case "displacements":
    case "reactions":
    case "forces":
    case "baseReactions":
      return !!r;
    case "linkForces":
      return !!r && m.links.length > 0;
    case "areaForces":
      return !!r && m.areas.length > 0;
    case "storyDrifts":
    case "jointDrifts":
    case "storyForces":
      return !!r?.cases.some((c) => c.story_drifts?.length);
    case "centres":
      return !!r?.diaphragm_centers?.length;
    case "pierLabels":
      return !!m.pier_labels?.length;
    case "spandrelLabels":
      return !!m.spandrel_labels?.length;
    case "wallLabelAssignments":
      return [...m.frames, ...m.areas].some((o) => o.pier || o.spandrel);
    case "pierForces":
      return !!r?.cases.some((c) => c.pier_forces?.length);
    case "spandrelForces":
      return !!r?.cases.some((c) => c.spandrel_forces?.length);
    case "pierSections":
      return !!r?.pier_sections?.length;
    case "spandrelSections":
      return !!r?.spandrel_sections?.length;
    case "modalPeriods":
    case "modalRatios":
      return !!r?.modal;
    case "modalLoadParticipation":
      return !!r?.modal?.load_participation?.length;
    case "meshJoints":
    case "meshElements":
      return !!r?.mesh;
    default:
      return true;
  }
}

/** A table's columns (with their quantities: numbers in a column with one are shown in display units) and rows. */
export interface TableData {
  head: string[];
  qs: (Quantity | null)[];
  rows: (string | number)[][];
}

export interface TableContext {
  model: Model;
  results: Results | null;
  /** The chosen cases with their values (for the tables of `VALUE_TABLES`). */
  cases: CaseResult[];
  /** The chosen cases, values or not (their summaries: base reactions, story results). */
  heads: CaseResult[];
  /** The chosen load patterns (null: all). */
  patterns: Set<Id> | null;
  U: UnitFormat;
}

/** An ASCE 7-16 function's inputs in one cell. */
const asceText = (a: Asce716) => `Ss ${a.ss}, S1 ${a.s1}, TL ${a.tl}, ${a.site_class}${a.fa !== undefined ? `, Fa ${a.fa}` : ""}${a.fv !== undefined ? `, Fv ${a.fv}` : ""}`;

export function buildTable(id: TableId, ctx: TableContext): TableData {
  const { model, results, U } = ctx;
  let qs: (Quantity | null)[] = [];
  const col = (name: string, q: Quantity) => `${name} (${U.label(q)})`;
  const types = objectTypes(model);
  const mesh = results?.mesh;
  const patterns = model.load_patterns.filter((p) => !ctx.patterns || ctx.patterns.has(p.id));
  /** Rows of every chosen case, each with the case first. */
  const perCase = (of: (c: CaseResult) => (string | number)[][], list = ctx.cases) => list.flatMap((c) => of(c).map((r) => [resultLabel(c), ...r]));
  let head: string[] = [];
  let rows: (string | number)[][] = [];
  switch (id) {
    case "joints":
      head = [t("col.joint"), col("X", "distance"), col("Y", "distance"), col("Z", "distance"), t("col.diaphragm")];
      qs = [null, "distance", "distance", "distance"];
      rows = model.nodes.map((n) => [n.id, n.x, n.y, n.z, n.diaphragm ?? ""]);
      break;
    case "groups":
      head = [t("col.group"), t("col.color"), t("grp.count.nodes", { n: "" }).trim(), t("grp.count.frames", { n: "" }).trim(), t("grp.count.links", { n: "" }).trim(), t("grp.count.areas", { n: "" }).trim()];
      rows = (model.groups ?? []).map((g) => [g.id, g.color ?? "", g.nodes?.length ?? 0, g.frames?.length ?? 0, g.links?.length ?? 0, g.areas?.length ?? 0]);
      break;
    case "groupAssignments": {
      head = [t("col.group"), t("col.objectKind"), t("col.objects")];
      const kinds: [keyof Group, string][] = [
        ["nodes", t("col.joint")],
        ["frames", t("col.frame")],
        ["links", t("col.link")],
        ["areas", t("col.area")],
      ];
      rows = (model.groups ?? []).flatMap((g) => kinds.flatMap(([k, label]) => ((g[k] as string[] | undefined) ?? []).map((o) => [g.id, label, o])));
      break;
    }
    case "frameSections":
      head = [
        t("col.section"),
        t("col.shape"),
        col("A", "sectionArea"),
        col("I33", "length4"),
        col("I22", "length4"),
        col("I23", "length4"),
        col("J", "length4"),
        col("As2", "sectionArea"),
        col("As3", "sectionArea"),
        col(t("col.depth"), "sectionLength"),
        col(t("col.width"), "sectionLength"),
      ];
      qs = [null, null, "sectionArea", "length4", "length4", "length4", "length4", "sectionArea", "sectionArea", "sectionLength", "sectionLength"];
      rows = model.sections.map((s) => {
        const shape = sectionShape(model, s);
        return [s.id, !shape ? t("df.shapeGeneral") : shape.type === "aisc" ? `AISC ${shape.name ?? ""}` : t(SHAPES[shape.type].label), s.A, s.I33, s.I22, s.I23 ?? 0, s.J, s.As2 ?? 0, s.As3 ?? 0, s.depth ?? "", s.width ?? ""];
      });
      break;
    case "frames":
      head = [t("col.frame"), t("col.objectType"), t("col.type"), t("col.jointI"), t("col.jointJ"), t("col.section"), t("col.material"), t("col.angle"), t("col.releases"), t("col.offsets", { unit: U.label("distance") }), t("col.insertion"), t("col.lineSpring")];
      const offsets = endOffsets(model);
      rows = model.frames.map((e) => [
        e.id,
        t(typeName(types.frames.get(e.id) ?? "brace", "frame")),
        e.type,
        e.nodes[0],
        e.nodes[1],
        e.section || t("obj.noSection"),
        e.material,
        e.angle ?? 0,
        (["i", "j"] as const).flatMap((end) => (e.releases?.[end]?.length ? [`${end}: ${e.releases[end]!.join(" ")}`] : [])).join(" · "),
        offsetText(e, offsets.get(e.id), (v) => U.value(v, "distance")),
        insertionText(e.insertion, false),
        e.line_spring ?? "",
      ]);
      break;
    case "springs":
      head = [t("col.joint"), ...["UX", "UY", "UZ"].map((d) => col(d, "transStiffness")), ...["RX", "RY", "RZ"].map((d) => col(d, "rotStiffness"))];
      qs = [null, "transStiffness", "transStiffness", "transStiffness", "rotStiffness", "rotStiffness", "rotStiffness"];
      rows = model.springs.map((s) => [s.node, ...springsOf(model, s.node)]);
      break;
    case "links":
      head = [t("col.link"), t("col.type"), t("col.jointI"), t("col.jointJ")];
      rows = model.links.map((l) => [l.id, l.type, l.nodes[0], l.nodes[1]]);
      break;
    case "areas":
      head = [t("col.area"), t("col.objectType"), t("col.corners"), t("col.section"), t("col.angle"), t("col.mesh"), t("col.diaphragm"), t("col.insertion"), t("col.areaSpring")];
      rows = model.areas.map((a) => [
        a.id,
        t(typeName(types.areas.get(a.id) ?? "wall", "area")),
        a.nodes.join(" "),
        a.section || t("obj.noSection"),
        a.angle ?? 0,
        areaMeshText(a),
        a.diaphragm ?? "",
        a.opening ? "" : insertionText(a.insertion, true),
        a.area_spring ?? "",
      ]);
      break;
    case "springProps": {
      head = [t("col.property"), t("col.type"), ...["U1", "U2", "U3"].map((d) => `${d} (${U.label("lineSpring")} | ${U.label("areaSpring")})`), col("R1", "lineRotSpring")];
      // Line and area values have different units: each row is shown in its own kind's unit.
      rows = [
        ...(model.line_spring_props ?? []).map((p) => [p.id, t("tbl.lineSpring"), ...[p.u1, p.u2, p.u3].map((v) => U.value(v ?? 0, "lineSpring")), U.value(p.r1 ?? 0, "lineRotSpring")]),
        ...(model.area_spring_props ?? []).map((p) => [p.id, t("tbl.areaSpring"), ...[p.u1, p.u2, p.u3].map((v) => U.value(v ?? 0, "areaSpring")), ""]),
      ];
      break;
    }
    case "areaSections":
      head = [t("col.section"), t("col.propertyType"), t("col.type"), t("col.material"), t("col.thickness", { unit: U.label("sectionLength") })];
      qs = [null, null, null, null, "sectionLength"];
      rows = model.area_sections.map((s) => [s.id, t(PROPERTY_TYPE_KEYS[s.property_type ?? "slab"]), s.type, s.material, s.thickness]);
      break;
    case "materials": {
      const st = (k: Key) => t(k, { unit: U.label("stress") });
      head = [t("col.material"), t("col.type"), t("col.E", { unit: U.label("modulus") }), t("col.nu"), t("col.G", { unit: U.label("modulus") }), t("col.unitWeight", { unit: U.label("weightPerVolume") }), st("col.fy"), st("col.fu"), st("col.fc")];
      qs = [null, null, "modulus", null, "modulus", "weightPerVolume", "stress", "stress", "stress"];
      const typeKey = (x: Material) => MATERIAL_TYPES.find(([k]) => k === materialType(x))?.[1] ?? "df.matOther";
      rows = model.materials.map((x) => [x.id, t(typeKey(x)), x.E, x.nu ?? "", x.G ?? "", x.weight ?? 0, x.fy ?? "", x.fu ?? "", x.fc ?? ""]);
      break;
    }
    case "deckSections": {
      const len = (k: Key) => t(k, { unit: U.label("sectionLength") });
      const fpa = (k: Key) => t(k, { unit: U.label("forcePerArea") });
      head = [t("col.section"), t("col.deckType"), t("col.material"), t("col.deckMaterial"), len("col.slabDepth"), len("col.ribDepth"), len("col.ribWidthTop"), len("col.ribWidthBottom"), len("col.ribSpacing"), len("col.shearThickness"), fpa("col.deckUnitWeight"), fpa("col.weightPerArea")];
      qs = [null, null, null, null, "sectionLength", "sectionLength", "sectionLength", "sectionLength", "sectionLength", "sectionLength", "forcePerArea", "forcePerArea"];
      rows = model.area_sections.flatMap((s) => {
        const d = s.deck;
        if (!d) return [];
        const typeKey = DECK_TYPES.find(([k]) => k === d.type)?.[1] ?? "df.deckFilled";
        return [[s.id, t(typeKey), s.material, d.steel, d.slab_depth, d.rib_depth, d.rib_width_top, d.rib_width_bottom, d.rib_spacing, d.shear_thickness, d.unit_weight, sectionWeight(model, s)]];
      });
      break;
    }
    case "frameModifiers": {
      head = [t("col.object"), ...FRAME_MODIFIERS];
      const row = (label: string, m?: Partial<Record<string, number>>) => (m && Object.keys(m).length ? [[label, ...FRAME_MODIFIERS.map((k) => m[k] ?? 1)]] : []);
      rows = [
        ...model.sections.flatMap((s) => row(t("mod.ofSection", { id: s.id }), s.modifiers)),
        ...model.frames.flatMap((e) => row(t("mod.ofFrame", { id: e.id }), e.modifiers)),
      ];
      break;
    }
    case "areaModifiers": {
      head = [t("col.object"), ...AREA_MODIFIERS];
      const row = (label: string, m?: Partial<Record<string, number>>) => (m && Object.keys(m).length ? [[label, ...AREA_MODIFIERS.map((k) => m[k] ?? 1)]] : []);
      rows = [
        ...model.area_sections.flatMap((s) => row(t("mod.ofAreaSection", { id: s.id }), sectionAreaModifiers(s))),
        ...model.areas.flatMap((a) => row(t("mod.ofArea", { id: a.id }), a.modifiers)),
      ];
      break;
    }
    case "areaLoads":
      head = [t("col.pattern"), t("col.area"), t("col.dir"), col("q", "forcePerArea")];
      qs = [null, null, null, "forcePerArea"];
      rows = patterns.flatMap((lp) => lp.area_loads.map((l) => [lp.id, l.area, l.dir, l.q]));
      break;
    case "loadPatterns":
      head = [t("col.pattern"), t("col.type"), t("df.selfWeight"), t("col.loads")];
      rows = patterns.map((p) => [
        p.id,
        t(`lp.type.${p.type}` as Key),
        p.self_weight ?? 0,
        p.node_loads.length + p.frame_loads.length + p.area_loads.length + (p.wind_loads?.length ?? 0),
      ]);
      break;
    case "loadCases":
      head = [t("col.case"), t("col.pattern"), t("df.comboScale")];
      rows = model.load_cases.flatMap((c) => (c.loads.length ? c.loads.map((l) => [c.id, l.pattern, l.scale]) : [[c.id, "", ""]]));
      break;
    case "windLoads":
      head = [t("col.pattern"), t("lp.story"), t("lp.diaphragm"), col("Fx", "force"), col("Fy", "force"), col("Mz", "moment"), col("X", "distance"), col("Y", "distance")];
      qs = [null, null, null, "force", "force", "moment", "distance", "distance"];
      rows = patterns.flatMap((p) => (p.wind_loads ?? []).map((w) => [p.id, w.story, w.diaphragm, w.fx ?? 0, w.fy ?? 0, w.mz ?? 0, w.x, w.y]));
      break;
    case "areaForces":
      head = [
        t("col.area"),
        t("col.element"),
        t("col.joint"),
        ...["F11", "F22", "F12"].map((k) => col(k, "forcePerLength")),
        ...["M11", "M22", "M12"].map((k) => col(k, "momentPerLength")),
        ...["V13", "V23"].map((k) => col(k, "forcePerLength")),
      ];
      qs = [null, null, null, "forcePerLength", "forcePerLength", "forcePerLength", "momentPerLength", "momentPerLength", "momentPerLength", "forcePerLength", "forcePerLength"];
      rows = perCase((c) => (c.shells ?? []).flatMap((s) => s.corners.map((k) => [s.area, s.element, k.node, k.F11, k.F22, k.F12, k.M11, k.M22, k.M12, k.V13, k.V23])));
      break;
    case "meshJoints":
      head = [t("col.joint"), col("X", "distance"), col("Y", "distance"), col("Z", "distance")];
      qs = [null, "distance", "distance", "distance"];
      rows = (mesh?.joints ?? []).map((j) => [j.id, j.x, j.y, j.z]);
      break;
    case "meshElements":
      head = [t("col.element"), t("col.parent"), t("col.corners")];
      rows = [...(mesh?.shells ?? []), ...(mesh?.frames ?? [])].map((el) => [el.id, el.parent, el.nodes.join(" ")]);
      break;
    case "displacements":
      head = [t("col.joint"), ...["UX", "UY", "UZ"].map((d) => col(d, "displacement")), ...["RX", "RY", "RZ"].map((d) => col(d, "rotation"))];
      qs = [null, "displacement", "displacement", "displacement", "rotation", "rotation", "rotation"];
      rows = perCase((c) => c.displacements.map((d) => [d.node, d.ux, d.uy, d.uz, d.rx, d.ry, d.rz]));
      break;
    case "reactions":
      head = [t("col.joint"), ...["FX", "FY", "FZ"].map((d) => col(d, "force")), ...["MX", "MY", "MZ"].map((d) => col(d, "moment"))];
      qs = [null, "force", "force", "force", "moment", "moment", "moment"];
      rows = perCase((c) => c.reactions.map((r) => [r.node, r.fx, r.fy, r.fz, r.mx, r.my, r.mz]));
      break;
    case "forces":
      head = [t("col.frame"), t("col.station", { unit: U.label("distance") }), ...["P", "V2", "V3"].map((d) => col(d, "force")), ...["T", "M2", "M3"].map((d) => col(d, "moment"))];
      qs = [null, "distance", "force", "force", "force", "moment", "moment", "moment"];
      rows = perCase((c) => c.frames.flatMap((e) => e.stations.map((s) => [e.element, s.x, s.P, s.V2, s.V3, s.T, s.M2, s.M3])));
      break;
    case "centres":
      head = [
        t("col.story"),
        t("lp.diaphragm"),
        col("MassX", "mass"),
        col("MassY", "mass"),
        col("XCM", "distance"),
        col("YCM", "distance"),
        col("CumMassX", "mass"),
        col("CumMassY", "mass"),
        col("XCCM", "distance"),
        col("YCCM", "distance"),
        col("XCR", "distance"),
        col("YCR", "distance"),
      ];
      qs = [null, null, "mass", "mass", "distance", "distance", "mass", "mass", "distance", "distance", "distance", "distance"];
      rows = (results?.diaphragm_centers ?? []).map((c) => [c.story, c.diaphragm, c.mass_x, c.mass_y, c.xcm ?? "", c.ycm ?? "", c.cum_mass_x, c.cum_mass_y, c.xccm ?? "", c.yccm ?? "", c.xcr ?? "", c.ycr ?? ""]);
      break;
    case "modalPeriods":
      head = [t("col.mode"), t("col.period"), t("col.frequency"), t("col.circular"), t("col.eigenvalue")];
      // A static mode of a Ritz case has no period.
      rows = (results?.modal?.modes ?? []).map((md) =>
        md.static_mode ? [md.mode, t("col.staticMode"), "", "", ""] : [md.mode, md.period, md.frequency, md.circular_frequency, md.eigenvalue],
      );
      break;
    case "modalRatios":
      head = [t("col.mode"), t("col.period"), "UX", "UY", "UZ", t("col.sum", { d: "UX" }), t("col.sum", { d: "UY" }), t("col.sum", { d: "UZ" }), "RX", "RY", "RZ", t("col.sum", { d: "RX" }), t("col.sum", { d: "RY" }), t("col.sum", { d: "RZ" })];
      rows = (results?.modal?.modes ?? []).map((md) => [md.mode, md.static_mode ? t("col.staticMode") : md.period, ...md.mass_ratio.slice(0, 3), ...md.sum_mass_ratio.slice(0, 3), ...md.mass_ratio.slice(3), ...md.sum_mass_ratio.slice(3)]);
      break;
    case "modalLoadParticipation":
      head = [t("col.type"), t("col.item"), t("col.staticPercent"), t("col.dynamicPercent")];
      rows = (results?.modal?.load_participation ?? []).map((l) => [l.kind === "acceleration" ? t("col.acceleration") : t("col.loadPattern"), l.name, l.static_percent, l.dynamic_percent]);
      break;
    case "spectrumFunctions":
      head = [t("col.function"), t("col.type"), t("rs.damping"), t("col.period"), t("col.value"), t("col.asce")];
      rows = (model.spectrum_functions ?? []).flatMap((f) =>
        f.asce7_16
          ? [[f.id, t("rs.kind.asce"), f.damping ?? 0.05, "", "", asceText(f.asce7_16)]]
          : (f.points ?? []).map(([p, v], k) => [k ? "" : f.id, k ? "" : t("rs.kind.table"), k ? "" : (f.damping ?? 0.05), p, v, ""]),
      );
      break;
    case "spectrumCases":
      head = [t("col.case"), t("rs.dir"), t("rs.functionCol"), col(t("rs.scaleShort"), "acceleration"), t("rs.angle"), t("rs.damping"), t("rs.modalRule"), t("rs.dirRule"), t("rs.eccentricity")];
      qs = [null, null, null, "acceleration"];
      rows = (model.spectrum_cases ?? []).flatMap((c) =>
        c.loads.map((l, k) => [
          k ? "" : c.id,
          l.dir,
          l.function,
          l.scale,
          l.angle ?? 0,
          k ? "" : (c.damping ?? 0.05),
          k ? "" : t(`rs.rule.${c.modal_combination ?? "cqc"}` as Key),
          k ? "" : t(`rs.rule.${c.directional_combination ?? "srss"}` as Key) + (c.directional_combination === "absolute" ? ` (${c.abs_factor ?? 1})` : ""),
          k ? "" : (c.eccentricity ?? 0),
        ]),
      );
      break;
    case "baseReactions":
      head = [t("col.case"), ...["FX", "FY", "FZ"].map((d) => col(d, "force")), ...["MX", "MY", "MZ"].map((d) => col(d, "moment"))];
      qs = [null, "force", "force", "force", "moment", "moment", "moment"];
      rows = ctx.heads.filter((rc) => rc.base_reaction).map((rc) => [resultLabel(rc), ...rc.base_reaction!]);
      break;
    case "storyDrifts":
      head = [t("col.story"), t("col.dir"), t("col.drift"), t("col.joint")];
      rows = perCase((c) => (c.story_drifts ?? []).map((d) => [d.story, d.dir, d.drift, d.node]), ctx.heads);
      break;
    case "jointDrifts":
      head = [t("col.story"), t("col.joint"), t("col.jointBelow"), t("col.driftX"), t("col.driftY")];
      rows = perCase((c) => (c.joint_drifts ?? []).map((d) => [d.story, d.node, d.below, d.drift[0], d.drift[1]]));
      break;
    case "storyForces":
      head = [t("col.story"), t("col.location"), col("P", "force"), col("VX", "force"), col("VY", "force"), col("T", "moment"), col("MX", "moment"), col("MY", "moment")];
      qs = [null, null, "force", "force", "force", "moment", "moment", "moment"];
      rows = perCase(
        (c) =>
          (c.story_forces ?? []).flatMap((f) => [
            [f.story, t("col.top"), ...f.top],
            [f.story, t("col.bottom"), ...f.bottom],
          ]),
        ctx.heads,
      );
      break;
    case "pierLabels":
      head = [t("col.label"), t("col.color")];
      rows = (model.pier_labels ?? []).map((l) => [l.id, l.color ?? ""]);
      break;
    case "spandrelLabels":
      head = [t("col.label"), t("col.multiStory"), t("col.color")];
      rows = (model.spandrel_labels ?? []).map((l) => [l.id, l.multi_story ? t("col.yes") : t("col.no"), l.color ?? ""]);
      break;
    case "wallLabelAssignments":
      head = [t("col.objectKind"), t("col.object"), t("col.pier"), t("col.spandrel")];
      rows = [
        ...model.frames.filter((f) => f.pier || f.spandrel).map((f) => [t("col.frame"), f.id, f.pier ?? "", f.spandrel ?? ""]),
        ...model.areas.filter((a) => a.pier || a.spandrel).map((a) => [t("col.area"), a.id, a.pier ?? "", a.spandrel ?? ""]),
      ];
      break;
    case "pierForces":
    case "spandrelForces": {
      const pier = id === "pierForces";
      head = [t("col.story"), t(pier ? "col.pier" : "col.spandrel"), t("col.location"), col("P", "force"), col("V2", "force"), col("V3", "force"), col("T", "moment"), col("M2", "moment"), col("M3", "moment")];
      qs = [null, null, null, "force", "force", "force", "moment", "moment", "moment"];
      rows = perCase(
        (c) =>
          pier
            ? (c.pier_forces ?? []).flatMap((f) => [
                [f.story, f.pier, t("col.top"), ...f.top],
                [f.story, f.pier, t("col.bottom"), ...f.bottom],
              ])
            : (c.spandrel_forces ?? []).flatMap((f) => [
                [f.story, f.spandrel, t("col.left"), ...f.left],
                [f.story, f.spandrel, t("col.right"), ...f.right],
              ]),
        ctx.heads,
      );
      break;
    }
    case "pierSections":
      head = [t("col.story"), t("col.pier"), t("col.axisAngle"), ...["X", "Y", "Z"].map((d) => col(t("col.cgBottom", { d }), "distance")), ...["X", "Y", "Z"].map((d) => col(t("col.cgTop", { d }), "distance"))];
      qs = [null, null, null, "distance", "distance", "distance", "distance", "distance", "distance"];
      rows = (results?.pier_sections ?? []).map((p) => [p.story, p.pier, p.angle, ...p.bottom, ...p.top]);
      break;
    case "spandrelSections":
      head = [t("col.story"), t("col.spandrel"), ...["X", "Y", "Z"].map((d) => col(t("col.cgLeft", { d }), "distance")), ...["X", "Y", "Z"].map((d) => col(t("col.cgRight", { d }), "distance"))];
      qs = [null, null, "distance", "distance", "distance", "distance", "distance", "distance"];
      rows = (results?.spandrel_sections ?? []).map((p) => [p.story, p.spandrel, ...p.left, ...p.right]);
      break;
    case "linkForces":
      head = [t("col.link"), t("col.station", { unit: U.label("distance") }), ...["P", "V2", "V3"].map((d) => col(d, "force")), ...["T", "M2", "M3"].map((d) => col(d, "moment"))];
      qs = [null, "distance", "force", "force", "force", "moment", "moment", "moment"];
      rows = perCase((c) => (c.links ?? []).flatMap((e) => e.stations.map((s) => [e.element, s.x, s.P, s.V2, s.V3, s.T, s.M2, s.M3])));
      break;
  }
  if (CASE_TABLES.includes(id) && id !== "baseReactions") {
    head = [t("col.case"), ...head];
    qs = [null, ...qs];
  }
  return { head, qs, rows };
}
