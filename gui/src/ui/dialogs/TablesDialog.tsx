// Display > Tables. First the tables are chosen in a tree (model definition, analysis results) with the
// load patterns and the load cases / combinations they show; only on OK are they built, and only then are
// the chosen cases' values fetched (results on demand). The table viewer sorts and filters by column, draws
// only the rows in view of a long table, and copies what it shows as CSV.

import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { t } from "../../i18n";
import { backend } from "../../ipc/backend";
import type { CaseResult, Id } from "../../model/types";
import { resultKey, resultLabel } from "../../scene/buildResultsScene";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import { Dialog } from "../Dialog";
import { viewRows, type Cell, type SortBy } from "../tables/grid";
import { workbook } from "../tables/xlsx";
import { PATTERN_TABLES, TABLE_LABEL, TABLE_TREE, VALUE_TABLES, CASE_TABLES, buildTable, hasTable, type TableData, type TableGroup, type TableId } from "../tables/tableData";

const num = (v: number) => (v === 0 ? "0" : Number(v.toPrecision(6)).toString());

/** What was chosen last (this session): tables, load patterns and cases (null: all of them). */
interface Choice {
  tables: TableId[];
  patterns: Id[] | null;
  cases: string[] | null;
}
let remembered: Choice = { tables: [], patterns: null, cases: null };

const close = () => useStore.getState().openDialog(null);

export function TablesDialog() {
  const [choice, setChoice] = useState<Choice | null>(null);
  if (!choice)
    return (
      <ChooseTables
        onOk={(c) => {
          remembered = c;
          setChoice(c);
        }}
      />
    );
  return <TableViewer choice={choice} onChoose={() => setChoice(null)} />;
}

/** The ids of a group's tables that this model and these results have. */
function tablesOf(g: TableGroup, has: (id: TableId) => boolean): TableId[] {
  return g.items.flatMap((it) => (typeof it === "string" ? (has(it) ? [it] : []) : tablesOf(it, has)));
}

/** A tick box that shows "some" (indeterminate) when part of a group is ticked. */
function Tick(props: { label: string; checked: boolean; some?: boolean; onChange: (on: boolean) => void }) {
  const ref = useRef<HTMLInputElement>(null);
  useLayoutEffect(() => {
    if (ref.current) ref.current.indeterminate = !!props.some && !props.checked;
  });
  return (
    <label className="tree-row">
      <input ref={ref} type="checkbox" aria-label={props.label} checked={props.checked} onChange={(e) => props.onChange(e.target.checked)} /> {props.label}
    </label>
  );
}

function ChooseTables(props: { onOk: (c: Choice) => void }) {
  const model = useStore((s) => s.model);
  const results = useStore((s) => s.results);
  const has = (id: TableId) => hasTable(id, model, results);
  const all = TABLE_TREE.flatMap((g) => tablesOf(g, has));
  const [tables, setTables] = useState<TableId[]>(() => remembered.tables.filter(has));
  const patternIds = model.load_patterns.map((p) => p.id);
  const caseKeys = (results?.cases ?? []).map(resultKey);
  const [patterns, setPatterns] = useState<Id[]>(() => (remembered.patterns ?? patternIds).filter((id) => patternIds.includes(id)));
  const [cases, setCases] = useState<string[]>(() => (remembered.cases ?? caseKeys).filter((k) => caseKeys.includes(k)));
  const set = (ids: TableId[], on: boolean) => setTables((cur) => (on ? all.filter((id) => cur.includes(id) || ids.includes(id)) : cur.filter((id) => !ids.includes(id))));
  const group = (g: TableGroup, depth: number): ReactNode => {
    const ids = tablesOf(g, has);
    if (!ids.length) return null;
    const ticked = ids.filter((id) => tables.includes(id)).length;
    return (
      <li key={g.label}>
        <Tick label={t(g.label)} checked={ticked === ids.length} some={ticked > 0} onChange={(on) => set(ids, on)} />
        <ul className="tree" style={{ paddingLeft: depth ? 18 : 14 }}>
          {g.items.map((it) =>
            typeof it === "string" ? (
              has(it) && (
                <li key={it}>
                  <Tick label={t(TABLE_LABEL[it])} checked={tables.includes(it)} onChange={(on) => set([it], on)} />
                </li>
              )
            ) : (
              group(it, depth + 1)
            ),
          )}
        </ul>
      </li>
    );
  };
  const usesPatterns = tables.some((id) => PATTERN_TABLES.includes(id));
  const usesCases = tables.some((id) => CASE_TABLES.includes(id));
  const ok = () =>
    props.onOk({
      tables,
      patterns: patterns.length === patternIds.length ? null : patterns,
      cases: cases.length === caseKeys.length ? null : cases,
    });
  const picker = (label: string, aria: string, options: [string, string][], value: string[], onChange: (v: string[]) => void, enabled: boolean) => (
    <div className="table-filter">
      <div className="row">
        <span>{label}</span>
        <button disabled={!enabled} onClick={() => onChange(options.map(([v]) => v))}>
          {t("tbl.selectAll")}
        </button>
      </div>
      <select
        multiple
        aria-label={aria}
        disabled={!enabled}
        size={Math.min(10, Math.max(4, options.length))}
        value={value}
        onChange={(e) => onChange([...e.target.selectedOptions].map((o) => o.value))}
      >
        {options.map(([v, text]) => (
          <option key={v} value={v}>
            {text}
          </option>
        ))}
      </select>
    </div>
  );
  return (
    <Dialog
      title={t("tbl.chooseTitle")}
      wide
      buttons={
        <>
          <button className="primary" disabled={!tables.length || (usesCases && !!results && !cases.length)} onClick={ok}>
            {t("dlg.ok")}
          </button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <div className="table-chooser">
        <ul className="tree tables-tree" aria-label={t("tbl.treeLabel")}>
          {TABLE_TREE.map((g) => group(g, 0))}
        </ul>
        <div>
          {patternIds.length > 0 &&
            picker(t("tbl.patternsLabel"), t("tbl.patternsSelect"), patternIds.map((id) => [id, id]), patterns, setPatterns, usesPatterns)}
          {results &&
            picker(
              t("tbl.casesLabel"),
              t("tbl.casesSelect"),
              results.cases.map((c) => [resultKey(c), resultLabel(c)]),
              cases,
              setCases,
              usesCases,
            )}
          {!results && <p className="hint note">{t("tbl.runFirst")}</p>}
          <p className="hint">{t("tbl.chooseHint")}</p>
        </div>
      </div>
    </Dialog>
  );
}

/** A model file's name without its extension (the default name of an export). */
const baseOf = (name: string) => name.replace(/\.[^.]*$/, "") || "OSAB";

/** Saves an export and says where (or why not). */
async function saveExport(name: string, data: Uint8Array, filter: { name: string; extensions: string[] }) {
  try {
    const saved = await backend.saveFile(name, data, filter);
    if (saved) useStore.getState().notify(t("tbl.exported", { name: saved }));
  } catch (e) {
    useStore.getState().notify(e instanceof Error ? e.message : String(e), "error");
  }
}

/**
 * A table as sheet rows: its name, the column names, their units (a heading "UX (mm)" is the name UX and
 * the unit mm), then the rows with numbers in the units shown.
 */
export function sheetRows(title: string, d: TableData, value: (v: Cell, k: number) => Cell): { rows: Cell[][]; boldRows: number } {
  const split = d.head.map((h) => /^(.*) \(([^()]*)\)$/.exec(h));
  return {
    rows: [[`TABLE: ${title}`], split.map((m, k) => (m ? m[1] : d.head[k])), split.map((m) => (m ? m[2] : "")), ...d.rows.map((r) => r.map(value))],
    boldRows: 3,
  };
}

/** Table rows drawn at once; a longer table draws only the rows in view (rows of this height). */
const ALL_ROWS = 500;
const ROW_HEIGHT = 22;

function TableViewer(props: { choice: Choice; onChoose: () => void }) {
  const model = useStore((s) => s.model);
  const results = useStore((s) => s.results);
  const run = useStore((s) => s.resultsRun);
  const U = useUnits();
  const has = (id: TableId) => hasTable(id, model, results);
  const tables = props.choice.tables.filter(has);
  const [table, setTable] = useState<TableId | undefined>(tables[0]);
  const active = table && tables.includes(table) ? table : tables[0];
  // The chosen cases (all when none were chosen), and their values for the tables that need them.
  const heads = useMemo(
    () => (results?.cases ?? []).flatMap((c, i) => (!props.choice.cases || props.choice.cases.includes(resultKey(c)) ? [{ c, i }] : [])),
    [results, props.choice.cases],
  );
  const needValues = !!results && tables.some((id) => VALUE_TABLES.includes(id));
  const [values, setValues] = useState<{ run: number; cases: Map<number, CaseResult> }>({ run, cases: new Map() });
  const [loadError, setLoadError] = useState("");
  useEffect(() => {
    if (!needValues) return;
    let stopped = false;
    const got = new Map<number, CaseResult>();
    setValues({ run, cases: got });
    setLoadError("");
    // The cases as they are now: loading a case elsewhere changes the results object, not the run.
    const now = useStore.getState().results?.cases ?? [];
    const chosen = now.flatMap((c, i) => (!props.choice.cases || props.choice.cases.includes(resultKey(c)) ? [{ c, i }] : []));
    void (async () => {
      for (const { c, i } of chosen) {
        if (stopped) return;
        try {
          got.set(i, c.pending ? await backend.resultCase(i) : c);
        } catch (e) {
          if (!stopped) setLoadError(e instanceof Error ? e.message : String(e));
          return;
        }
        if (!stopped) setValues({ run, cases: new Map(got) });
      }
    })();
    return () => {
      stopped = true;
    };
  }, [needValues, run, props.choice.cases]);
  const loaded = values.run === run ? values.cases : new Map<number, CaseResult>();
  const loading = needValues && loaded.size < heads.length;
  const build = (id: TableId) =>
    buildTable(id, {
      model,
      results,
      cases: heads.flatMap(({ i }) => (loaded.has(i) ? [loaded.get(i)!] : [])),
      heads: heads.map(({ c }) => c),
      patterns: props.choice.patterns ? new Set(props.choice.patterns) : null,
      U,
    });
  const data = useMemo<TableData | null>(() => {
    if (!active) return null;
    if (VALUE_TABLES.includes(active) && loading) return null;
    return build(active);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, loading, model, results, heads, loaded, props.choice.patterns, U]);
  /** Every chosen table into one workbook, a sheet each (all rows, in the table's order). */
  const exportExcel = async () => {
    const sheets = tables.map((id) => {
      const d = build(id);
      return { name: t(TABLE_LABEL[id]), ...sheetRows(t(TABLE_LABEL[id]), d, (v, k) => (typeof v === "number" && d.qs[k] ? U.value(v, d.qs[k]!) : v)) };
    });
    await saveExport(`${baseOf(useStore.getState().fileName)} tables.xlsx`, workbook(sheets), { name: "Excel", extensions: ["xlsx"] });
  };
  const spectrumShown = !!active && CASE_TABLES.includes(active) && heads.some(({ c }) => c.spectrum);
  return (
    <Dialog
      title={t("tbl.title")}
      wide
      buttons={
        <>
          <button onClick={props.onChoose}>{t("tbl.choose")}</button>
          <button disabled={!tables.length || loading} onClick={() => void exportExcel()}>
            {t("tbl.exportExcel")}
          </button>
          <button onClick={close}>{t("dlg.close")}</button>
        </>
      }
    >
      <div className="tabs">
        {tables.map((id) => (
          <button key={id} className={active === id ? "tab active" : "tab"} onClick={() => setTable(id)}>
            {t(TABLE_LABEL[id])}
          </button>
        ))}
      </div>
      {!tables.length && <p className="hint note">{t("tbl.noneLeft")}</p>}
      {active === "reactions" && model.springs.length > 0 && <p className="hint note">{t("tbl.springReactions")}</p>}
      {active === "areaForces" && <p className="hint">{t("tbl.areaForcesHint")}</p>}
      {active === "baseReactions" && <p className="hint">{t("tbl.baseReactionsHint")}</p>}
      {(active === "storyDrifts" || active === "jointDrifts") && <p className="hint">{t("tbl.driftsHint")}</p>}
      {active === "storyForces" && <p className="hint">{t("tbl.storyForcesHint")}</p>}
      {(active === "pierForces" || active === "spandrelForces") && <p className="hint">{t("tbl.pierForcesHint")}</p>}
      {spectrumShown && <p className="hint">{t("tbl.spectrumHint")}</p>}
      {(active === "frameModifiers" || active === "areaModifiers") && <p className="hint">{t("tbl.modifiersHint")}</p>}
      {loadError && <p className="error">{loadError}</p>}
      {active && !data && !loadError && <p className="hint note">{t("tbl.loading", { done: loaded.size, all: heads.length })}</p>}
      {active && data && <Grid key={active} id={active} data={data} />}
    </Dialog>
  );
}

/** One table: column headers sort it (ascending, descending, back to the table's order); the row under them filters each column. */
function Grid(props: { id: TableId; data: TableData }) {
  const { head, qs, rows } = props.data;
  const U = useUnits();
  const [sort, setSort] = useState<SortBy | null>(null);
  const [filters, setFilters] = useState<string[]>(() => head.map(() => ""));
  const shown = (v: Cell, k: number) => (typeof v === "number" ? num(qs[k] ? U.value(v, qs[k]) : v) : v);
  const order = useMemo(() => viewRows(rows, shown, filters, sort), [rows, filters, sort, U, qs]); // eslint-disable-line react-hooks/exhaustive-deps
  const wrap = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState({ top: 0, height: 600 });
  const virtual = order.length > ALL_ROWS;
  const first = virtual ? Math.max(0, Math.floor(scroll.top / ROW_HEIGHT) - 10) : 0;
  const last = virtual ? Math.min(order.length, first + Math.ceil(scroll.height / ROW_HEIGHT) + 30) : order.length;
  const clickHead = (k: number) => setSort((s) => (s?.col !== k ? { col: k, dir: 1 } : s.dir === 1 ? { col: k, dir: -1 } : null));
  const csvText = () => {
    const line = (r: Cell[]) => r.map((v, k) => `"${String(shown(v, k)).replace(/"/g, '""')}"`).join(",");
    return [line(head), ...order.map((i) => line(rows[i]))].join("\n");
  };
  /** The table as shown (filters and order) as a CSV file (UTF-8 with a byte-order mark, which Excel reads). */
  const saveCsv = () =>
    void saveExport(`${baseOf(useStore.getState().fileName)} ${t(TABLE_LABEL[props.id])}.csv`, new TextEncoder().encode("\ufeff" + csvText() + "\n"), {
      name: "CSV",
      extensions: ["csv"],
    });
  const copy = () => {
    const csv = csvText();
    void navigator.clipboard.writeText(csv).then(
      () => useStore.getState().notify(t("tbl.copied")),
      () => useStore.getState().notify(t("tbl.copyFailed"), "error"),
    );
  };
  return (
    <>
      <div className="row table-tools">
        <span className="hint note">{order.length === rows.length ? t("tbl.rows", { n: rows.length }) : t("tbl.rowsOf", { n: order.length, all: rows.length })}</span>
        {filters.some((f) => f.trim()) && <button onClick={() => setFilters(head.map(() => ""))}>{t("tbl.clearFilters")}</button>}
        <button onClick={copy}>{t("tbl.copy")}</button>
        <button onClick={saveCsv}>{t("tbl.saveCsv")}</button>
      </div>
      <div
        className="table-wrap"
        ref={wrap}
        onScroll={(e) => virtual && setScroll({ top: e.currentTarget.scrollTop, height: e.currentTarget.clientHeight })}
      >
        <table className="data" data-testid={`table-${props.id}`}>
          <thead>
            <tr>
              {head.map((h, k) => (
                <th key={k} aria-sort={sort?.col === k ? (sort.dir === 1 ? "ascending" : "descending") : "none"}>
                  <button className="sort-head" title={t("tbl.sortTitle")} onClick={() => clickHead(k)}>
                    {h} {sort?.col === k ? (sort.dir === 1 ? "▲" : "▼") : ""}
                  </button>
                </th>
              ))}
            </tr>
            <tr className="filters">
              {head.map((h, k) => (
                <th key={k}>
                  <input
                    aria-label={t("tbl.filter", { col: h })}
                    placeholder={t("tbl.filterHint")}
                    value={filters[k] ?? ""}
                    onChange={(e) => {
                      const v = e.target.value;
                      setFilters((f) => f.map((x, j) => (j === k ? v : x)));
                      wrap.current?.scrollTo({ top: 0 });
                    }}
                  />
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {first > 0 && <tr className="spacer" aria-hidden style={{ height: first * ROW_HEIGHT }} />}
            {order.slice(first, last).map((i) => (
              <tr key={i} style={virtual ? { height: ROW_HEIGHT } : undefined}>
                {rows[i].map((v, k) => (
                  <td key={k} className={typeof v === "number" ? "num" : ""}>
                    {shown(v, k)}
                  </td>
                ))}
              </tr>
            ))}
            {last < order.length && <tr className="spacer" aria-hidden style={{ height: (order.length - last) * ROW_HEIGHT }} />}
          </tbody>
        </table>
      </div>
    </>
  );
}
