// Colours of the drawing data for the light and dark themes. The scene builders read COLORS and
// RESULT_COLORS; `setSceneTheme` switches them (the views then rebuild their scenes).

import type { Theme } from "../state/prefs";

const MODEL_DARK = {
  background: "#0d1117",
  hover: "#5ce1e6",
  grid: "#33404f",
  node: "#e6edf3",
  selected: "#ffd84d",
  highlight: "#ff5c5c",
  dimmed: "#56626f",
  support: "#7bed9f",
  load: "#ff9f43",
  link: "#dfe6e9",
  spring: "#4dd0e1",
  /** Dots near released frame ends (the usual symbol). */
  release: "#7bed9f",
  /** End length offsets (thicker line over the offset part). */
  offset: "#ffd166",
  /** Rigid diaphragms: their joints, extent lines, centre and name. */
  diaphragm: "#c084fc",
  /** Joint masses (arrows with their values). */
  mass: "#f78fb3",
  /** Lines of the analysis mesh (View > Show Analysis Mesh). */
  mesh: "#9aa7b5",
  /** The span symbol of one-way floors in plan views. */
  span: "#e6edf3",
  /** Local axes 1, 2, 3 (red, white, cyan). */
  axes: ["#ff4d4d", "#f5f5f5", "#33d6e0"],
  sections: ["#5cc8ff", "#c56cf0", "#eccc68", "#70a1ff", "#ff6b81", "#a4b0be", "#ffa8d9", "#2ed573"],
  areas: ["#4f8fbf", "#9a7fd1", "#c9a64a", "#4fb39a", "#d07a8f", "#8796a8", "#c58ad8", "#6fbf5f"],
};

const MODEL_LIGHT: typeof MODEL_DARK = {
  background: "#ffffff",
  hover: "#0a7bd8",
  grid: "#ccd3db",
  node: "#1d2733",
  selected: "#e08e00",
  highlight: "#d32f2f",
  dimmed: "#b4bcc6",
  support: "#1f9d55",
  load: "#d35400",
  link: "#5d6b7a",
  spring: "#00838f",
  release: "#1f9d55",
  offset: "#b7791f",
  diaphragm: "#7e22ce",
  mass: "#c2185b",
  mesh: "#5f6b78",
  span: "#1d2733",
  axes: ["#e53935", "#4a4a4a", "#00838f"],
  sections: ["#1e88e5", "#8e24aa", "#b28704", "#3949ab", "#d81b60", "#546e7a", "#c2185b", "#2e7d32"],
  areas: ["#64a6d9", "#a58fd8", "#d9b44f", "#5fbfa5", "#de8aa0", "#97a6b8", "#cf98df", "#7dc96c"],
};

const RESULTS_DARK = {
  deformed: "#ff9f43",
  positive: "#f5c542",
  negative: "#ff5c5c",
  outline: "#e6edf3",
  edge: "#0d1117",
  reaction: "#7bed9f",
  link: "#dfe6e9",
  /** Centres of mass and of rigidity in plan views. */
  centreMass: "#c084fc",
  centreRigidity: "#4dd0e1",
};

const RESULTS_LIGHT: typeof RESULTS_DARK = {
  deformed: "#e65100",
  positive: "#f2b705",
  negative: "#e53935",
  outline: "#1d2733",
  edge: "#1d2733",
  reaction: "#1f9d55",
  link: "#5d6b7a",
  centreMass: "#7e22ce",
  centreRigidity: "#00838f",
};

/** Model colours of the current theme (dark until `setSceneTheme` is called). */
export const COLORS = structuredClone(MODEL_DARK);
/** Result colours of the current theme. */
export const RESULT_COLORS = structuredClone(RESULTS_DARK);

export function setSceneTheme(theme: Theme) {
  Object.assign(COLORS, structuredClone(theme === "light" ? MODEL_LIGHT : MODEL_DARK));
  Object.assign(RESULT_COLORS, structuredClone(theme === "light" ? RESULTS_LIGHT : RESULTS_DARK));
}
