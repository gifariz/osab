import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from "react";

import { t } from "../i18n";
import { useStore } from "../state/store";

/** Open windows, the newest last: Esc closes only the newest (a window opened from inside another). */
const open: object[] = [];

// Help texts: the explanations in windows and in the side panel (`p.hint` without `note`) show only while help is
// on. One switch for all of them, kept between sessions; `note` texts (status, values, warnings) always show.
const HELP_KEY = "osab.help";
const helpListeners = new Set<() => void>();
let helpOn = (() => {
  try {
    return localStorage.getItem(HELP_KEY) === "1";
  } catch {
    return false;
  }
})();
const showHelp = (on: boolean) => {
  helpOn = on;
  document.body.classList.toggle("show-help", on);
  try {
    localStorage.setItem(HELP_KEY, on ? "1" : "0");
  } catch {
    // Not kept: the switch still works for this session.
  }
  helpListeners.forEach((l) => l());
};
if (typeof document !== "undefined") document.body.classList.toggle("show-help", helpOn);
const subscribeHelp = (l: () => void) => {
  helpListeners.add(l);
  return () => void helpListeners.delete(l);
};

/** The ? that shows or hides the explanations (in every window title bar that has some, and in the side panel). */
export function HelpButton() {
  const help = useSyncExternalStore(subscribeHelp, () => helpOn);
  return (
    <button className={help ? "dialog-help on" : "dialog-help"} onClick={() => showHelp(!help)} aria-label={t("dlg.help")} aria-pressed={help} title={t(help ? "dlg.helpHide" : "dlg.helpShow")}>
      ?
    </button>
  );
}

/**
 * A modal window with a title, content and buttons; Esc or the × closes it. While points are picked in
 * the view for it, it is hidden (its fields keep their values) and Esc ends the picking instead. A window
 * opened from inside another (with `onClose`) lies on top of it.
 */
export function Dialog(props: { title: string; children: ReactNode; buttons?: ReactNode; wide?: boolean; onClose?: () => void }) {
  const close = props.onClose ?? (() => useStore.getState().openDialog(null));
  const picking = useStore((s) => s.pick !== null);
  const self = useRef({});
  const box = useRef<HTMLDivElement>(null);
  // The ? button shows only on windows that have explanations (looked up after each render).
  const [hasHelp, setHasHelp] = useState(false);
  useLayoutEffect(() => {
    const found = !!box.current?.querySelector(".dialog-body p.hint:not(.note)");
    if (found !== hasHelp) setHasHelp(found);
  });
  useEffect(() => {
    const me = self.current;
    open.push(me);
    return () => void open.splice(open.indexOf(me), 1);
  }, []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && open[open.length - 1] === self.current) {
        e.stopPropagation();
        if (useStore.getState().pick) useStore.getState().cancelPick();
        else close();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [close]);
  return (
    <div className="dialog-backdrop" style={picking ? { display: "none" } : undefined} onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div ref={box} className={props.wide ? "dialog wide" : "dialog"} role="dialog" aria-label={props.title}>
        <div className="dialog-title">
          <span>{props.title}</span>
          <span className="dialog-title-buttons">
            {hasHelp && <HelpButton />}
            <button className="dialog-x" onClick={close} aria-label={t("dlg.closeWindow")} title={t("dlg.closeTitle")}>
              ×
            </button>
          </span>
        </div>
        <div className="dialog-body">{props.children}</div>
        {props.buttons && <div className="dialog-buttons">{props.buttons}</div>}
      </div>
    </div>
  );
}

/** A labelled number input that keeps the text while typing. */
export function NumberField(props: { label: string; value: string; onChange(v: string): void; unit?: string; width?: number; name?: string; disabled?: boolean; title?: string }) {
  return (
    <label className="field" title={props.title}>
      <span className="field-label">{props.label}</span>
      <input
        name={props.name ?? props.label}
        aria-label={props.label}
        value={props.value}
        disabled={props.disabled}
        style={props.width ? { width: props.width } : undefined}
        onChange={(e) => props.onChange(e.target.value)}
      />
      {props.unit && <span className="field-unit">{props.unit}</span>}
    </label>
  );
}

/** Reads a number typed by the user (blank counts as 0 when `blankZero`). */
export function parseNumber(text: string, blankZero = false): number | null {
  const t = text.trim();
  if (t === "") return blankZero ? 0 : null;
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
}
