// Response spectrum functions and cases: reading them from a file and the Define dialogs' edits. The
// analysis itself (and an ASCE 7-16 function's points) is the solver's.

import { t } from "../i18n";
import { EditError, caseAndComboIds, checkNewId, combosUsing, renameNotRun } from "./ops";
import {
  DIRECTIONAL_COMBINATIONS,
  MODAL_COMBINATIONS,
  SITE_CLASSES,
  SPECTRUM_DIRS,
  type Asce716,
  type Id,
  type Model,
  type SpectrumCase,
  type SpectrumFunction,
  type SpectrumLoad,
} from "./types";

/** Standard gravity (m/s²): the scale factor of a function given in g. */
export const G = 9.80665;

/** Modes the modal case gets when the first spectrum case turns it on. */
export const DEFAULT_MODES = 12;

const num = (v: unknown) => (typeof v === "number" ? v : Number(v ?? 0));
const pick = <T extends string>(list: readonly T[], v: unknown): T | undefined => (list.includes(v as T) ? (v as T) : undefined);

/** A function from a file: a table, or ASCE 7-16 when it has `asce7_16`. */
export function readSpectrumFunction(raw: Record<string, unknown>): SpectrumFunction {
  const f: SpectrumFunction = { id: String(raw.id ?? "") };
  if (raw.damping !== undefined && raw.damping !== null) f.damping = num(raw.damping);
  const a = raw.asce7_16 as Record<string, unknown> | undefined;
  if (a && typeof a === "object") {
    const asce: Asce716 = { ss: num(a.ss), s1: num(a.s1), tl: num(a.tl), site_class: pick(SITE_CLASSES, a.site_class) ?? "D" };
    if (a.fa !== undefined && a.fa !== null) asce.fa = num(a.fa);
    if (a.fv !== undefined && a.fv !== null) asce.fv = num(a.fv);
    f.asce7_16 = asce;
  } else f.points = (Array.isArray(raw.points) ? raw.points : []).map((p: unknown[]) => [num(p?.[0]), num(p?.[1])] as [number, number]);
  return f;
}

/** A spectrum case from a file. */
export function readSpectrumCase(raw: Record<string, unknown>): SpectrumCase {
  const c: SpectrumCase = {
    id: String(raw.id ?? ""),
    loads: (Array.isArray(raw.loads) ? raw.loads : []).map((l: Record<string, unknown>) => {
      const out: SpectrumLoad = { dir: pick(SPECTRUM_DIRS, String(l.dir).toUpperCase()) ?? "X", function: String(l.function ?? ""), scale: num(l.scale) };
      if (num(l.angle)) out.angle = num(l.angle);
      return out;
    }),
  };
  if (raw.damping !== undefined && raw.damping !== null) c.damping = num(raw.damping);
  const mc = pick(MODAL_COMBINATIONS, raw.modal_combination);
  if (mc) c.modal_combination = mc;
  const dc = pick(DIRECTIONAL_COMBINATIONS, raw.directional_combination);
  if (dc) c.directional_combination = dc;
  if (raw.abs_factor !== undefined && raw.abs_factor !== null) c.abs_factor = num(raw.abs_factor);
  if (num(raw.eccentricity)) c.eccentricity = num(raw.eccentricity);
  return c;
}

const functionsOf = (m: Model) => m.spectrum_functions ?? [];
const casesOf = (m: Model) => m.spectrum_cases ?? [];

/** Spectrum cases that use a function. */
export function casesUsingFunction(m: Model, id: Id): Id[] {
  return casesOf(m)
    .filter((c) => c.loads.some((l) => l.function === id))
    .map((c) => c.id);
}

/** A function as the model keeps it: the defaults left out, only the fields of its kind. */
function cleanFunction(f: SpectrumFunction): SpectrumFunction {
  const out: SpectrumFunction = { id: f.id.trim() };
  if (f.damping !== undefined && f.damping !== 0.05) out.damping = f.damping;
  if (f.asce7_16) {
    const a = f.asce7_16;
    out.asce7_16 = { ss: a.ss, s1: a.s1, tl: a.tl, site_class: a.site_class };
    if (a.fa !== undefined) out.asce7_16.fa = a.fa;
    if (a.fv !== undefined) out.asce7_16.fv = a.fv;
  } else out.points = (f.points ?? []).map(([p, v]) => [p, v]);
  return out;
}

/** Adds or edits a spectrum function. Renaming follows into the spectrum cases. */
export function setSpectrumFunction(m: Model, oldId: Id | null, f: SpectrumFunction): Model {
  const id = f.id.trim();
  checkNewId(
    functionsOf(m).map((x) => x.id),
    oldId,
    id,
    "spectrumFunction",
  );
  const damping = f.damping ?? 0.05;
  if (!(damping >= 0 && damping < 1)) throw new EditError(t("err.spectrumDamping"));
  if (f.asce7_16) {
    const a = f.asce7_16;
    if (![a.ss, a.s1, a.tl].every((v) => Number.isFinite(v) && v > 0)) throw new EditError(t("err.asceValues"));
    for (const v of [a.fa, a.fv]) if (v !== undefined && !(Number.isFinite(v) && v > 0)) throw new EditError(t("err.asceCoefficients"));
    if (a.site_class === "F" && (a.fa === undefined || a.fv === undefined)) throw new EditError(t("err.asceSiteF"));
  } else {
    const p = f.points ?? [];
    if (!p.length) throw new EditError(t("err.spectrumNoPoints"));
    if (!p.every(([x, v]) => Number.isFinite(x) && x >= 0 && Number.isFinite(v))) throw new EditError(t("err.spectrumPoints"));
    if (p.some((q, k) => k > 0 && q[0] <= p[k - 1][0])) throw new EditError(t("err.spectrumPeriods"));
  }
  const model = structuredClone(m);
  const list = (model.spectrum_functions ??= []);
  const clean = cleanFunction({ ...f, id });
  const i = oldId === null ? -1 : list.findIndex((x) => x.id === oldId);
  if (i >= 0) list[i] = clean;
  else list.push(clean);
  if (oldId !== null && oldId !== id) for (const c of model.spectrum_cases ?? []) for (const l of c.loads) if (l.function === oldId) l.function = id;
  return model;
}

/** Deletes a spectrum function; refused while a spectrum case uses it. */
export function deleteSpectrumFunction(m: Model, id: Id): Model {
  const users = casesUsingFunction(m, id);
  if (users.length) throw new EditError(t("err.functionInCase", { id, list: users.join(", ") }));
  const model = structuredClone(m);
  model.spectrum_functions = functionsOf(model).filter((f) => f.id !== id);
  if (!model.spectrum_functions.length) delete model.spectrum_functions;
  return model;
}

/** A case as the model keeps it: the defaults left out; the absolute factor only for the absolute rule. */
function cleanCase(c: SpectrumCase): SpectrumCase {
  const out: SpectrumCase = {
    id: c.id.trim(),
    loads: c.loads.map((l) => ({ dir: l.dir, function: l.function, scale: l.scale, ...(l.angle ? { angle: l.angle } : {}) })),
  };
  if (c.damping !== undefined && c.damping !== 0.05) out.damping = c.damping;
  if (c.modal_combination && c.modal_combination !== "cqc") out.modal_combination = c.modal_combination;
  if (c.directional_combination && c.directional_combination !== "srss") out.directional_combination = c.directional_combination;
  if (out.directional_combination === "absolute") out.abs_factor = c.abs_factor ?? 1;
  if (c.eccentricity) out.eccentricity = c.eccentricity;
  return out;
}

/**
 * Adds or edits a response spectrum case. Its name is one of the load case names; renaming follows into
 * the combinations. The first spectrum case turns on the modal case when the model has none.
 */
export function setSpectrumCase(m: Model, oldId: Id | null, c: SpectrumCase): Model {
  const id = c.id.trim();
  checkNewId(caseAndComboIds(m), oldId, id, "case");
  if (m.modal?.id === id) throw new EditError(t("err.modalName", { id }));
  if (!c.loads.length) throw new EditError(t("err.spectrumNoLoads"));
  for (const l of c.loads) {
    if (!functionsOf(m).some((f) => f.id === l.function)) throw new EditError(t("err.unknownFunction", { id: l.function }));
    if (!Number.isFinite(l.scale) || !Number.isFinite(l.angle ?? 0)) throw new EditError(t("err.spectrumScale"));
  }
  const damping = c.damping ?? 0.05;
  if (!(damping >= 0 && damping < 1)) throw new EditError(t("err.spectrumDamping"));
  const abs = c.abs_factor ?? 1;
  const ecc = c.eccentricity ?? 0;
  if (!(Number.isFinite(abs) && abs >= 0) || !(Number.isFinite(ecc) && ecc >= 0)) throw new EditError(t("err.spectrumFactors"));
  if (c.directional_combination === "cqc3") {
    const of = (d: string) => c.loads.filter((l) => l.dir === d);
    const [x, y, z] = [of("X"), of("Y"), of("Z")];
    if (x.length !== 1 || y.length !== 1 || z.length > 1 || (x[0].angle ?? 0) !== (y[0].angle ?? 0)) throw new EditError(t("err.cqc3"));
  }
  const model = structuredClone(m);
  const list = (model.spectrum_cases ??= []);
  const clean = cleanCase({ ...c, id });
  const i = oldId === null ? -1 : list.findIndex((x) => x.id === oldId);
  if (i >= 0) list[i] = clean;
  else list.push(clean);
  if (oldId !== null && oldId !== id) for (const k of model.load_combinations) for (const it of k.items) if (it.case === oldId) it.case = id;
  if (oldId !== null && oldId !== id) renameNotRun(model, oldId, id);
  if (!model.modal) model.modal = { id: "Modal", modes: DEFAULT_MODES };
  return model;
}

/** Deletes a spectrum case; refused while a combination uses it. */
export function deleteSpectrumCase(m: Model, id: Id): Model {
  const users = combosUsing(m, "case", id);
  if (users.length) throw new EditError(t("err.caseInCombo", { id, list: users.join(", ") }));
  const model = structuredClone(m);
  model.spectrum_cases = casesOf(model).filter((c) => c.id !== id);
  if (!model.spectrum_cases.length) delete model.spectrum_cases;
  renameNotRun(model, id, null);
  return model;
}
