// Area dialogs: Define > Area Sections, Assign > Area Section / Local Axes, Assign > Area Loads and
// Edit > Divide Areas. The assign and divide dialogs act on the selected areas.

import { useState } from "react";

import { t, tn, type Key } from "../../i18n";
import { assignArea, deleteAreaSection, divideAreas, setAreaLoads, setOpening, upsertAreaSection } from "../../model/areaOps";
import type { LoadMode } from "../../model/ops";
import { DECK_TYPES, PROPERTY_TYPE_KEYS, newDeck, sectionWeight } from "../../model/deck";
import { firstOfType } from "../../model/materials";
import { AREA_MODIFIERS, type AreaModifiers, type AreaPropertyType, type AreaSection, type Deck, type DeckType, type LoadDir, type Model, type ShellType, type SlabType } from "../../model/types";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import { Dialog, NumberField, parseNumber } from "../Dialog";
import { PatternSelect, ModeRadio, NONE, usePattern } from "./AssignDialogs";
import { ListPicker, initialItem } from "./DefineDialogs";
import { SectionModifiers, modifierTexts, readModifierTexts } from "./ModifierDialogs";

const close = () => useStore.getState().openDialog(null);

function OkApplyCancel({ apply }: { apply(ok: boolean): void }) {
  return (
    <>
      <button className="primary" onClick={() => apply(true)}>
        {t("dlg.ok")}
      </button>
      <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
      <button onClick={close}>{t("dlg.cancel")}</button>
    </>
  );
}

export function AreaSectionsDialog() {
  const model = useStore((s) => s.model);
  const [selected, setSelected] = useState<string | null>(() => initialItem(model.area_sections));
  const current = model.area_sections.find((m) => m.id === selected);
  return (
    <Dialog title={t("df.areaSections")} wide buttons={<button onClick={close}>{t("dlg.close")}</button>}>
      <div className="define-layout">
        <ListPicker label={t("df.areaSections")} items={model.area_sections.map((m) => m.id)} selected={selected} onSelect={setSelected} />
        <AreaSectionForm key={selected ?? "(new)"} section={current} onSaved={setSelected} />
      </div>
    </Dialog>
  );
}

const SHELL_TYPES: [ShellType, Key][] = [
  ["shell-thin", "df.shellThin"],
  ["shell-thick", "df.shellThick"],
  ["membrane", "df.membrane"],
];

const PROPERTY_TYPES: AreaPropertyType[] = ["slab", "deck", "wall"];

const SLAB_TYPES: [SlabType, Key][] = [
  ["slab", "df.slabTypeSlab"],
  ["drop", "df.slabTypeDrop"],
  ["stiff", "df.slabTypeStiff"],
];

/** A deck's length fields, in the order shown, and the deck types that use each. */
const DECK_LENGTHS: [Exclude<keyof Deck, "type" | "steel" | "unit_weight">, Key, DeckType[]][] = [
  ["slab_depth", "df.slabDepth", ["filled", "solid"]],
  ["rib_depth", "df.ribDepth", ["filled", "unfilled"]],
  ["rib_width_top", "df.ribWidthTop", ["filled", "unfilled"]],
  ["rib_width_bottom", "df.ribWidthBottom", ["filled", "unfilled"]],
  ["rib_spacing", "df.ribSpacing", ["filled", "unfilled"]],
  ["shear_thickness", "df.shearThickness", ["filled", "unfilled"]],
];

function AreaSectionForm({ section, onSaved }: { section?: AreaSection; onSaved(id: string | null): void }) {
  const model = useStore((s) => s.model);
  const U = useUnits();
  const [id, setId] = useState(section?.id ?? "");
  const [type, setType] = useState<ShellType>(section?.type ?? "shell-thin");
  const [property, setProperty] = useState<AreaPropertyType>(section?.property_type ?? "slab");
  const [material, setMaterial] = useState(section?.material ?? firstOfType(model, "concrete") ?? model.materials[0]?.id ?? "");
  const [thickness, setThickness] = useState(U.show(section?.thickness ?? 0.2, "sectionLength"));
  const [slabType, setSlabType] = useState<SlabType>(section?.slab_type ?? "slab");
  const [oneWay, setOneWay] = useState(section?.one_way ?? false);
  const [mods, setMods] = useState(() => modifierTexts(AREA_MODIFIERS, section?.modifiers));
  // The deck's values as typed (a new deck starts from typical values, on the first steel material if there is one).
  const [deck0] = useState(() => section?.deck ?? newDeck(firstOfType(model, "steel") ?? model.materials[0]?.id ?? ""));
  const [deckType, setDeckType] = useState<DeckType>(deck0.type);
  const [steel, setSteel] = useState(deck0.steel);
  const [deckTexts, setDeckTexts] = useState(() => Object.fromEntries(DECK_LENGTHS.map(([k]) => [k, U.show(deck0[k], "sectionLength")])) as Record<string, string>);
  const [unitWeight, setUnitWeight] = useState(U.show(deck0.unit_weight, "forcePerArea"));
  const [error, setError] = useState("");
  const wall = property === "wall";
  const isDeck = property === "deck";
  /** The deck as typed, or null if a field is not a number (fields its deck type does not use are 0). */
  const readDeckFields = (): Deck | null => {
    const d: Deck = { ...newDeck(steel), type: deckType };
    for (const [k, , uses] of DECK_LENGTHS) {
      const v = uses.includes(deckType) ? U.read(deckTexts[k], "sectionLength") : 0;
      if (v === null) return null;
      d[k] = v;
    }
    const w = deckType === "solid" ? 0 : U.read(unitWeight, "forcePerArea");
    if (w === null) return null;
    d.unit_weight = w;
    return d;
  };
  const typed = isDeck ? readDeckFields() : null;
  const shell = isDeck ? "membrane" : type;
  const save = () => {
    const th = isDeck ? 0 : U.read(thickness, "sectionLength");
    if (th === null || (isDeck && !typed)) return setError(t("dlg.enterNumbers"));
    const modifiers = readModifierTexts(AREA_MODIFIERS, mods, false);
    if (!modifiers) return setError(t("mod.enterAll"));
    const sec: AreaSection = {
      id: id.trim(),
      type: shell,
      property_type: property,
      material,
      thickness: th,
      slab_type: property === "slab" ? slabType : "slab",
      modifiers: modifiers as AreaModifiers,
      one_way: isDeck || (type === "membrane" && oneWay),
      ...(isDeck && typed ? { deck: typed } : {}),
    };
    if (useStore.getState().edit((m) => upsertAreaSection(m, section?.id ?? null, sec))) {
      setError("");
      onSaved(sec.id);
    }
  };
  const remove = () => {
    if (section && useStore.getState().edit((m) => deleteAreaSection(m, section.id))) onSaved(null);
  };
  const materialSelect = (label: string, value: string, set: (v: string) => void, disabled = false) => (
    <label className="row">
      <span>{label}</span>
      <select aria-label={label} value={value} disabled={disabled} onChange={(e) => set(e.target.value)}>
        {model.materials.map((m) => (
          <option key={m.id}>{m.id}</option>
        ))}
      </select>
    </label>
  );
  // The order of the fields: name, shell type, property type, its type, material(s), dimensions.
  return (
    <div className="define-form">
      <h4>{section ? t("df.areaSection", { id: section.id }) : t("df.newAreaSection")}</h4>
      <NumberField label={t("dlg.name")} value={id} onChange={setId} width={140} />
      <label className="row">
        <span>{t("df.shellType")}</span>
        <select aria-label={t("df.shellType")} value={shell} disabled={isDeck} onChange={(e) => setType(e.target.value as ShellType)}>
          {SHELL_TYPES.map(([k, label]) => (
            <option key={k} value={k}>
              {t(label)}
            </option>
          ))}
        </select>
      </label>
      <label className="row">
        <span>{t("df.propertyType")}</span>
        <select aria-label={t("df.propertyType")} value={property} onChange={(e) => setProperty(e.target.value as AreaPropertyType)}>
          {PROPERTY_TYPES.map((k) => (
            <option key={k} value={k}>
              {t(PROPERTY_TYPE_KEYS[k])}
            </option>
          ))}
        </select>
      </label>
      {/* One list whose choices follow the property type; a wall has only "Wall". */}
      {isDeck ? (
        <label className="row">
          <span>{t("df.deckType")}</span>
          <select aria-label={t("df.deckType")} value={deckType} onChange={(e) => setDeckType(e.target.value as DeckType)}>
            {DECK_TYPES.map(([k, label]) => (
              <option key={k} value={k}>
                {t(label)}
              </option>
            ))}
          </select>
        </label>
      ) : (
        <label className="row">
          <span>{wall ? t("df.wallType") : t("df.slabType")}</span>
          <select aria-label={wall ? t("df.wallType") : t("df.slabType")} value={wall ? "wall" : slabType} disabled={wall} onChange={(e) => setSlabType(e.target.value as SlabType)}>
            {wall ? (
              <option value="wall">{t("df.propertyWall")}</option>
            ) : (
              SLAB_TYPES.map(([k, label]) => (
                <option key={k} value={k}>
                  {t(label)}
                </option>
              ))
            )}
          </select>
        </label>
      )}
      {isDeck ? (
        <>
          {materialSelect(t("df.slabMaterial"), material, setMaterial, deckType === "unfilled")}
          {materialSelect(t("df.deckMaterial"), steel, setSteel)}
          {DECK_LENGTHS.map(([k, label, uses]) =>
            uses.includes(deckType) ? <NumberField key={k} label={t(label)} value={deckTexts[k]} onChange={(v) => setDeckTexts({ ...deckTexts, [k]: v })} unit={U.label("sectionLength")} /> : null,
          )}
          {deckType !== "solid" && <NumberField label={t("df.deckUnitWeight")} value={unitWeight} onChange={setUnitWeight} unit={U.label("forcePerArea")} />}
          {typed && (
            <p className="hint note">
              {t("df.deckWeight", { w: `${U.fmt(sectionWeight(model, { id: "", type: "membrane", material, thickness: 0, deck: typed }), "forcePerArea")} ${U.label("forcePerArea")}` })}
            </p>
          )}
        </>
      ) : (
        <>
          {materialSelect(t("as.material"), material, setMaterial)}
          <NumberField label={t("df.thickness")} value={thickness} onChange={setThickness} unit={U.label("sectionLength")} />
        </>
      )}
      <p className="hint">{t("df.areaSectionHint")}</p>
      <p className="hint">{t("df.propertyTypeHint")}</p>
      {property === "slab" && <p className="hint">{t("df.slabTypeHint")}</p>}
      {isDeck && <p className="hint">{t("df.deckHint")}</p>}
      {/* Always shown so it is easy to find; only Membrane sections can use it (greyed out otherwise; a deck always). */}
      <label className={shell === "membrane" && !isDeck ? "check" : "check disabled"}>
        <input type="checkbox" disabled={shell !== "membrane" || isDeck} checked={isDeck || (type === "membrane" && oneWay)} onChange={(e) => setOneWay(e.target.checked)} /> {t("df.oneWay")}
      </label>
      <p className="hint">{t("df.oneWayHint")}</p>
      <SectionModifiers names={AREA_MODIFIERS} values={mods} onChange={setMods} stiff={property === "slab" && slabType === "stiff"} />
      {error && <p className="error">{error}</p>}
      <div className="buttons">
        <button className="primary" onClick={save}>
          {section ? t("dlg.apply") : t("dlg.add")}
        </button>
        {section && <button onClick={remove}>{t("dlg.delete")}</button>}
      </div>
    </div>
  );
}

/** The area sections as options of a list, grouped by property type (slab, deck, then wall sections). */
export function AreaSectionOptions({ sections }: { sections: AreaSection[] }) {
  return (
    <>
      {PROPERTY_TYPES.map((k) => {
        const list = sections.filter((s) => (s.property_type ?? "slab") === k);
        return list.length ? (
          <optgroup key={k} label={t(PROPERTY_TYPE_KEYS[k])}>
            {list.map((s) => (
              <option key={s.id}>{s.id}</option>
            ))}
          </optgroup>
        ) : null;
      })}
    </>
  );
}

export function AreaAssignDialog() {
  const model = useStore((s) => s.model);
  const areas = useStore((s) => s.selection.areas);
  const first = model.areas.find((a) => a.id === areas[0]);
  const sameSection = areas.every((id) => model.areas.find((a) => a.id === id)?.section === first?.section);
  const [section, setSection] = useState(sameSection ? (first && !first.section && !first.opening ? NONE : first?.section ?? "") : "");
  const [opening, setOpeningChoice] = useState<"" | "yes" | "no">("");
  const [angle, setAngle] = useState(areas.length === 1 ? String(first?.angle ?? 0) : "");
  const [reverse, setReverse] = useState(false);
  const [error, setError] = useState("");
  const apply = (ok: boolean) => {
    const a = angle.trim() === "" ? undefined : parseNumber(angle);
    if (a === null) return setError(t("as.angleNumber"));
    const patch = { ...(section ? { section: section === NONE ? "" : section } : {}), ...(a !== undefined ? { angle: a } : {}), reverse };
    // An opening turned back into an area is a null area until it gets a section.
    const edit = (m: Model) => assignArea(opening ? setOpening(m, areas, opening === "yes") : m, areas, patch);
    if (useStore.getState().edit(edit)) {
      setReverse(false);
      if (ok) close();
    }
  };
  return (
    <Dialog title={tn("as.areaAssignTitle", areas.length)} buttons={<OkApplyCancel apply={apply} />}>
      <label className="row">
        <span>{t("as.areaSection")}</span>
        <select aria-label={t("as.areaSection")} value={section} onChange={(e) => setSection(e.target.value)}>
          <option value="">{t("dlg.keep")}</option>
          <AreaSectionOptions sections={model.area_sections} />
          <option value={NONE}>{t("obj.noSection")}</option>
        </select>
      </label>
      <label className="row">
        <span>{t("as.opening")}</span>
        <select aria-label={t("as.opening")} value={opening} onChange={(e) => setOpeningChoice(e.target.value as "" | "yes" | "no")}>
          <option value="">{t("dlg.keep")}</option>
          <option value="yes">{t("dlg.yes")}</option>
          <option value="no">{t("dlg.no")}</option>
        </select>
      </label>
      <NumberField label={t("as.localAngle")} value={angle} onChange={setAngle} unit="°" width={70} />
      <p className="hint">{t("as.areaAngleHint")}</p>
      <label className="check">
        <input type="checkbox" checked={reverse} onChange={(e) => setReverse(e.target.checked)} /> {t("as.reverse")}
      </label>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

const AREA_DIRECTIONS: [string, Key][] = [
  ["G", "as.dirGravity"],
  ["X", "as.dirX"],
  ["Y", "as.dirY"],
  ["Z", "as.dirZ"],
  ["1", "as.dir1"],
  ["2", "as.dir2"],
  ["3", "as.dir3Area"],
];

export function AreaLoadsDialog() {
  const areas = useStore((s) => s.selection.areas);
  const [patternId, setPatternId] = usePattern();
  const [dir, setDir] = useState("G");
  const [value, setValue] = useState("5");
  const [mode, setMode] = useState<LoadMode>("add");
  const [error, setError] = useState("");
  const U = useUnits();
  const apply = (ok: boolean) => {
    const v = U.read(value, "forcePerArea");
    if (v === null) return setError(t("as.enterLoad"));
    if (!patternId) return setError(t("dlg.definePatternFirst"));
    // Gravity loads are typed positive downwards and stored as -Z.
    const spec = { dir: (dir === "G" ? "Z" : dir) as LoadDir, q: dir === "G" ? -v : v };
    useStore.getState().setDisplay({ loadsPattern: patternId });
    if (useStore.getState().edit((m) => setAreaLoads(m, patternId, areas, mode === "delete" ? null : spec, mode)) && ok) close();
  };
  return (
    <Dialog title={tn("as.areaLoadsTitle", areas.length)} buttons={<OkApplyCancel apply={apply} />}>
      <PatternSelect value={patternId} onChange={setPatternId} />
      <label className="row">
        <span>{t("as.direction")}</span>
        <select aria-label={t("as.loadDirection")} value={dir} onChange={(e) => setDir(e.target.value)}>
          {AREA_DIRECTIONS.map(([k, label]) => (
            <option key={k} value={k}>
              {t(label)}
            </option>
          ))}
        </select>
      </label>
      <NumberField label={t("as.loadQ")} value={value} onChange={setValue} unit={U.label("forcePerArea")} />
      <p className="hint">{t("as.areaLoadsHint")}</p>
      <ModeRadio value={mode} onChange={setMode} />
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}

export function DivideAreasDialog() {
  const areas = useStore((s) => s.selection.areas);
  const [n1, setN1] = useState("2");
  const [n2, setN2] = useState("2");
  const [error, setError] = useState("");
  const apply = () => {
    const [a, b] = [Number(n1), Number(n2)];
    if (![a, b].every((n) => Number.isInteger(n) && n >= 1 && n <= 50)) return setError(t("as.divideError"));
    let created: string[] = [];
    const s = useStore.getState();
    const ok = s.edit((m) => {
      const r = divideAreas(m, areas, a, b);
      created = r.ids;
      return r.model;
    });
    if (!ok) return;
    useStore.getState().select(
      created.map((id) => ({ kind: "area" as const, id })),
      "replace",
    );
    close();
  };
  return (
    <Dialog
      title={tn("as.divideTitle", areas.length)}
      buttons={
        <>
          <button className="primary" onClick={apply}>
            {t("dlg.ok")}
          </button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <div className="field-row">
        <NumberField label={t("as.divideN1")} value={n1} onChange={setN1} width={60} />
        <NumberField label={t("as.divideN2")} value={n2} onChange={setN2} width={60} />
      </div>
      <p className="hint">{t("as.divideHint")}</p>
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}
