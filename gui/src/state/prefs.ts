// App preferences, kept on this computer (browser storage of the app window), not in model files.

import { create } from "zustand";

import { setLang, type Lang } from "../i18n";

export type ThemeChoice = "system" | "light" | "dark";
export type Theme = "light" | "dark";

export interface Prefs {
  /** Drawing snap step in mm: free points in plan and elevation views are rounded to it. */
  snapMm: number;
  /** UI language. */
  lang: Lang;
  /** Colour theme of the window and the views ("system" follows Windows; light by default). */
  theme: ThemeChoice;
  /** Width of the left panel (Model / Properties), px. */
  panelWidth: number;
  /** The left panel: the model explorer above the properties ("stacked", the default) or one of them at a time ("tabs"). */
  panelLayout: PanelLayout;
  /** Stacked: the explorer's share of the panel's height (the rest is the properties'). */
  explorerShare: number;
}

export type PanelLayout = "stacked" | "tabs";

export const DEFAULT_PREFS: Prefs = { snapMm: 0.1, lang: "en", theme: "light", panelWidth: 250, panelLayout: "stacked", explorerShare: 0.45 };
/** Limits of the left panel's width, px. */
export const PANEL_WIDTH = { min: 180, max: 600 };
export const clampPanelWidth = (w: number) => Math.round(Math.min(PANEL_WIDTH.max, Math.max(PANEL_WIDTH.min, w)));
/** Limits of the explorer's share of the stacked panel. */
export const EXPLORER_SHARE = { min: 0.1, max: 0.9 };
export const clampExplorerShare = (f: number) => Math.round(Math.min(EXPLORER_SHARE.max, Math.max(EXPLORER_SHARE.min, f)) * 1000) / 1000;
const KEY = "osab.prefs";

function load(): Prefs {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    const snapMm = Number(raw.snapMm);
    const panelWidth = Number(raw.panelWidth);
    const explorerShare = Number(raw.explorerShare);
    return {
      snapMm: snapMm > 0 ? snapMm : DEFAULT_PREFS.snapMm,
      lang: raw.lang === "id" ? "id" : "en",
      theme: raw.theme === "system" || raw.theme === "dark" ? raw.theme : DEFAULT_PREFS.theme,
      panelWidth: panelWidth > 0 ? clampPanelWidth(panelWidth) : DEFAULT_PREFS.panelWidth,
      panelLayout: raw.panelLayout === "tabs" ? "tabs" : "stacked",
      explorerShare: explorerShare > 0 ? clampExplorerShare(explorerShare) : DEFAULT_PREFS.explorerShare,
    };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

interface PrefsState extends Prefs {
  /** Whether Windows (or the browser) is in dark mode now. */
  systemDark: boolean;
  set(p: Partial<Prefs>): void;
}

const darkQuery = typeof window !== "undefined" && window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;

/** The theme in use: the choice, or the system's when the choice is "system". */
export const themeOf = (p: Prefs & { systemDark: boolean }): Theme => (p.theme === "system" ? (p.systemDark ? "dark" : "light") : p.theme);

function applyTheme(theme: Theme) {
  if (typeof document !== "undefined") document.documentElement.dataset.theme = theme;
}

const initial = load();
setLang(initial.lang);

export const usePrefs = create<PrefsState>()((set, get) => ({
  ...initial,
  systemDark: darkQuery?.matches ?? false,
  set(p) {
    if (p.lang) setLang(p.lang);
    set(p);
    applyTheme(themeOf(get()));
    try {
      const { snapMm, lang, theme, panelWidth, panelLayout, explorerShare } = get();
      localStorage.setItem(KEY, JSON.stringify({ snapMm, lang, theme, panelWidth, panelLayout, explorerShare }));
    } catch {
      // Storage may be unavailable (private window); the setting then lasts for this session only.
    }
  },
}));

applyTheme(themeOf(usePrefs.getState()));
darkQuery?.addEventListener?.("change", (e) => {
  usePrefs.setState({ systemDark: e.matches });
  applyTheme(themeOf(usePrefs.getState()));
});

/** The theme in use, for components. */
export const useTheme = () => usePrefs((p) => themeOf(p));

/** Snap step in model length units (the app works in metres). */
export const snapStep = (p: Prefs) => p.snapMm / 1000;
