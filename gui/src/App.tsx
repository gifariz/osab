import { useEffect } from "react";

import { confirmDiscard, openFile } from "./actions";
import { exampleOnOpen } from "./demo";
import { backend } from "./ipc/backend";
import { usePrefs } from "./state/prefs";
import { useStore } from "./state/store";
import { commands, shortcuts } from "./ui/commands";
import { Dialogs } from "./ui/dialogs/Dialogs";
import { MemberDiagram } from "./ui/MemberDiagram";
import { MenuBar } from "./ui/MenuBar";
import { SidePanel } from "./ui/SidePanel";
import { StatusBar } from "./ui/StatusBar";
import { Toolbar } from "./ui/Toolbar";
import { ViewportPanel, finishArea } from "./ui/ViewportPanel";

function keyName(e: KeyboardEvent) {
  return `${e.ctrlKey || e.metaKey ? "Ctrl+" : ""}${e.shiftKey && (e.ctrlKey || e.metaKey) ? "Shift+" : ""}${e.key}`;
}

function onKeyDown(e: KeyboardEvent) {
  const s = useStore.getState();
  const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
  if (s.dialog) return;
  if (e.key === "Escape") {
    if (s.chainStart) s.setChainStart(null);
    else if (s.measureStart) s.setMeasureStart(null);
    else if (s.areaCorners.length) s.setAreaCorners([]);
    else if (s.tool !== "select") s.setTool("select");
    else s.clearSelection();
    return;
  }
  if (typing) return;
  if (e.key === "Enter" && s.tool === "area" && s.areaCorners.length) {
    finishArea();
    return;
  }
  const name = shortcuts[keyName(e)];
  if (!name) return;
  e.preventDefault();
  const c = commands[name];
  if (!("enabled" in c) || !c.enabled || c.enabled()) c.run();
}

function Notice() {
  const notice = useStore((s) => s.notice);
  // While points are picked, clicks go through the hint to the view.
  const picking = useStore((s) => s.pick !== null);
  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => useStore.setState({ notice: null }), notice.level === "error" ? 9000 : 4500);
    return () => clearTimeout(t);
  }, [notice]);
  if (!notice) return null;
  return (
    <div className={`notice ${notice.level}`} role="status" style={picking ? { pointerEvents: "none" } : undefined} onClick={() => useStore.setState({ notice: null })}>
      {notice.text}
    </div>
  );
}

// Ask before closing the window with unsaved changes (registered once).
backend.onCloseRequested(() => useStore.getState().dirty, confirmDiscard);
exampleOnOpen();
// A model file the desktop app was started with (double-clicked in the file explorer).
void backend.startupFile().then((path) => {
  if (path) void openFile(path);
});

export function App() {
  const fileName = useStore((s) => s.fileName);
  const dirty = useStore((s) => s.dirty);
  const memberPanel = useStore((s) => s.memberPanel);
  // Re-render everything when the language changes (all text comes from t()).
  usePrefs((p) => p.lang);
  const themeChoice = usePrefs((p) => p.theme);

  useEffect(() => {
    backend.setTheme(themeChoice === "system" ? null : themeChoice);
  }, [themeChoice]);

  useEffect(() => {
    backend.setTitle(`${dirty ? "* " : ""}${fileName} - OSAB`);
  }, [fileName, dirty]);

  useEffect(() => {
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="app">
      <MenuBar />
      <Toolbar />
      <div className="main">
        <SidePanel />
        <ViewportPanel />
        {memberPanel && <MemberDiagram />}
      </div>
      <StatusBar />
      <Notice />
      <Dialogs />
    </div>
  );
}
