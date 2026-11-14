// Define menu: materials, frame sections.

import { useEffect, useRef, useState } from "react";

import { t, type Key } from "../../i18n";

import { deleteMaterial, deleteSection, upsertMaterial, upsertSection } from "../../model/ops";
import type { SectionProps } from "../../model/sections";
import { OPTIONAL_LENGTHS, SHAPES, defaultShape, sectionShape, type FlagKey, type LengthKey, type ShapeKind, type ShapeSpec } from "../../model/sectionShapes";
import { backend, type ShapeProps } from "../../ipc/backend";
import { FRAME_MODIFIERS, type FrameModifiers, type Material, type MaterialType, type Section } from "../../model/types";
import { MATERIAL_TYPES, STRENGTHS, materialType } from "../../model/materials";
import { inputNumber } from "../../scene/format";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import type { Quantity } from "../../units/units";
import { Dialog, NumberField, parseNumber } from "../Dialog";
import { SectionModifiers, modifierTexts, readModifierTexts } from "./ModifierDialogs";

const close = () => useStore.getState().openDialog(null);

/** The item the dialog was opened on (e.g. double-clicked in the explorer), else the first one. */
export const initialItem = (list: { id: string }[]) => {
  const item = useStore.getState().dialogItem;
  return list.some((x) => x.id === item) ? item : (list[0]?.id ?? null);
};

/** A list on the left, a form on the right; `null` selection = adding a new item. */
export function ListPicker(props: { items: string[]; selected: string | null; onSelect(id: string | null): void; label: string }) {
  return (
    <div className="list-picker">
      <select size={8} aria-label={props.label} value={props.selected ?? ""} onChange={(e) => props.onSelect(e.target.value)}>
        {props.items.map((id) => (
          <option key={id} value={id}>
            {id}
          </option>
        ))}
      </select>
      <button onClick={() => props.onSelect(null)}>{t("dlg.addNew")}</button>
    </div>
  );
}

export function MaterialsDialog() {
  const model = useStore((s) => s.model);
  const [selected, setSelected] = useState<string | null>(() => initialItem(model.materials));
  const current = model.materials.find((m) => m.id === selected);
  return (
    <Dialog title={t("df.materials")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("df.materials")} items={model.materials.map((m) => m.id)} selected={selected} onSelect={setSelected} />
        <MaterialForm key={selected ?? "(new)"} material={current} onSaved={setSelected} />
      </div>
    </Dialog>
  );
}

/** Strengths by their usual symbols; the long names are the tooltips. */
const STRENGTH_SYMBOL = { fy: "Fy", fu: "Fu", fc: "f'c" } as const;

function MaterialForm({ material, onSaved }: { material?: Material; onSaved(id: string | null): void }) {
  const U = useUnits();
  const [id, setId] = useState(material?.id ?? "");
  const [E, setE] = useState(U.show(material?.E ?? 200e6, "modulus"));
  const useG = material?.G !== undefined && material?.nu === undefined;
  const [mode, setMode] = useState<"nu" | "G">(useG ? "G" : "nu");
  const [nu, setNu] = useState(inputNumber(material?.nu ?? 0.3));
  const [G, setG] = useState(U.show(material?.G ?? 80e6, "modulus"));
  const [weight, setWeight] = useState(U.show(material?.weight ?? 0, "weightPerVolume"));
  const [type, setType] = useState<MaterialType>(material ? materialType(material) : "steel");
  // Strengths as typed; blank = not given.
  const [strengths, setStrengths] = useState(() => ({ fy: U.show(material?.fy, "stress"), fu: U.show(material?.fu, "stress"), fc: U.show(material?.fc, "stress") }));
  const [error, setError] = useState("");
  const save = () => {
    const e = U.read(E, "modulus");
    const v = mode === "nu" ? parseNumber(nu) : U.read(G, "modulus");
    const w = U.read(weight, "weightPerVolume", true);
    if (e === null || v === null || w === null) return setError(t("dlg.enterNumbers"));
    const given: Partial<Record<"fy" | "fu" | "fc", number>> = {};
    for (const k of STRENGTHS[type]) {
      if (!strengths[k].trim()) continue;
      const x = U.read(strengths[k], "stress");
      if (x === null) return setError(t("dlg.enterNumbers"));
      given[k] = x;
    }
    const mat: Material = { id: id.trim(), type, E: e, ...(mode === "nu" ? { nu: v } : { G: v }), weight: w, ...given };
    if (useStore.getState().edit((m) => upsertMaterial(m, material?.id ?? null, mat))) {
      setError("");
      onSaved(mat.id);
    }
  };
  const remove = () => {
    if (material && useStore.getState().edit((m) => deleteMaterial(m, material.id))) onSaved(null);
  };
  return (
    <div className="define-form">
      <h4>{material ? t("df.material", { id: material.id }) : t("df.newMaterial")}</h4>
      <NumberField label={t("dlg.name")} value={id} onChange={setId} width={140} />
      <label className="row">
        <span>{t("df.materialType")}</span>
        <select aria-label={t("df.materialType")} value={type} onChange={(e) => setType(e.target.value as MaterialType)}>
          {MATERIAL_TYPES.map(([k, label]) => (
            <option key={k} value={k}>
              {t(label)}
            </option>
          ))}
        </select>
      </label>
      <NumberField label={t("df.E")} value={E} onChange={setE} unit={U.label("modulus")} />
      <div className="radio-list horizontal">
        <label>
          <input type="radio" checked={mode === "nu"} onChange={() => setMode("nu")} /> {t("df.poisson")}
        </label>
        <label>
          <input type="radio" checked={mode === "G"} onChange={() => setMode("G")} /> {t("df.shearModulus")}
        </label>
      </div>
      {mode === "nu" ? (
        <NumberField label={t("df.nu")} value={nu} onChange={setNu} />
      ) : (
        <NumberField label={t("df.G")} value={G} onChange={setG} unit={U.label("modulus")} />
      )}
      <NumberField label={t("df.weight")} value={weight} onChange={setWeight} unit={U.label("weightPerVolume")} />
      {STRENGTHS[type].map((k) => (
        <NumberField key={k} label={STRENGTH_SYMBOL[k]} title={t(`df.${k}`)} value={strengths[k]} onChange={(x) => setStrengths({ ...strengths, [k]: x })} unit={U.label("stress")} />
      ))}
      <p className="hint">{t("df.materialHint")}</p>
      <p className="hint">{t("df.strengthHint")}</p>
      {error && <p className="error">{error}</p>}
      <div className="buttons">
        <button className="primary" onClick={save}>
          {material ? t("dlg.apply") : t("dlg.add")}
        </button>
        {material && <button onClick={remove}>{t("dlg.delete")}</button>}
      </div>
    </div>
  );
}


export function SectionsDialog() {
  const model = useStore((s) => s.model);
  const [selected, setSelected] = useState<string | null>(() => initialItem(model.sections));
  const current = model.sections.find((m) => m.id === selected);
  return (
    <Dialog title={t("df.frameSections")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("df.sectionsList")} items={model.sections.map((m) => m.id)} selected={selected} onSelect={setSelected} />
        <SectionForm key={selected ?? "(new)"} section={current} onSaved={setSelected} />
      </div>
    </Dialog>
  );
}

const PROP_FIELDS: { key: keyof SectionProps; label: Key; q: Quantity }[] = [
  { key: "A", label: "df.propA", q: "sectionArea" },
  { key: "I33", label: "df.propI33", q: "length4" },
  { key: "I22", label: "df.propI22", q: "length4" },
  { key: "J", label: "df.propJ", q: "length4" },
  { key: "As2", label: "df.propAs2", q: "sectionArea" },
  { key: "As3", label: "df.propAs3", q: "sectionArea" },
];

type Plain = Exclude<ShapeKind, "aisc">;
type Mode = "general" | ShapeKind;

/** A shape's outline drawn to scale (steel dark, a composite's concrete light), local 3 across, 2 up. */
export function ShapeDrawing({ props }: { props: ShapeProps }) {
  const size = 150;
  const all = [...props.outline, ...(props.fill_outline ?? [])].flat();
  if (!all.length) return null;
  const span = Math.max(props.width, props.depth) || 1;
  const k = (size - 16) / span;
  const path = (polys: [number, number][][]) =>
    polys.map((p) => p.map(([h, v], i) => `${i ? "L" : "M"}${(size / 2 + h * k).toFixed(2)},${(size / 2 - v * k).toFixed(2)}`).join(" ") + " Z").join(" ");
  const [c2, c3] = props.centroid_offset;
  return (
    <svg className="shape-drawing" width={size} height={size} viewBox={`0 0 ${size} ${size}`} role="img" aria-label={t("sh.drawing")}>
      {props.fill_outline?.length ? <path d={path(props.fill_outline)} className="shape-fill" fillRule="evenodd" /> : null}
      <path d={path(props.outline)} className="shape-steel" fillRule="evenodd" />
      <circle cx={size / 2 + c3 * k} cy={size / 2 - c2 * k} r={2.5} className="shape-centroid" />
    </svg>
  );
}

/** The numbers a shape gives, as a table (the solver's values). */
function ShapeTable({ props }: { props: ShapeProps }) {
  const U = useUnits();
  const rows: [string, number, Quantity][] = [
    [t("df.propA"), props.A, "sectionArea"],
    [t("df.propI33"), props.I33, "length4"],
    [t("df.propI22"), props.I22, "length4"],
    ...(props.I23 ? ([[t("sh.propI23"), props.I23, "length4"]] as [string, number, Quantity][]) : []),
    [t("df.propJ"), props.J, "length4"],
    [t("df.propAs2"), props.As2, "sectionArea"],
    [t("df.propAs3"), props.As3, "sectionArea"],
    ...(props.centroid_offset[0] || props.centroid_offset[1]
      ? ([
          [t("sh.centroid2"), props.centroid_offset[0], "sectionLength"],
          [t("sh.centroid3"), props.centroid_offset[1], "sectionLength"],
        ] as [string, number, Quantity][])
      : []),
    ...(props.self_weight ? ([[t("sh.selfWeight"), props.self_weight, "forcePerLength"]] as [string, number, Quantity][]) : []),
  ];
  return (
    <table className="props" data-testid="shape-props">
      <tbody>
        {rows.map(([label, v, q]) => (
          <tr key={label}>
            <td>{label}</td>
            <td>
              {U.fmt(v, q)} {U.label(q)}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function SectionForm({ section, onSaved }: { section?: Section; onSaved(id: string | null): void }) {
  const U = useUnits();
  const model = useStore((s) => s.model);
  const [id, setId] = useState(section?.id ?? "");
  const initialShape = section ? sectionShape(model, section) : defaultShape("rectangle");
  const [mode, setMode] = useState<Mode>(initialShape?.type ?? "general");
  // Lengths as typed (display units), by key; the core of an encased section under "core.".
  const show = (v: number | undefined) => (v === undefined ? "" : U.show(v, "sectionLength"));
  const textsOf = (s: ShapeSpec): Record<string, string> => {
    const out: Record<string, string> = {};
    for (const k of LENGTHS) out[k] = show(s[k]);
    for (const k of CORE_LENGTHS) out[`core.${k}`] = show(s.core?.[k]);
    return out;
  };
  const [texts, setTexts] = useState<Record<string, string>>(() => textsOf(initialShape?.type === "aisc" || !initialShape ? defaultShape("rectangle") : initialShape));
  // The steel list: its shapes (loaded once), the type shown, a search and the chosen name.
  const [list, setList] = useState<[string, string][] | null>(null);
  const [listType, setListType] = useState(() => (initialShape?.type === "aisc" ? (initialShape.name ?? "").replace(/[0-9].*$/, "") || "W" : "W"));
  const [search, setSearch] = useState("");
  const [listName, setListName] = useState(initialShape?.type === "aisc" ? (initialShape.name ?? "") : "");
  useEffect(() => {
    if (mode !== "aisc" || list) return;
    backend.sectionList().then(setList, (e) => setError(e instanceof Error ? e.message : String(e)));
  }, [mode, list]);
  const [flags, setFlags] = useState<Record<FlagKey, boolean>>(() => ({
    mirror: !!initialShape?.mirror,
    mirror2: !!initialShape?.mirror2,
    mirror3: !!initialShape?.mirror3,
  }));
  const steelDefault = model.materials.find((m) => /steel|a992|stl/i.test(m.id))?.id ?? model.materials[0]?.id ?? "";
  const concreteDefault = model.materials.find((m) => /con|fc/i.test(m.id))?.id ?? model.materials[0]?.id ?? "";
  const [steel, setSteel] = useState(initialShape?.material ?? steelDefault);
  const [fill, setFill] = useState(initialShape?.fill ?? initialShape?.encasement ?? concreteDefault);
  const [props, setProps] = useState<Record<keyof SectionProps, string>>({
    A: U.show(section?.A, "sectionArea"),
    I33: U.show(section?.I33, "length4"),
    I22: U.show(section?.I22, "length4"),
    J: U.show(section?.J, "length4"),
    As2: U.show(section?.As2 ?? 0, "sectionArea"),
    As3: U.show(section?.As3 ?? 0, "sectionArea"),
  });
  // A general section's depth and width (for automatic end offsets; blank = none).
  const [size, setSize] = useState({ d: section?.depth ? U.show(section.depth, "sectionLength") : "", b: section?.width ? U.show(section.width, "sectionLength") : "" });
  const [mods, setMods] = useState(() => modifierTexts(FRAME_MODIFIERS, section?.modifiers));
  const [error, setError] = useState("");
  const [computed, setComputed] = useState<{ key: string; props?: ShapeProps; error?: string } | null>(null);
  // A save waiting for the solver's numbers still adds the section after the form is left (Add New…),
  // but no longer selects it.
  const alive = useRef(true);
  useEffect(
    () => () => {
      alive.current = false;
    },
    [],
  );

  /** The shape as typed, or a message saying what is missing. */
  const spec = (): ShapeSpec | string => {
    if (mode === "general") return "";
    if (mode === "aisc") return listName ? { type: "aisc", name: listName } : t("sh.pickList");
    const info = SHAPES[mode];
    const s: ShapeSpec = { type: mode };
    for (const [k] of info.lengths) {
      const blank = texts[k].trim() === "";
      if (blank && OPTIONAL_LENGTHS.includes(k)) continue;
      const v = U.read(texts[k], "sectionLength");
      if (v === null || !(v > 0 || (k === "gap" && v === 0))) return t("df.dimsPositive");
      s[k] = v;
    }
    for (const [k] of info.flags) if (flags[k]) s[k] = true;
    if (info.fill) {
      s.material = steel;
      s[info.fill] = fill;
    }
    if (mode === "encased_rectangle" || mode === "encased_circle") {
      const core: ShapeSpec = { type: "i" };
      for (const k of CORE_LENGTHS) {
        const v = U.read(texts[`core.${k}`], "sectionLength");
        if (v === null || !(v > 0)) return t("df.dimsPositive");
        core[k] = v;
      }
      s.core = core;
    }
    return s;
  };
  const current = spec();
  const key = typeof current === "string" ? current : JSON.stringify(current);
  const materials = model.materials;
  useEffect(() => {
    if (typeof current === "string") return;
    let stale = false;
    const timer = setTimeout(() => {
      backend.section(current, materials, model.units?.length).then(
        (p) => !stale && setComputed({ key, props: p }),
        (e) => !stale && setComputed({ key, error: e instanceof Error ? e.message : String(e) }),
      );
    }, 150);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, materials]);
  const shown = computed?.key === key ? computed : null;

  const save = async () => {
    const modifiers = readModifierTexts(FRAME_MODIFIERS, mods, false);
    if (!modifiers) return setError(t("mod.enterNumbers"));
    if (mode === "general") {
      const v = Object.fromEntries(PROP_FIELDS.map((f) => [f.key, U.read(props[f.key], f.q, f.key.startsWith("As"))]));
      if (Object.values(v).some((x) => x === null)) return setError(t("df.allNumbers"));
      const sec: Section = { id: id.trim(), ...(v as unknown as SectionProps), modifiers: modifiers as FrameModifiers };
      const [d, b] = [U.read(size.d, "sectionLength", true), U.read(size.b, "sectionLength", true)];
      if (d === null || b === null) return setError(t("df.allNumbers"));
      if (d > 0) sec.depth = d;
      if (b > 0) sec.width = b;
      if (useStore.getState().edit((m) => upsertSection(m, section?.id ?? null, sec, null))) {
        setError("");
        onSaved(sec.id);
      }
      return;
    }
    if (typeof current === "string") return setError(current);
    let p: ShapeProps;
    try {
      p = shown?.props ?? (await backend.section(current, materials, model.units?.length));
    } catch (e) {
      return alive.current ? setError(e instanceof Error ? e.message : String(e)) : undefined;
    }
    const sec: Section = {
      id: id.trim(),
      A: p.A,
      I22: p.I22,
      I33: p.I33,
      J: p.J,
      As2: p.As2,
      As3: p.As3,
      depth: p.depth,
      width: p.width,
      modifiers: modifiers as FrameModifiers,
      ...(p.I23 ? { I23: p.I23 } : {}),
      centroid_offset: p.centroid_offset,
      ...(p.self_weight ? { self_weight: p.self_weight } : {}),
    };
    const keep: ShapeSpec = current.type === "aisc" && p.like ? { ...current, like: p.like } : current;
    if (useStore.getState().edit((m) => upsertSection(m, section?.id ?? null, sec, keep)) && alive.current) {
      setError("");
      onSaved(sec.id);
    }
  };
  const remove = () => {
    if (section && useStore.getState().edit((m) => deleteSection(m, section.id))) onSaved(null);
  };
  const changeMode = (m: Mode) => {
    setMode(m);
    if (m !== "general" && m !== "aisc") setTexts(textsOf(defaultShape(m)));
  };
  const length = (k: string, label: string) => (
    <NumberField key={k} label={label} value={texts[k] ?? ""} onChange={(v) => setTexts({ ...texts, [k]: v })} unit={U.label("sectionLength")} width={90} />
  );
  const materialSelect = (label: string, value: string, onChange: (v: string) => void) => (
    <label className="row">
      <span>{label}</span>
      <select aria-label={label} value={value} onChange={(e) => onChange(e.target.value)}>
        {model.materials.map((m) => (
          <option key={m.id} value={m.id}>
            {m.id}
          </option>
        ))}
      </select>
    </label>
  );
  const info = mode === "general" || mode === "aisc" ? null : SHAPES[mode];
  const types = list ? [...new Set(list.map(([, k]) => k))] : [];
  const needle = search.trim().toUpperCase();
  const names = (list ?? []).filter(([n, k]) => (needle ? n.toUpperCase().includes(needle) : k === listType)).map(([n]) => n);
  const result =
    typeof current === "string" ? (
      <p className="error">{current}</p>
    ) : shown?.error ? (
      <p className="error">{shown.error}</p>
    ) : shown?.props ? (
      <div className="shape-result">
        <ShapeDrawing props={shown.props} />
        <ShapeTable props={shown.props} />
      </div>
    ) : (
      <p className="hint note">{t("sh.working")}</p>
    );
  return (
    <div className="define-form">
      <h4>{section ? t("df.section", { id: section.id }) : t("df.newSection")}</h4>
      <NumberField label={t("dlg.name")} value={id} onChange={setId} width={140} />
      <label className="row">
        <span>{t("df.shape")}</span>
        <select aria-label={t("df.sectionShape")} value={mode} onChange={(e) => changeMode(e.target.value as Mode)}>
          <option value="general">{t("df.shapeGeneral")}</option>
          <option value="aisc">{t("sh.aisc")}</option>
          {(Object.keys(SHAPES) as Plain[]).map((k) => (
            <option key={k} value={k}>
              {t(SHAPES[k].label)}
            </option>
          ))}
        </select>
      </label>
      {mode === "aisc" ? (
        <>
          {!list ? (
            <p className="hint note">{t("sh.working")}</p>
          ) : (
            <div className="list-pick">
              <label className="row">
                <span>{t("sh.listType")}</span>
                <select aria-label={t("sh.listType")} value={listType} onChange={(e) => setListType(e.target.value)}>
                  {types.map((k) => (
                    <option key={k} value={k}>
                      {k}
                    </option>
                  ))}
                </select>
              </label>
              <NumberField label={t("sh.listSearch")} value={search} onChange={setSearch} width={140} />
              <select size={8} aria-label={t("sh.listName")} value={listName} onChange={(e) => setListName(e.target.value)}>
                {names.map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
            </div>
          )}
          <p className="hint">{t("sh.aiscHint")}</p>
          {listName && result}
        </>
      ) : info ? (
        <>
          <div className="field-grid">{info.lengths.map(([k, label]) => length(k, t(label)))}</div>
          {info.lengths.some(([k]) => OPTIONAL_LENGTHS.includes(k)) && <p className="hint">{t("sh.optional")}</p>}
          {(mode === "encased_rectangle" || mode === "encased_circle") && (
            <>
              <p className="hint note">{t("sh.core")}</p>
              <div className="field-grid">{CORE_LENGTHS.map((k) => length(`core.${k}`, t(CORE_LABELS[k])))}</div>
            </>
          )}
          {info.flags.map(([k, label]) => (
            <label key={k} className="check">
              <input type="checkbox" checked={flags[k]} onChange={(e) => setFlags({ ...flags, [k]: e.target.checked })} /> {t(label)}
            </label>
          ))}
          {info.fill && (
            <>
              {materialSelect(t("sh.steelMaterial"), steel, setSteel)}
              {materialSelect(t(info.fill === "fill" ? "sh.fillMaterial" : "sh.encasementMaterial"), fill, setFill)}
              <p className="hint">{t("sh.compositeHint")}</p>
            </>
          )}
          {typeof current === "string" ? (
            <p className="error">{current}</p>
          ) : shown?.error ? (
            <p className="error">{shown.error}</p>
          ) : shown?.props ? (
            <div className="shape-result">
              <ShapeDrawing props={shown.props} />
              <ShapeTable props={shown.props} />
            </div>
          ) : (
            <p className="hint note">{t("sh.working")}</p>
          )}
        </>
      ) : (
        <div className="field-grid">
          <NumberField label={t("df.depth2")} value={size.d} onChange={(v) => setSize({ ...size, d: v })} unit={U.label("sectionLength")} width={110} />
          <NumberField label={t("df.width3")} value={size.b} onChange={(v) => setSize({ ...size, b: v })} unit={U.label("sectionLength")} width={110} />
          {PROP_FIELDS.map((f) => (
            <NumberField
              key={f.key}
              label={t(f.label)}
              value={props[f.key]}
              onChange={(v) => setProps({ ...props, [f.key]: v })}
              unit={U.label(f.q)}
              width={110}
            />
          ))}
        </div>
      )}
      <SectionModifiers names={FRAME_MODIFIERS} values={mods} onChange={setMods} />
      {error && <p className="error">{error}</p>}
      <div className="buttons">
        <button className="primary" onClick={() => void save()}>
          {section ? t("dlg.apply") : t("dlg.add")}
        </button>
        {section && <button onClick={remove}>{t("dlg.delete")}</button>}
      </div>
    </div>
  );
}

const LENGTHS: LengthKey[] = ["depth", "width", "flange", "web", "bottom_width", "bottom_flange", "gap", "web_tip", "diameter", "wall"];
const CORE_LENGTHS: ("depth" | "width" | "flange" | "web")[] = ["depth", "width", "flange", "web"];
const CORE_LABELS: Record<(typeof CORE_LENGTHS)[number], Key> = {
  depth: "sh.coreDepth",
  width: "sh.coreWidth",
  flange: "sh.coreFlange",
  web: "sh.coreWeb",
};

