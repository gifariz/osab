// Sorting and filtering a table's rows by column (Display > Tables). A filter is matched against the
// text a cell shows: a comparison (`> 5`, `<= -2`, `= 0`, `!= 0`) or a range (`1..3`) for numbers,
// otherwise the cells that contain the text (any case).

export type Cell = string | number;

/** The sorted column and direction (1 ascending, -1 descending). */
export interface SortBy {
  col: number;
  dir: 1 | -1;
}

const COMPARE = /^(<=|>=|!=|<|>|=)\s*(-?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)$/i;
const RANGE = /^(-?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)\s*\.\.\s*(-?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)$/i;

/** A filter as a test on a cell's shown text (null: no filter). */
export function cellTest(filter: string): ((shown: string) => boolean) | null {
  const f = filter.trim();
  if (!f) return null;
  const cmp = COMPARE.exec(f);
  if (cmp) {
    const b = Number(cmp[2]);
    const op = cmp[1];
    return (shown) => {
      if (shown.trim() === "") return false;
      const a = Number(shown);
      if (!Number.isFinite(a)) return op === "!=" ? shown !== cmp[2] : op === "=" && shown === cmp[2];
      return op === "<" ? a < b : op === "<=" ? a <= b : op === ">" ? a > b : op === ">=" ? a >= b : op === "=" ? a === b : a !== b;
    };
  }
  const range = RANGE.exec(f);
  if (range) {
    const [lo, hi] = [Number(range[1]), Number(range[2])].sort((x, y) => x - y);
    return (shown) => {
      const a = Number(shown);
      return shown.trim() !== "" && Number.isFinite(a) && a >= lo && a <= hi;
    };
  }
  const needle = f.toLowerCase();
  return (shown) => shown.toLowerCase().includes(needle);
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

/** Orders two cells: numbers by value, text in natural order (A2 before A10), empty cells last. */
export function compareCells(a: Cell, b: Cell): number {
  const ea = a === "";
  const eb = b === "";
  if (ea || eb) return ea === eb ? 0 : ea ? 1 : -1;
  if (typeof a === "number" && typeof b === "number") return a - b;
  if (typeof a === "number") return -1;
  if (typeof b === "number") return 1;
  return collator.compare(a, b);
}

/**
 * The indices of the rows to show: those whose cells pass every column's filter (on the text `shown`
 * gives), in the sort order (a stable sort: equal cells keep the table's order).
 */
export function viewRows(rows: Cell[][], shown: (v: Cell, col: number) => string, filters: string[], sort: SortBy | null): number[] {
  const tests = filters.map(cellTest);
  const active = tests.flatMap((f, k) => (f ? [[k, f] as const] : []));
  const out: number[] = [];
  for (let i = 0; i < rows.length; i++) {
    const r = rows[i];
    if (active.every(([k, f]) => f(shown(r[k] ?? "", k)))) out.push(i);
  }
  if (sort) {
    const { col, dir } = sort;
    out.sort((i, j) => {
      const a = rows[i][col] ?? "";
      const b = rows[j][col] ?? "";
      const c = compareCells(a, b);
      // Empty cells stay last both ways.
      return a === "" || b === "" ? c : dir * c || i - j;
    });
  }
  return out;
}
