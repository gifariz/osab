// Every menu/toolbar command in one place, with its shortcut and when it is available.

import { deleteSelection, openFile, runAnalysis, saveFile, selectAllInView, storeSelectedMeshes } from "../actions";
import { removeStoredMeshes } from "../model/storedMesh";
import { t, type Key } from "../i18n";
import { gridOf } from "../model/ops";
import { isDemo } from "../demo";
import { selectionCount, useStore, type DialogName, type DisplayMode, type ViewKind } from "../state/store";
import { allOn, setAll, type ViewOptions } from "../scene/viewOptions";
import { zoomToFit } from "./ViewportPanel";

export interface Command {
  shortcut?: string;
  run(): void;
  enabled?(): boolean;
  checked?(): boolean;
}

const s = () => useStore.getState();
const dialog = (d: DialogName) => () => s().openDialog(d);
const hasSelection = () => selectionCount(s().selection) > 0;
const hasNodes = () => s().selection.nodes.length > 0;
const hasFrames = () => s().selection.frames.length > 0;
const hasAreas = () => s().selection.areas.length > 0;
const hasResults = () => s().results !== null;
const opts = () => s().display.options;
const setOpts = (options: ViewOptions) => s().setDisplay({ options });
const extruded = () => opts().extrudeFrames && opts().extrudeAreas;
const display = (mode: DisplayMode) => () => {
  if (mode !== "model" && !s().results) {
    s().notify(t("msg.runFirst"));
    return;
  }
  s().setDisplay({ mode });
  if (mode !== "forces") s().setMemberPanel(null);
};

/**
 * Undeformed Shape (F4): back to the model with the default display: no loads, labels, extrusion,
 * local axes or analysis mesh. The view, zoom, selection, hidden objects and results stay.
 */
function defaultView() {
  const o = setAll(setAll(opts(), "labels", false), "localAxes", false);
  s().setDisplay({ mode: "model", loadsPattern: null, options: { ...o, extrudeFrames: false, extrudeAreas: false, mesh: false } });
  s().setMemberPanel(null);
}

export function setViewKind(kind: ViewKind) {
  const st = s();
  const g = gridOf(st.model);
  const keep = st.view.kind === kind;
  const at =
    kind === "plan"
      ? keep ? st.view.at : g.stories[g.stories.length - 1]?.z ?? 0
      : kind === "elev-xz"
        ? keep ? st.view.at : g.y[0]?.coord ?? 0
        : kind === "elev-yz"
          ? keep ? st.view.at : g.x[0]?.coord ?? 0
          : 0;
  st.setView({ kind, at });
}

/** Moves a plan or elevation view to the next story / grid line (dir = +1 or -1). */
export function stepView(dir: 1 | -1) {
  const st = s();
  const g = gridOf(st.model);
  const list =
    st.view.kind === "plan" ? g.stories.map((x) => x.z) : st.view.kind === "elev-xz" ? g.y.map((x) => x.coord) : st.view.kind === "elev-yz" ? g.x.map((x) => x.coord) : [];
  if (!list.length) return;
  let i = list.findIndex((v) => Math.abs(v - st.view.at) < 1e-6);
  i = Math.max(0, Math.min(list.length - 1, (i < 0 ? 0 : i) + dir));
  st.setView({ kind: st.view.kind, at: list[i] });
}

export const commands = {
  newModel: { shortcut: "Ctrl+N", run: dialog("new") },
  open: { shortcut: "Ctrl+O", run: () => void openFile() },
  openExample: { run: dialog("examples") },
  save: { shortcut: "Ctrl+S", run: () => void saveFile(false) },
  saveAs: { shortcut: "Ctrl+Shift+S", run: () => void saveFile(true) },

  undo: { shortcut: "Ctrl+Z", run: () => s().undo(), enabled: () => s().past.length > 0 },
  redo: { shortcut: "Ctrl+Y", run: () => s().redo(), enabled: () => s().future.length > 0 },
  del: { shortcut: "Del", run: deleteSelection, enabled: hasSelection },
  move: { shortcut: "Ctrl+M", run: dialog("move"), enabled: hasSelection },
  replicate: { shortcut: "Ctrl+R", run: dialog("replicate"), enabled: hasSelection },
  selectAll: { shortcut: "Ctrl+A", run: selectAllInView },
  clearSelection: { shortcut: "Esc", run: () => s().clearSelection(), enabled: hasSelection },
  divideAreas: { run: dialog("divideAreas"), enabled: hasAreas },
  storeMesh: { run: () => void storeSelectedMeshes(), enabled: hasAreas },
  removeStoredMesh: {
    run: () => s().edit((m) => removeStoredMeshes(m, s().selection.areas)),
    enabled: () => s().selection.areas.some((id) => s().model.areas.find((a) => a.id === id)?.user_mesh),
  },
  editGrid: { run: dialog("grid") },
  jointCoords: { run: dialog("jointCoords") },

  view3d: { run: () => setViewKind("3d"), checked: () => s().view.kind === "3d" },
  // The 3D view in parallel projection (no perspective), kept with the model's view options.
  parallel: { run: () => setOpts({ ...opts(), parallel: !opts().parallel }), checked: () => opts().parallel },
  viewPlan: { run: () => setViewKind("plan"), checked: () => s().view.kind === "plan" },
  viewXZ: { run: () => setViewKind("elev-xz"), checked: () => s().view.kind === "elev-xz" },
  viewYZ: { run: () => setViewKind("elev-yz"), checked: () => s().view.kind === "elev-yz" },
  viewUp: { shortcut: "PgUp", run: () => stepView(1), enabled: () => s().view.kind !== "3d" },
  viewDown: { shortcut: "PgDn", run: () => stepView(-1), enabled: () => s().view.kind !== "3d" },
  fit: { shortcut: "Home", run: zoomToFit },
  hideSelection: { run: () => s().hide(s().selection), enabled: hasSelection },
  showOnlySelection: { run: () => s().showOnly(s().selection), enabled: hasSelection },
  showAll: { run: () => s().showAll(), enabled: () => selectionCount(s().hidden) > 0 },
  explorer: { run: () => s().setLeftTab("model"), checked: () => s().leftTab === "model" },
  // Browsers keep Ctrl+W (close the tab): the web demo opens it from the menu and the toolbar.
  displayOptions: { shortcut: isDemo ? undefined : "Ctrl+W", run: dialog("displayOptions") },
  showGrid: {
    shortcut: "Ctrl+D",
    run: () => setOpts({ ...opts(), grid: !opts().grid }),
    checked: () => opts().grid,
  },
  labels: {
    run: () => setOpts(setAll(opts(), "labels", !allOn(opts(), "labels"))),
    checked: () => allOn(opts(), "labels"),
  },
  extrude: {
    run: () => setOpts({ ...opts(), extrudeFrames: !extruded(), extrudeAreas: !extruded() }),
    checked: extruded,
  },
  localAxes: {
    run: () => setOpts(setAll(opts(), "localAxes", !allOn(opts(), "localAxes"))),
    checked: () => allOn(opts(), "localAxes"),
  },
  showMesh: {
    run: () => setOpts({ ...opts(), mesh: !opts().mesh }),
    checked: () => opts().mesh,
  },
  selectByType: { run: dialog("selectType") },
  selectByGroup: { run: dialog("groups") },
  groups: { run: dialog("groups") },
  pierLabels: { run: dialog("pierLabels") },
  spandrelLabels: { run: dialog("spandrelLabels") },
  assignPier: { run: dialog("pierLabels"), enabled: () => hasFrames() || hasAreas() },
  assignSpandrel: { run: dialog("spandrelLabels"), enabled: () => hasFrames() || hasAreas() },

  materials: { run: dialog("materials") },
  sections: { run: dialog("sections") },
  areaSections: { run: dialog("areaSections") },
  patterns: { run: dialog("patterns") },
  cases: { run: dialog("cases") },
  combos: { run: dialog("combos") },

  select: { run: () => s().setTool("select"), checked: () => s().tool === "select" },
  drawJoint: { run: () => s().setTool("joint"), checked: () => s().tool === "joint" },
  drawFrame: { run: () => s().setTool("frame"), checked: () => s().tool === "frame" },
  drawLink: { run: () => s().setTool("link"), checked: () => s().tool === "link" },
  measure: { run: () => s().setTool("measure"), checked: () => s().tool === "measure" },
  drawArea: { run: () => s().setTool("area"), checked: () => s().tool === "area" },
  jointByCoords: { run: dialog("jointCoords") },

  restraints: { run: dialog("restraints"), enabled: hasNodes },
  springs: { run: dialog("springs"), enabled: hasNodes },
  jointLoads: { run: dialog("jointLoads"), enabled: hasNodes },
  frameAssign: { run: dialog("frameAssign"), enabled: hasFrames },
  frameModifiers: { run: dialog("frameModifiers"), enabled: hasFrames },
  frameReleases: { run: dialog("frameReleases"), enabled: hasFrames },
  frameOffsets: { run: dialog("frameOffsets"), enabled: hasFrames },
  frameInsertion: { run: dialog("frameInsertion"), enabled: hasFrames },
  areaInsertion: { run: dialog("areaInsertion"), enabled: hasAreas },
  storyData: { run: dialog("storyData") },
  diaphragms: { run: dialog("diaphragms") },
  massSource: { run: dialog("massSource") },
  pDelta: { run: dialog("pDelta") },
  spectrumFunctions: { run: dialog("spectrumFunctions") },
  lineSpringProps: { run: dialog("lineSpringProps") },
  areaSpringProps: { run: dialog("areaSpringProps") },
  lineSprings: { run: dialog("lineSprings"), enabled: hasFrames },
  areaSprings: { run: dialog("areaSprings"), enabled: hasAreas },
  jointMasses: { run: dialog("jointMasses"), enabled: hasNodes },
  assignDiaphragm: { run: dialog("assignDiaphragm"), enabled: () => hasNodes() || hasAreas() },
  areaModifiers: { run: dialog("areaModifiers"), enabled: hasAreas },
  frameLoads: { run: dialog("frameLoads"), enabled: hasFrames },
  areaAssign: { run: dialog("areaAssign"), enabled: hasAreas },
  areaLoads: { run: dialog("areaLoads"), enabled: hasAreas },
  areaMesh: { run: dialog("areaMesh"), enabled: hasAreas },
  floorMeshing: { run: dialog("floorMeshing"), enabled: () => hasNodes() || hasFrames() },
  specialJoint: { run: dialog("specialJoint"), enabled: hasNodes },

  run: { shortcut: "F5", run: () => void runAnalysis(), enabled: () => !s().running },
  check: { run: () => s().openDialog("check") },

  showModel: { shortcut: "F4", run: defaultView, checked: () => s().display.mode === "model" },
  showDeformed: { shortcut: "F6", run: display("deformed"), enabled: hasResults, checked: () => s().display.mode === "deformed" },
  showForces: { shortcut: "F7", run: display("forces"), enabled: hasResults, checked: () => s().display.mode === "forces" },
  showModes: { run: display("modes"), enabled: () => !!s().results?.modal, checked: () => s().display.mode === "modes" },
  showShellForces: { run: display("shells"), enabled: hasResults, checked: () => s().display.mode === "shells" },
  showReactions: { shortcut: "F8", run: display("reactions"), enabled: hasResults, checked: () => s().display.mode === "reactions" },
  showLoads: { run: dialog("showLoads"), enabled: () => s().model.load_patterns.length > 0 },
  hideLoads: { run: () => s().setDisplay({ loadsPattern: null }), enabled: () => s().display.loadsPattern !== null },
  tables: { shortcut: "Ctrl+T", run: dialog("tables") },

  preferences: { run: dialog("preferences") },
  angleLimits: { run: dialog("angleLimits") },
  analysisOptions: { run: dialog("analysisOptions") },
  runCases: { run: dialog("runCases") },
  meshSettings: { run: dialog("meshSettings") },

  about: { run: dialog("about") },
  welcome: { run: dialog("welcome") },
} satisfies Record<string, Command>;

export type CommandName = keyof typeof commands;

/** Menu text of a command, in the current language. */
export const cmdLabel = (name: CommandName) => t(`cmd.${name}` as Key);

export const menus: { title: Key; items: (CommandName | "-")[] }[] = [
  { title: "menu.file", items: ["newModel", "open", "openExample", "-", "save", "saveAs"] },
  { title: "menu.edit", items: ["undo", "redo", "-", "del", "-", "move", "replicate", "-", "divideAreas", "storeMesh", "removeStoredMesh", "-", "editGrid", "jointCoords"] },
  {
    title: "menu.view",
    items: [
      "view3d",
      "parallel",
      "viewPlan",
      "viewXZ",
      "viewYZ",
      "-",
      "viewUp",
      "viewDown",
      "fit",
      "-",
      "hideSelection",
      "showOnlySelection",
      "showAll",
      "-",
      "displayOptions",
      "showGrid",
      "labels",
      "localAxes",
      "extrude",
      "showMesh",
      "explorer",
    ],
  },
  { title: "menu.define", items: ["storyData", "-", "materials", "sections", "areaSections", "diaphragms", "lineSpringProps", "areaSpringProps", "spectrumFunctions", "patterns", "cases", "combos", "massSource", "pDelta", "-", "groups", "pierLabels", "spandrelLabels"] },
  { title: "menu.draw", items: ["select", "drawJoint", "drawFrame", "drawArea", "drawLink", "-", "jointByCoords", "-", "measure"] },
  { title: "menu.select", items: ["selectByType", "selectByGroup", "-", "selectAll", "clearSelection"] },
  { title: "menu.assign", items: ["restraints", "springs", "jointLoads", "jointMasses", "assignDiaphragm", "specialJoint", "-", "frameAssign", "frameModifiers", "frameReleases", "frameOffsets", "frameInsertion", "lineSprings", "frameLoads", "-", "areaAssign", "areaModifiers", "areaInsertion", "areaSprings", "areaLoads", "areaMesh", "-", "floorMeshing", "-", "assignPier", "assignSpandrel"] },
  { title: "menu.analyze", items: ["analysisOptions", "meshSettings", "runCases", "-", "run", "check"] },
  { title: "menu.display", items: ["showLoads", "hideLoads", "-", "showModel", "showDeformed", "showForces", "showShellForces", "showReactions", "showModes", "-", "tables"] },
  { title: "menu.options", items: ["preferences", "angleLimits"] },
  { title: "menu.help", items: isDemo ? ["welcome", "about"] : ["about"] },
];

/** Keyboard shortcut -> command (key names as in KeyboardEvent.key, with Ctrl+/Shift+ prefixes). */
export const shortcuts: Record<string, CommandName> = {
  "Ctrl+n": "newModel",
  "Ctrl+o": "open",
  "Ctrl+s": "save",
  "Ctrl+Shift+S": "saveAs",
  "Ctrl+z": "undo",
  "Ctrl+y": "redo",
  "Ctrl+Shift+Z": "redo",
  Delete: "del",
  "Ctrl+a": "selectAll",
  PageUp: "viewUp",
  PageDown: "viewDown",
  Home: "fit",
  F4: "showModel",
  F5: "run",
  F6: "showDeformed",
  F7: "showForces",
  F8: "showReactions",
  "Ctrl+t": "tables",
  "Ctrl+d": "showGrid",
  "Ctrl+m": "move",
  "Ctrl+r": "replicate",
  "Ctrl+w": "displayOptions",
};
