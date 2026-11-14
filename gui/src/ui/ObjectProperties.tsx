// Properties of one selected object in three groups: Geometry, Assignments and Loads. Values with one
// choice or number are edited in place (a drop-down or a number field); values with several parts
// (modifiers, releases, offsets, springs, loads…) show a summary that expands, and their "…" opens the
// window that edits them (it applies to the selection: this object).

import { useState, type ReactNode } from "react";

import { t, type Key } from "../i18n";
import { areaOf } from "../model/area";
import { areaPoints, assignArea } from "../model/areaOps";
import { objectTypes, typeName } from "../model/objectTypes";
import { PROPERTY_TYPE_KEYS } from "../model/deck";
import { endOffsets } from "../model/offsets";
import { assignFrame, linkLength, memberLength, moveNode, restraintsOf, setDiaphragm, setInMesh, setRestraints, setSpecial, springsOf } from "../model/ops";
import { assignAreaSpring, assignLineSpring } from "../model/springs";
import { objectStories } from "../model/stories";
import { AREA_MODIFIERS, DOFS, FRAME_MODIFIERS, type Id, type Model } from "../model/types";
import { assignWallLabel, labelsOf, type WallKind } from "../model/wallLabels";
import { fmt } from "../scene/format";
import { useStore } from "../state/store";
import { modelUnits } from "../units/format";
import type { Quantity } from "../units/units";
import { commands } from "./commands";

/** A value edited in place: a drop-down or a number (`apply` gets the model value). */
export type Edit =
  | {
      kind: "select";
      value: string;
      options: { value: string; label: string }[];
      apply: (v: string) => void;
    }
  | {
      kind: "number";
      text: string;
      unit?: string;
      read: (text: string) => number | null;
      apply: (v: number) => void;
    };

/** One row: a label and its value; with `rows`, the value is a summary and the row expands to them. */
export interface Prop {
  label: string;
  value: string;
  /** The full name, shown on hover, when the label is short. */
  title?: string;
  rows?: Prop[];
  edit?: Edit;
  /** Opens the window that edits this value. */
  more?: () => void;
}

export interface PropGroup {
  key: "geometry" | "assignments" | "loads";
  label: string;
  rows: Prop[];
  more?: () => void;
}

type Kind = "node" | "frame" | "link" | "area";

const edit = (fn: (m: Model) => Model) => useStore.getState().edit(fn);
const run = (name: keyof typeof commands) => () => commands[name].run();

function restraintText(r: boolean[]) {
  if (r.every((x) => x)) return t("side.fixed");
  if (r.every((x) => !x)) return t("side.free");
  if (r.slice(0, 3).every((x) => x) && r.slice(3).every((x) => !x)) return t("side.pinned");
  return DOFS.filter((_, i) => r[i])
    .map((d) => d.toUpperCase())
    .join(", ");
}

/** Each load on the object in one load pattern, as text in display units. */
export function loadsOn(model: Model, patternId: string | null, kind: "node" | "frame" | "area", id: string): string[] {
  const c = model.load_patterns.find((x) => x.id === patternId);
  if (!c) return [];
  const U = modelUnits(model);
  if (kind === "area")
    return c.area_loads
      .filter((l) => l.area === id)
      .map((l) =>
        t("side.areaLoad", {
          q: `${U.fmt(l.q, "forcePerArea")} ${U.label("forcePerArea")}`,
          dir: l.dir,
        }),
      );
  if (kind === "node") {
    return c.node_loads
      .filter((l) => l.node === id)
      .map((l) =>
        (["fx", "fy", "fz", "mx", "my", "mz"] as const)
          .filter((f) => l[f])
          .map((f) => `${f.toUpperCase()} ${U.fmt(l[f]!, f.startsWith("f") ? "force" : "moment")}`)
          .join(", "),
      );
  }
  return c.frame_loads
    .filter((l) => l.frame === id)
    .map((l) => {
      const d = (v: number) => `${U.fmt(v, "distance")} ${U.label("distance")}`;
      const w = (v: number) => `${U.fmt(v, "forcePerLength")} ${U.label("forcePerLength")}`;
      if (l.type === "point")
        return t("side.pointLoad", {
          p: `${U.fmt(l.p, "force")} ${U.label("force")}`,
          dir: l.dir,
          at: d(l.at),
        });
      const range =
        l.from !== undefined || l.to !== undefined
          ? t("side.uniformRange", {
              from: d(l.from ?? 0),
              to: l.to !== undefined ? d(l.to) : t("side.end"),
            })
          : "";
      return l.type === "uniform"
        ? t("side.uniformLoad", { w: w(l.w), dir: l.dir }) + range
        : t("side.trapezoidalLoad", { w1: w(l.w1), w2: w(l.w2), dir: l.dir }) + range;
    });
}

/** The object's loads: one row per load pattern that has any ("None" when there are none). */
function loadsGroup(model: Model, kind: "node" | "frame" | "area", id: string): PropGroup {
  const rows = model.load_patterns.flatMap((p) => {
    const loads = loadsOn(model, p.id, kind, id);
    return loads.length ? [{ label: p.id, value: loads.join("; ") }] : [];
  });
  const more = run(kind === "node" ? "jointLoads" : kind === "frame" ? "frameLoads" : "areaLoads");
  return {
    key: "loads",
    label: t("prop.loads"),
    rows: rows.length ? rows : [{ label: t("prop.none"), value: "" }],
    more,
  };
}

/** Factors (1 = unchanged): "Default" when all are 1, else "Specified" with each factor. */
function modifiersProp(names: readonly string[], mods: Record<string, number> | undefined, more: () => void): Prop {
  const specified = names.some((k) => mods?.[k] !== undefined && mods[k] !== 1);
  return {
    label: t("prop.modifiers"),
    title: t("prop.modifiersLong"),
    value: specified ? t("prop.specified") : t("prop.default"),
    rows: names.map((k) => ({
      label: k,
      value: String(Number((mods?.[k] ?? 1).toPrecision(6))),
    })),
    more,
  };
}

/** Six values per DOF (springs, joint masses): "None" when all are 0. */
function dofProp(label: string, values: number[], show: (v: number, i: number) => string, more: () => void): Prop {
  return {
    label,
    value: values.some(Boolean) ? t("prop.specified") : t("prop.none"),
    rows: DOFS.map((d, i) => ({
      label: d.toUpperCase(),
      value: show(values[i] ?? 0, i),
    })),
    more,
  };
}

const yesNo = (v: boolean) => (v ? t("prop.yes") : t("prop.no"));
const yesNoEdit = (v: boolean, apply: (v: boolean) => void): Edit => ({
  kind: "select",
  value: v ? "yes" : "no",
  options: [
    { value: "yes", label: t("prop.yes") },
    { value: "no", label: t("prop.no") },
  ],
  apply: (s) => apply(s === "yes"),
});

/** A drop-down of names with "None" first ("" = none); the current name is listed even when it is unknown. */
function namesEdit(current: string | undefined, names: string[], apply: (v: string | null) => void, noneLabel = t("prop.none")): Edit {
  const list = current && !names.includes(current) ? [current, ...names] : names;
  return {
    kind: "select",
    value: current ?? "",
    options: [{ value: "", label: noneLabel }, ...list.map((n) => ({ value: n, label: n }))],
    apply: (v) => apply(v || null),
  };
}

/** An object's label and story (no story row in a model without stories). */
function placeProps(model: Model, kind: Kind, id: string, label: string | undefined): Prop[] {
  const key = ({ node: "nodes", frame: "frames", link: "links", area: "areas" } as const)[kind];
  const story = objectStories(model)[key].get(id);
  return [{ label: t("prop.label"), value: label ?? "-" }, ...(story ? [{ label: t("prop.story"), value: story }] : [])];
}

/** Pier and spandrel label rows (frames and areas), each a drop-down of the model's labels. */
function wallLabelProps(model: Model, o: { pier?: Id; spandrel?: Id }, objects: { frames?: Id[]; areas?: Id[] }): Prop[] {
  return (["pier", "spandrel"] as WallKind[]).map((k) => ({
    label: t(k === "pier" ? "col.pier" : "col.spandrel"),
    value: o[k] ?? t("prop.none"),
    edit: namesEdit(
      o[k],
      labelsOf(model, k).map((l) => l.id),
      (v) => edit((m) => assignWallLabel(m, k, v, objects)),
    ),
  }));
}

/** The groups for one selected joint, frame, link or area. */
export function objectProps(model: Model, kind: Kind, id: string): PropGroup[] {
  const U = modelUnits(model);
  const q = (v: number, quantity: Quantity) => `${U.fmt(v, quantity)} ${U.label(quantity)}`;
  const dist = (v: number) => q(v, "distance");
  const num = (v: number, quantity: Quantity, apply: (v: number) => void): Edit => ({
    kind: "number",
    text: U.show(v, quantity),
    unit: U.label(quantity),
    read: (s) => U.read(s, quantity),
    apply,
  });
  const degrees = (v: number, apply: (v: number) => void): Edit => ({
    kind: "number",
    text: String(Number(v.toPrecision(10))),
    unit: "°",
    read: (s) => (s.trim() && Number.isFinite(Number(s)) ? Number(s) : null),
    apply,
  });
  const diaphragmEdit = (current: string | undefined, nodes: Id[], areas: Id[]) =>
    namesEdit(
      current,
      (model.diaphragms ?? []).map((d) => d.id),
      (v) => edit((m) => setDiaphragm(m, nodes, areas, v)),
    );

  if (kind === "node") {
    const n = model.nodes.find((x) => x.id === id);
    if (!n) return [];
    const mass = model.masses?.find((x) => x.node === id);
    const r = restraintsOf(model, id);
    const named = [
      {
        value: "fixed",
        label: t("side.fixed"),
        flags: [true, true, true, true, true, true],
      },
      {
        value: "pinned",
        label: t("side.pinned"),
        flags: [true, true, true, false, false, false],
      },
      {
        value: "free",
        label: t("side.free"),
        flags: [false, false, false, false, false, false],
      },
    ];
    const current = named.find((x) => x.flags.every((f, i) => f === r[i]));
    const coord = (axis: 0 | 1 | 2): Prop => ({
      label: "XYZ"[axis],
      value: dist([n.x, n.y, n.z][axis]),
      edit: num([n.x, n.y, n.z][axis], "distance", (v) =>
        edit((m) => {
          const p: [number, number, number] = [n.x, n.y, n.z];
          p[axis] = v;
          return moveNode(m, id, p);
        }),
      ),
    });
    return [
      {
        key: "geometry",
        label: t("prop.geometry"),
        rows: [{ label: t("side.joint"), value: n.id }, ...placeProps(model, kind, id, n.label), coord(0), coord(1), coord(2)],
      },
      {
        key: "assignments",
        label: t("prop.assignments"),
        rows: [
          {
            label: t("side.restraints"),
            value: restraintText(r),
            edit: {
              kind: "select",
              value: current?.value ?? "custom",
              options: [...named.map(({ value, label }) => ({ value, label })), ...(current ? [] : [{ value: "custom", label: restraintText(r) }])],
              apply: (v) => {
                const flags = named.find((x) => x.value === v)?.flags;
                if (flags) edit((m) => setRestraints(m, [id], flags));
              },
            },
            more: run("restraints"),
          },
          dofProp(t("side.springs"), springsOf(model, id), (v, i) => q(v, i < 3 ? "transStiffness" : "rotStiffness"), run("springs")),
          dofProp(
            t("prop.masses"),
            DOFS.map((d) => mass?.[d] ?? 0),
            (v, i) => q(v, i < 3 ? "mass" : "rotationalInertia"),
            run("jointMasses"),
          ),
          {
            label: t("col.diaphragm"),
            value: n.diaphragm ?? t("prop.none"),
            edit: diaphragmEdit(n.diaphragm, [id], []),
          },
          {
            label: t("prop.inMesh"),
            title: t("prop.inMeshLong"),
            value: yesNo(!!n.in_mesh),
            edit: yesNoEdit(!!n.in_mesh, (v) => edit((m) => setInMesh(m, [id], [], v))),
          },
          {
            label: t("prop.special"),
            value: yesNo(!!n.special),
            edit: yesNoEdit(!!n.special, (v) => edit((m) => setSpecial(m, [id], v))),
          },
        ],
      },
      loadsGroup(model, "node", id),
    ];
  }
  if (kind === "link") {
    const l = model.links.find((x) => x.id === id);
    if (!l) return [];
    return [
      {
        key: "geometry",
        label: t("prop.geometry"),
        rows: [
          { label: t("side.rigidLink"), value: l.id },
          ...placeProps(model, kind, id, l.label),
          {
            label: t("prop.ends"),
            title: t("side.ends"),
            value: `${l.nodes[0]} → ${l.nodes[1]}`,
          },
          { label: t("side.length"), value: dist(linkLength(model, id)) },
        ],
      },
    ];
  }
  if (kind === "frame") {
    const e = model.frames.find((x) => x.id === id);
    if (!e) return [];
    const [oi, oj] = e.end_offsets ? (endOffsets(model).get(id) ?? [0, 0]) : [0, 0];
    const releases = (end: "i" | "j") => (e.releases?.[end]?.length ? e.releases[end]!.join(", ") : t("prop.none"));
    const geometry: PropGroup = {
      key: "geometry",
      label: t("prop.geometry"),
      rows: [
        {
          label: t(typeName(objectTypes(model).frames.get(id) ?? "brace", "frame")),
          value: e.id,
        },
        ...placeProps(model, kind, id, e.label),
        {
          label: t("prop.ends"),
          title: t("side.ends"),
          value: `${e.nodes[0]} → ${e.nodes[1]}`,
        },
        { label: t("side.length"), value: dist(memberLength(model, id)) },
      ],
    };
    const section: Prop = {
      label: t("side.section"),
      value: e.section || t("obj.noSection"),
      edit: namesEdit(
        e.section || undefined,
        model.sections.map((s) => s.id),
        (v) =>
          edit((m) =>
            assignFrame(
              m,
              [id],
              v
                ? {
                    section: v,
                    ...(e.material ? {} : { material: m.materials[0]?.id ?? "" }),
                  }
                : { section: "" },
            ),
          ),
        t("obj.noSection"),
      ),
    };
    const inMesh: Prop = {
      label: t("prop.inMesh"),
      title: t("prop.inMeshLong"),
      value: yesNo(e.in_mesh !== false),
      edit: yesNoEdit(e.in_mesh !== false, (v) => edit((m) => setInMesh(m, [], [id], v))),
    };
    if (!e.section)
      return [
        geometry,
        {
          key: "assignments",
          label: t("prop.assignments"),
          rows: [section, inMesh],
        },
        loadsGroup(model, "frame", id),
      ];
    const ins = e.insertion;
    return [
      geometry,
      {
        key: "assignments",
        label: t("prop.assignments"),
        rows: [
          section,
          {
            label: t("side.material"),
            value: e.material,
            edit: {
              kind: "select",
              value: e.material,
              options: model.materials.map((x) => ({
                value: x.id,
                label: x.id,
              })),
              apply: (v) => edit((m) => assignFrame(m, [id], { material: v })),
            },
          },
          {
            label: t("side.type"),
            value: e.type === "truss" ? t("side.typeTruss") : t("side.typeFrame"),
            edit: {
              kind: "select",
              value: e.type,
              options: [
                { value: "frame", label: t("side.typeFrame") },
                { value: "truss", label: t("side.typeTruss") },
              ],
              apply: (v) => edit((m) => assignFrame(m, [id], { type: v as "frame" | "truss" })),
            },
          },
          {
            label: t("prop.angle"),
            title: t("prop.angleLong"),
            value: `${fmt(e.angle ?? 0)}°`,
            edit: degrees(e.angle ?? 0, (v) => edit((m) => assignFrame(m, [id], { angle: v }))),
          },
          modifiersProp(FRAME_MODIFIERS, e.modifiers, run("frameModifiers")),
          {
            label: t("col.releases"),
            value: e.releases?.i?.length || e.releases?.j?.length ? t("prop.specified") : t("prop.none"),
            rows: [
              { label: t("prop.endI"), value: releases("i") },
              { label: t("prop.endJ"), value: releases("j") },
            ],
            more: run("frameReleases"),
          },
          {
            label: t("prop.offsets"),
            title: t("prop.offsetsLong"),
            value: e.end_offsets === "auto" ? t("prop.automatic") : e.end_offsets ? t("prop.specified") : t("prop.none"),
            rows: [
              { label: t("prop.endI"), value: dist(oi) },
              { label: t("prop.endJ"), value: dist(oj) },
              { label: t("prop.rigidZone"), value: String(e.rigid_zone ?? 0) },
            ],
            more: run("frameOffsets"),
          },
          {
            label: t("prop.insertion"),
            value: `${ins?.point ?? 10} ${t(`ins.p${ins?.point ?? 10}` as Key)}`,
            rows: [
              {
                label: t("prop.mirror2"),
                title: t("ins.mirror2"),
                value: yesNo(!!ins?.mirror2),
              },
              {
                label: t("prop.mirror3"),
                title: t("ins.mirror3"),
                value: yesNo(!!ins?.mirror3),
              },
              {
                label: t("ins.endI"),
                value: (ins?.offset_i ?? [0, 0, 0]).map(dist).join(", "),
              },
              {
                label: t("ins.endJ"),
                value: (ins?.offset_j ?? [0, 0, 0]).map(dist).join(", "),
              },
              {
                label: t("ins.axes"),
                value: t(ins?.global ? "ins.global" : "ins.local"),
              },
              {
                label: t("prop.transform"),
                title: t("prop.transformLong"),
                value: yesNo(!!ins?.transform),
              },
            ],
            more: run("frameInsertion"),
          },
          {
            label: t("col.lineSpring"),
            value: e.line_spring ?? t("prop.none"),
            edit: namesEdit(
              e.line_spring,
              (model.line_spring_props ?? []).map((p) => p.id),
              (v) => edit((m) => assignLineSpring(m, [id], v)),
            ),
          },
          ...wallLabelProps(model, e, { frames: [id] }),
          inMesh,
        ],
      },
      loadsGroup(model, "frame", id),
    ];
  }
  const a = model.areas.find((x) => x.id === id);
  if (!a) return [];
  const sec = model.area_sections.find((s) => s.id === a.section);
  const mesh = a.user_mesh
    ? t("amesh.storedShort", { n: a.user_mesh.elements.length })
    : a.mesh === "none"
      ? t("amesh.noneShort")
      : a.mesh
        ? t("amesh.sizeShort", { size: dist(a.mesh.size) })
        : t("amesh.defaultShort");
  const geometry: PropGroup = {
    key: "geometry",
    label: t("prop.geometry"),
    rows: [
      {
        label: t(typeName(objectTypes(model).areas.get(id) ?? "wall", "area")),
        value: a.id,
      },
      ...placeProps(model, kind, id, a.label),
      { label: t("side.corners"), value: a.nodes.join(", ") },
      {
        label: t("col.area"),
        value: q(areaOf(areaPoints(model, a)), "structureArea"),
      },
    ],
  };
  const section: Prop = {
    label: t("side.areaSection"),
    value: a.section
      ? `${a.section}${sec ? ` (${t(PROPERTY_TYPE_KEYS[sec.property_type ?? "slab"])}, ${sec.type}, ${q(sec.thickness, "sectionLength")})` : ""}`
      : t("obj.noSection"),
    ...(a.opening
      ? {}
      : {
          edit: namesEdit(
            a.section || undefined,
            model.area_sections.map((s) => s.id),
            (v) => edit((m) => assignArea(m, [id], { section: v ?? "" })),
            t("obj.noSection"),
          ),
        }),
  };
  if (a.opening) return [geometry, { key: "assignments", label: t("prop.assignments"), rows: [section] }];
  return [
    geometry,
    {
      key: "assignments",
      label: t("prop.assignments"),
      rows: [
        section,
        {
          label: t("prop.angle"),
          title: t("prop.angleLong"),
          value: `${fmt(a.angle ?? 0)}°`,
          edit: degrees(a.angle ?? 0, (v) => edit((m) => assignArea(m, [id], { angle: v }))),
        },
        ...(a.section ? [modifiersProp(AREA_MODIFIERS, a.modifiers, run("areaModifiers"))] : []),
        {
          label: t("col.diaphragm"),
          value: a.diaphragm ?? t("prop.none"),
          edit: diaphragmEdit(a.diaphragm, [], [id]),
        },
        ...(a.section
          ? [
              {
                label: t("prop.insertion"),
                value: t(`ins.${a.insertion?.point ?? "middle"}`),
                rows: [
                  {
                    label: t("prop.transform"),
                    title: t("prop.transformLong"),
                    value: yesNo(!!a.insertion?.transform),
                  },
                ],
                more: run("areaInsertion"),
              },
              {
                label: t("col.areaSpring"),
                value: a.area_spring ?? t("prop.none"),
                edit: namesEdit(
                  a.area_spring,
                  (model.area_spring_props ?? []).map((p) => p.id),
                  (v) => edit((m) => assignAreaSpring(m, [id], v)),
                ),
              },
              ...wallLabelProps(model, a, { areas: [id] }),
              {
                label: t("col.mesh"),
                value: mesh,
                rows: [
                  {
                    label: t("prop.edgeRestraints"),
                    value: a.edge_restraints === undefined ? t("prop.default") : yesNo(a.edge_restraints),
                  },
                ],
                more: run("areaMesh"),
              },
            ]
          : []),
      ],
    },
    loadsGroup(model, "area", id),
  ];
}

function EditControl({ e, label }: { e: Edit; label: string }) {
  if (e.kind === "select") {
    return (
      <select
        className="prop-edit"
        aria-label={label}
        value={e.value}
        onChange={(ev) => {
          e.apply(ev.target.value);
          // Done: the keys (Ctrl+Z, Delete…) go to the app again.
          ev.target.blur();
        }}
      >
        {e.options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    );
  }
  return <NumberEdit e={e} label={label} />;
}

/** A number typed in place: applied on Enter or when the field is left, Esc puts the value back. */
function NumberEdit({ e, label }: { e: Extract<Edit, { kind: "number" }>; label: string }) {
  const [text, setText] = useState(e.text);
  const [shown, setShown] = useState(e.text);
  // A new value from the model (an undo, another object) replaces what is typed.
  if (shown !== e.text) {
    setShown(e.text);
    setText(e.text);
  }
  const commit = () => {
    if (text === e.text) return;
    const v = e.read(text);
    if (v === null) setText(e.text);
    else e.apply(v);
  };
  const input = (
    <input
      className="prop-edit"
      aria-label={label}
      value={text}
      onChange={(ev) => setText(ev.target.value)}
      onBlur={commit}
      onKeyDown={(ev) => {
        if (ev.key === "Enter")
          ev.currentTarget.blur(); // commits (onBlur); the keys go to the app again
        else if (ev.key === "Escape") {
          setText(e.text);
          ev.stopPropagation();
        }
      }}
    />
  );
  return e.unit ? (
    <>
      {input}
      <span className="prop-unit">{e.unit}</span>
    </>
  ) : (
    input
  );
}

function MoreButton({ label, run }: { label: string; run: () => void }) {
  return (
    <button className="prop-more" aria-label={t("prop.edit", { name: label })} title={t("prop.edit", { name: label })} onClick={run}>
      …
    </button>
  );
}

function PropRow({ p }: { p: Prop }) {
  const [open, setOpen] = useState(false);
  const key = (
    <span className="prop-k" title={p.title}>
      {p.rows && (
        <span className="prop-arrow" aria-hidden="true">
          {p.rows.length ? (open ? "▾" : "▸") : ""}
        </span>
      )}
      {p.label}
    </span>
  );
  const value: ReactNode = p.edit ? <EditControl e={p.edit} label={p.title ?? p.label} /> : <span className="prop-v">{p.value}</span>;
  const more = p.more && <MoreButton label={p.title ?? p.label} run={p.more} />;
  if (!p.rows) {
    return (
      <div className="prop-row">
        {key}
        <span className="prop-cell">
          {value}
          {more}
        </span>
      </div>
    );
  }
  const empty = p.rows.length === 0;
  return (
    <div className="prop-group">
      <div className="prop-row">
        <button className="prop-toggle" aria-expanded={open} disabled={empty} onClick={() => setOpen(!open)}>
          {key}
          <span className="prop-v">{p.value}</span>
        </button>
        {more}
      </div>
      {open && (
        <div className="prop-sub">
          {p.rows.map((r) => (
            <PropRow key={r.label} p={r} />
          ))}
        </div>
      )}
    </div>
  );
}

// Groups closed by the user stay closed for the next object.
const closedGroups = new Set<string>();

function Group({ g }: { g: PropGroup }) {
  const [open, setOpenState] = useState(!closedGroups.has(g.key));
  const setOpen = (v: boolean) => {
    if (v) closedGroups.delete(g.key);
    else closedGroups.add(g.key);
    setOpenState(v);
  };
  return (
    <div className="prop-section">
      <div className="prop-section-head">
        <button className="prop-section-toggle" aria-expanded={open} onClick={() => setOpen(!open)}>
          <span className="prop-arrow" aria-hidden="true">
            {open ? "▾" : "▸"}
          </span>
          {g.label}
        </button>
        {g.more && <MoreButton label={g.label} run={g.more} />}
      </div>
      {open && (
        <div className="prop-section-body">
          {g.rows.map((p) => (
            <PropRow key={p.label} p={p} />
          ))}
        </div>
      )}
    </div>
  );
}

export function ObjectProperties({ model, kind, id }: { model: Model; kind: Kind; id: string }) {
  return (
    <div className="props" aria-label={t("prop.title")}>
      {objectProps(model, kind, id).map((g) => (
        <Group key={g.key} g={g} />
      ))}
    </div>
  );
}
