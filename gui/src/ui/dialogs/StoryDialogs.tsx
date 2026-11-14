// Define > Story Data: the story table (name and height of each story, top down, and the base's
// elevation). OK applies it in one undo step: the structure moves with its stories, deleted stories take
// their structure with them (model/stories.ts).

import { useState } from "react";

import { t } from "../../i18n";
import { applyStoryTable, rowsFromHeights, storyLevels, storyObjects, type StoryEdit } from "../../model/stories";
import { round } from "../../model/grid";
import { useStore } from "../../state/store";
import { useUnits } from "../../units/format";
import { Dialog } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

interface Row {
  id: string;
  height: string;
  /** The story this row was (absent: added in the dialog). */
  from?: string;
}

/** A name not used yet: Story1, Story2, … */
function newName(rows: Row[], base: string): string {
  const used = new Set([base, ...rows.map((r) => r.id.trim())]);
  let k = rows.length + 1;
  while (used.has(`Story${k}`)) k++;
  return `Story${k}`;
}

export function StoryDataDialog() {
  const model = useStore((s) => s.model);
  const U = useUnits();
  const levels = storyLevels(model);
  const [baseName, setBaseName] = useState(levels[0]?.id ?? "Base");
  const [baseZ, setBaseZ] = useState(U.show(levels[0]?.z ?? 0, "distance"));
  // Rows bottom up (shown top down).
  const [rows, setRows] = useState<Row[]>(levels.slice(1).map((s, k) => ({ id: s.id, height: U.show(round(s.z - levels[k].z), "distance"), from: s.id })));
  const [error, setError] = useState("");
  const setRow = (i: number, patch: Partial<Row>) => setRows(rows.map((r, k) => (k === i ? { ...r, ...patch } : r)));
  /** Deleting a story: the story above grows by its height, so the levels above stay. */
  const remove = (i: number) => {
    const next = rows.map((r) => ({ ...r }));
    const h = U.read(next[i].height, "distance");
    if (i + 1 < next.length && h !== null) {
      const above = U.read(next[i + 1].height, "distance");
      if (above !== null) next[i + 1].height = U.show(round(above + h), "distance");
    }
    next.splice(i, 1);
    setRows(next);
  };
  /** A new story above row i (-1: just above the base), as high as the row it sits on (3 m on the base). */
  const insert = (i: number) => {
    const h = i >= 0 ? rows[i].height : U.show(3, "distance");
    const next = [...rows];
    next.splice(i + 1, 0, { id: newName(rows, baseName), height: h });
    setRows(next);
  };
  const kept = new Set(rows.map((r) => r.from).filter(Boolean));
  const deleted = levels.slice(1).filter((s) => !kept.has(s.id));
  const lost = deleted.map((s) => ({ id: s.id, ...storyObjects(model, s.id) })).filter((d) => d.frames.length + d.areas.length + d.links.length + d.nodes.length);

  const ok = () => {
    const z = U.read(baseZ, "distance");
    const heights = rows.map((r) => U.read(r.height, "distance"));
    if (z === null || heights.some((h) => h === null || !(h > 0))) return setError(t("sd.numbers"));
    const table = rowsFromHeights(
      { id: baseName.trim(), z, from: levels[0]?.id },
      rows.map((r, k) => ({ id: r.id.trim(), height: heights[k]!, from: r.from })),
    );
    let edit: StoryEdit | null = null;
    const done = useStore.getState().edit((m) => {
      edit = applyStoryTable(m, table);
      return edit.model;
    });
    if (!done) return;
    // A plan view at a story's level follows the story.
    const view = useStore.getState().view;
    if (view.kind === "plan") {
      const story = levels.find((l) => Math.abs(l.z - view.at) <= 1e-6);
      const row = story && table.find((r) => r.from === story.id);
      if (row) useStore.getState().setView({ ...view, at: row.z });
    }
    const e = edit as StoryEdit | null;
    if (e) {
      const parts: string[] = [];
      const n = e.deleted.frames.length + e.deleted.links.length + e.deleted.areas.length;
      if (n || e.deleted.nodes.length) parts.push(t("sd.deleted", { objects: n, joints: e.deleted.nodes.length }));
      if (e.meshesCleared.length) parts.push(t("sd.meshes", { list: e.meshesCleared.join(", ") }));
      if (e.windRowsRemoved) parts.push(t("sd.wind", { n: e.windRowsRemoved }));
      if (parts.length) useStore.getState().notify(parts.join(" "));
    }
    close();
  };

  const dist = U.label("distance");
  const shown = rows.map((r, i) => ({ r, i })).reverse();
  let level = U.read(baseZ, "distance");
  const elevation = rows.map((r) => {
    const h = U.read(r.height, "distance");
    level = level !== null && h !== null ? round(level + h) : null;
    return level;
  });
  return (
    <Dialog
      title={t("sd.title")}
      wide
      buttons={
        <>
          <button className="primary" onClick={ok}>
            {t("dlg.ok")}
          </button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <p className="hint">{t("sd.hint")}</p>
      <table className="data combo-rows">
        <thead>
          <tr>
            <th>{t("sd.story")}</th>
            <th>
              {t("sd.height")} ({dist})
            </th>
            <th>
              {t("sd.elevation")} ({dist})
            </th>
            <th />
          </tr>
        </thead>
        <tbody>
          {shown.map(({ r, i }) => (
            <tr key={i}>
              <td>
                <input aria-label={t("sd.nameOf", { n: i + 1 })} value={r.id} onChange={(e) => setRow(i, { id: e.target.value })} style={{ width: 110 }} />
              </td>
              <td>
                <input aria-label={t("sd.heightOf", { n: i + 1 })} value={r.height} onChange={(e) => setRow(i, { height: e.target.value })} style={{ width: 80 }} />
              </td>
              <td className="num">{elevation[i] === null ? "" : U.fmt(elevation[i]!, "distance", 0, 7)}</td>
              <td>
                <button aria-label={t("sd.insertAbove", { id: r.id })} title={t("sd.insertAbove", { id: r.id })} onClick={() => insert(i)}>
                  +
                </button>
                <button aria-label={t("sd.delete", { id: r.id })} title={t("sd.delete", { id: r.id })} onClick={() => remove(i)}>
                  ✕
                </button>
              </td>
            </tr>
          ))}
          <tr>
            <td>
              <input aria-label={t("sd.baseName")} value={baseName} onChange={(e) => setBaseName(e.target.value)} style={{ width: 110 }} />
            </td>
            <td />
            <td>
              <input aria-label={t("sd.baseElevation")} value={baseZ} onChange={(e) => setBaseZ(e.target.value)} style={{ width: 80 }} />
            </td>
            <td>
              <button aria-label={t("sd.insertAbove", { id: baseName })} title={t("sd.insertAbove", { id: baseName })} onClick={() => insert(-1)}>
                +
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      {lost.map((d) => (
        <p key={d.id} className="error">
          {t("sd.willDelete", { id: d.id, objects: d.frames.length + d.areas.length + d.links.length, joints: d.nodes.length })}
        </p>
      ))}
      {error && <p className="error">{error}</p>}
    </Dialog>
  );
}
