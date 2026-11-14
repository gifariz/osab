// Model Explorer: a tree of definitions (materials, sections, load patterns), object types (joints,
// columns, beams… with their counts; not each object), stories and results, drawn with tree lines.
// Double-click opens a definition or selects a group; right-click gives actions; the eye hides/shows.

import { useMemo, useState, type MouseEvent, type ReactElement } from "react";

import { t } from "../i18n";

const tr = t;
import { areasUsing, assignArea, deleteAreaSection } from "../model/areaOps";
import { objectTypes, type AreaType, type FrameType } from "../model/objectTypes";
import { assignFrame, copyDefinition, deleteLoadPattern, deleteMaterial, deleteSection, membersUsing } from "../model/ops";
import { storyLevels } from "../model/stories";
import type { Id, Model, Results } from "../model/types";
import { resultKey, resultLabel } from "../scene/buildResultsScene";
import { SELECTION_KEYS, fullSelection, selectionCount, useStore, type DialogName, type DisplayMode, type Selection } from "../state/store";
import { usePrefs } from "../state/prefs";
import { ContextMenu, type MenuItem } from "./ContextMenu";
import { springUsers, type SpringKind } from "../model/springs";

interface TreeNode {
  key: string;
  label: string;
  /** How many items or objects the row stands for, shown after the label: "Columns (8)". */
  count?: number;
  children?: TreeNode[];
  /** The joints, members and links this row stands for (select / hide / show / eye). */
  objects?: Partial<Selection>;
  /** Double-click action. */
  open?: () => void;
  /** Right-click menu. */
  menu?: () => (MenuItem | "-")[];
}

const st = () => useStore.getState();
const none: Selection = fullSelection({});

function objectMenu(objects: Partial<Selection>): (MenuItem | "-")[] {
  return [
    { label: t("ex.select"), run: () => st().select(picks(objects), "add") },
    { label: t("ex.deselect"), run: () => st().deselect(objects) },
    "-",
    { label: t("ex.show"), run: () => st().show(objects) },
    { label: t("ex.hide"), run: () => st().hide(objects) },
    { label: t("ex.showOnly"), run: () => st().showOnly(objects) },
  ];
}

/** Select, Deselect, Show, Hide and Show Only for the objects using a material or section. */
function usingMenu(objects: Partial<Selection>): (MenuItem | "-")[] {
  return [
    { label: t("ex.usingSelect"), run: () => st().select(picks(objects), "add") },
    { label: t("ex.usingDeselect"), run: () => st().deselect(objects) },
    { label: t("ex.usingShow"), run: () => st().show(objects) },
    { label: t("ex.usingHide"), run: () => st().hide(objects) },
    { label: t("ex.usingShowOnly"), run: () => st().showOnly(objects) },
  ];
}

const picks = (o: Partial<Selection>) => [
  ...(o.nodes ?? []).map((id) => ({ kind: "node" as const, id })),
  ...(o.frames ?? []).map((id) => ({ kind: "frame" as const, id })),
  ...(o.links ?? []).map((id) => ({ kind: "link" as const, id })),
  ...(o.areas ?? []).map((id) => ({ kind: "area" as const, id })),
];

function definitionMenu(kind: "material" | "section", id: Id, dialog: DialogName): (MenuItem | "-")[] {
  const field = kind === "material" ? "material" : "section";
  const selected = st().selection.frames;
  return [
    { label: t("ex.edit"), run: () => st().openDialog(dialog, id) },
    { label: t("ex.copy"), run: () => void st().edit((m) => copyDefinition(m, kind, id).model) },
    {
      label: t("ex.assignSelected", { n: selected.length }),
      enabled: selected.length > 0,
      run: () => st().edit((m) => assignFrame(m, selected, { [field]: id })),
    },
    "-",
    ...usingMenu({ frames: membersUsing(st().model, kind, id), areas: kind === "material" ? areasUsing(st().model, "material", id) : [] }),
    "-",
    { label: t("ex.delete"), run: () => st().edit((m) => (kind === "material" ? deleteMaterial(m, id) : deleteSection(m, id))) },
  ];
}

function areaSectionMenu(id: Id): (MenuItem | "-")[] {
  const selected = st().selection.areas;
  return [
    { label: t("ex.edit"), run: () => st().openDialog("areaSections", id) },
    { label: t("ex.copy"), run: () => void st().edit((m) => copyDefinition(m, "areaSection", id).model) },
    {
      label: t("ex.assignSelectedAreas", { n: selected.length }),
      enabled: selected.length > 0,
      run: () => st().edit((m) => assignArea(m, selected, { section: id })),
    },
    "-",
    ...usingMenu({ areas: areasUsing(st().model, "areaSection", id) }),
    "-",
    { label: t("ex.delete"), run: () => st().edit((m) => deleteAreaSection(m, id)) },
  ];
}

/** Line and area spring properties (when there are any): double-click edits, right-click selects their users. */
function springGroups(m: Model): TreeNode[] {
  const group = (kind: SpringKind, list: { id: Id }[] | undefined): TreeNode[] => {
    if (!list?.length) return [];
    const dialog = kind === "line" ? "lineSpringProps" : "areaSpringProps";
    const children: TreeNode[] = list.map((p) => ({
      key: `${kind}spr:${p.id}`,
      label: p.id,
      open: () => st().openDialog(dialog, p.id),
      menu: () => [
        { label: t("ex.edit"), run: () => st().openDialog(dialog, p.id) },
        "-",
        ...usingMenu(kind === "line" ? { frames: springUsers(st().model, "line", p.id) } : { areas: springUsers(st().model, "area", p.id) }),
      ],
    }));
    return [{ key: `${kind}sprs`, label: t(kind === "line" ? "ex.lineSprings" : "ex.areaSprings"), count: children.length, children, open: () => st().openDialog(dialog) }];
  };
  return [...group("line", m.line_spring_props), ...group("area", m.area_spring_props)];
}

function showResults(caseId: Id, mode: DisplayMode) {
  st().setDisplay({ caseId, mode });
  st().setLeftTab("properties");
}

function buildTree(m: Model, results: Results | null): TreeNode[] {
  const materials: TreeNode[] = m.materials.map((x) => ({
    key: `mat:${x.id}`,
    label: x.id,
    open: () => st().openDialog("materials", x.id),
    menu: () => definitionMenu("material", x.id, "materials"),
  }));
  const sections: TreeNode[] = m.sections.map((x) => ({
    key: `sec:${x.id}`,
    label: x.id,
    open: () => st().openDialog("sections", x.id),
    menu: () => definitionMenu("section", x.id, "sections"),
  }));
  const areaSections: TreeNode[] = m.area_sections.map((x) => ({
    key: `asec:${x.id}`,
    label: x.id,
    open: () => st().openDialog("areaSections", x.id),
    menu: () => areaSectionMenu(x.id),
  }));
  const patterns: TreeNode[] = m.load_patterns.map((p) => ({
    key: `pattern:${p.id}`,
    label: p.id,
    open: () => st().setDisplay({ loadsPattern: p.id, mode: "model" }),
    menu: () => [
      st().display.loadsPattern === p.id
        ? { label: t("ex.hideLoads"), run: () => st().setDisplay({ loadsPattern: null }) }
        : { label: t("ex.showLoads"), run: () => st().setDisplay({ loadsPattern: p.id, mode: "model" }) },
      { label: t("ex.edit"), run: () => st().openDialog("patterns", p.id) },
      { label: t("ex.copyPattern"), run: () => void st().edit((mm) => copyDefinition(mm, "pattern", p.id).model) },
      "-",
      { label: t("ex.deletePattern"), run: () => st().edit((mm) => deleteLoadPattern(mm, p.id)) },
    ],
  }));

  // An object type: its objects are selected, hidden or shown as a group (not listed one by one).
  const objectNode = (key: string, label: string, objects: Partial<Selection>, children?: TreeNode[]): TreeNode => ({
    key,
    label,
    count: selectionCount(fullSelection(objects)),
    children,
    objects,
    open: () => st().select(picks(objects), "add"),
    menu: () => objectMenu(objects),
  });

  // Object types (worked out from the geometry, the section and the opening flag).
  const types = objectTypes(m);
  const frameIds: Record<FrameType, Id[]> = { column: [], beam: [], brace: [], null: [] };
  for (const e of m.frames) frameIds[types.frames.get(e.id) ?? "brace"].push(e.id);
  const areaIds: Record<AreaType, Id[]> = { floor: [], wall: [], opening: [], null: [] };
  for (const a of m.areas) areaIds[types.areas.get(a.id) ?? "wall"].push(a.id);
  const frameGroups = (
    [
      ["column", "ex.columns"],
      ["beam", "ex.beams"],
      ["brace", "ex.braces"],
      ["null", "ex.nullFrames"],
    ] as const
  )
    .filter(([k]) => frameIds[k].length)
    .map(([k, label]) => objectNode(`frames:${k}`, t(label), { frames: frameIds[k] }));
  const areaGroups = (
    [
      ["floor", "ex.floors"],
      ["wall", "ex.walls"],
      ["opening", "ex.openings"],
      ["null", "ex.nullAreas"],
    ] as const
  )
    .filter(([k]) => areaIds[k].length)
    .map(([k, label]) => objectNode(`areas:${k}`, t(label), { areas: areaIds[k] }));

  // Stories, top down; they open Define > Story Data.
  const editGrid = () => st().openDialog("storyData");
  const storyMenu = () => [{ label: t("ex.editStories"), run: editGrid }];
  const stories: TreeNode[] = [...storyLevels(m)]
    .reverse()
    .map((x) => ({ key: `story:${x.id}`, label: x.id, open: editGrid, menu: storyMenu }));

  const tree: TreeNode[] = [
    {
      key: "props",
      label: t("ex.properties"),
      children: [
        { key: "mats", label: t("ex.materials"), count: materials.length, children: materials, open: () => st().openDialog("materials") },
        { key: "secs", label: t("ex.frameSections"), count: sections.length, children: sections, open: () => st().openDialog("sections") },
        { key: "asecs", label: t("ex.areaSections"), count: areaSections.length, children: areaSections, open: () => st().openDialog("areaSections") },
        ...springGroups(m),
      ],
    },
    { key: "patterns", label: t("ex.loadPatterns"), count: patterns.length, children: patterns, open: () => st().openDialog("patterns") },
    {
      key: "objects",
      label: t("ex.objects"),
      children: [
        objectNode("joints", t("ex.joints"), { nodes: m.nodes.map((n) => n.id) }),
        objectNode("frames", t("ex.frames"), { frames: m.frames.map((e) => e.id) }, frameGroups),
        objectNode("areas", t("ex.areas"), { areas: m.areas.map((a) => a.id) }, areaGroups),
        objectNode("links", t("ex.links"), { links: m.links.map((l) => l.id) }),
        objectNode("supports", t("ex.supports"), { nodes: m.supports.map((s) => s.node) }),
        objectNode("springs", t("ex.springs"), { nodes: m.springs.map((s) => s.node) }),
      ],
    },
    { key: "stories", label: t("ex.stories"), count: stories.length, children: stories, open: editGrid, menu: storyMenu },
  ];
  if (results) {
    tree.push({
      key: "results",
      label: t("ex.results"),
      children: results.cases.map((c) => ({
        key: `res:${resultKey(c)}`,
        label: resultLabel(c),
        open: () => showResults(resultKey(c), "deformed"),
        menu: () => [
          { label: t("ex.deformed"), run: () => showResults(resultKey(c), "deformed") },
          { label: t("ex.memberForces"), run: () => showResults(resultKey(c), "forces") },
          ...(c.shells?.length ? [{ label: t("ex.shellForces"), run: () => showResults(resultKey(c), "shells") }] : []),
          { label: t("ex.reactions"), run: () => showResults(resultKey(c), "reactions") },
          "-",
          { label: t("ex.tables"), run: () => st().openDialog("tables") },
        ],
      })),
    });
  }
  return tree;
}

export function Explorer() {
  const model = useStore((s) => s.model);
  const results = useStore((s) => s.results);
  const selection = useStore((s) => s.selection);
  const hidden = useStore((s) => s.hidden);
  const fileName = useStore((s) => s.fileName);
  const [open, setOpen] = useState<Set<string>>(() => new Set()); // every row starts collapsed
  const [menu, setMenu] = useState<{ x: number; y: number; items: (MenuItem | "-")[] } | null>(null);
  const lang = usePrefs((p) => p.lang);
  const tree = useMemo(() => buildTree(model, results), [model, results, lang]);

  type Sets = Record<keyof Selection, Set<Id>>;
  const sets = (o: Selection): Sets => ({ nodes: new Set(o.nodes), frames: new Set(o.frames), links: new Set(o.links), areas: new Set(o.areas) });
  const sel = sets(selection);
  const hid = sets(hidden);
  const kinds = SELECTION_KEYS;
  const all = (o: Selection, set: Sets) => selectionCount(o) > 0 && kinds.every((k) => o[k].every((id) => set[k].has(id)));
  const some = (o: Selection, set: Sets) => kinds.some((k) => o[k].some((id) => set[k].has(id)));

  const toggle = (key: string) => {
    const next = new Set(open);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setOpen(next);
  };

  const rows: ReactElement[] = [];
  // `more[k]`: whether the ancestor at depth k has siblings below it (its tree line goes on down).
  const walk = (nodes: TreeNode[], more: boolean[]) => {
    const depth = more.length;
    nodes.forEach((t, index) => {
      const last = index === nodes.length - 1;
      const o = t.objects ? fullSelection(t.objects) : none;
      const isHidden = all(o, hid);
      const isSelected = all(o, sel);
      const expanded = open.has(t.key);
      const onContext = (e: MouseEvent) => {
        e.preventDefault();
        if (t.menu) setMenu({ x: e.clientX, y: e.clientY, items: t.menu() });
      };
      rows.push(
        <div
          key={t.key}
          role="treeitem"
          aria-label={t.label}
          aria-expanded={t.children ? expanded : undefined}
          aria-selected={isSelected}
          className={`tree-row${isSelected ? " selected" : ""}${isHidden ? " hidden-obj" : ""}`}
          onDoubleClick={() => (t.open ? t.open() : t.children && toggle(t.key))}
          onContextMenu={onContext}
          title={t.open || t.menu ? tr("ex.rowTitle") : undefined}
        >
          {/* One column per ancestor (the top rows are not joined): its line goes on below it while it has
              more children, so a child's line starts under its parent's text. */}
          {more.map((goesOn, k) => (
            <span key={k} className={`tree-guide${k > 0 && goesOn ? " line" : ""}`} />
          ))}
          {/* The row's own column: its branch line (tee or elbow) and, for a row with children, the
              +/- box on it; the label starts where the line ends. */}
          <span className={`tree-guide${depth > 0 ? (last ? " elbow" : " tee") : ""}`}>
            {t.children && t.children.length ? (
              <span className="tree-box" onClick={() => toggle(t.key)}>
                {expanded ? "−" : "+"}
              </span>
            ) : null}
          </span>
          <span className="tree-label">
            {t.label}
            {t.count !== undefined && <span className="tree-count"> ({t.count})</span>}
          </span>
          {selectionCount(o) > 0 && (
            <button
              className={`eye${isHidden ? " off" : some(o, hid) ? " partial" : ""}`}
              aria-label={tr("ex.eyeLabel", { action: tr(isHidden ? "ex.eyeShow" : "ex.eyeHide"), label: t.label })}
              title={tr(isHidden ? "ex.eyeShow" : "ex.eyeHide")}
              onClick={() => (isHidden ? st().show(o) : st().hide(o))}
            >
              {isHidden ? "◌" : "●"}
            </button>
          )}
        </div>,
      );
      if (t.children && expanded) walk(t.children, [...more, !last]);
    });
  };
  walk(tree, []);

  const hiddenCount = selectionCount(hidden);
  return (
    <div className="explorer" role="tree" aria-label={tr("ex.tree")}>
      <div className="explorer-head">
        <b title={fileName}>{fileName}</b>
        {hiddenCount > 0 && (
          <button onClick={() => st().showAll()} title={tr("ex.showAllTitle")}>
            {tr("ex.showAllHidden", { n: hiddenCount })}
          </button>
        )}
      </div>
      {rows}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </div>
  );
}
