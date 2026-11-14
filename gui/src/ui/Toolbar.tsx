import { useShallow } from "zustand/react/shallow";

import { useStore } from "../state/store";
import { t, type Key } from "../i18n";
import { cmdLabel, commands, type CommandName } from "./commands";

const groups: { name: CommandName; text: Key; title?: Key }[][] = [
  [
    { name: "newModel", text: "tb.new" },
    { name: "open", text: "tb.open" },
    { name: "save", text: "tb.save" },
  ],
  [
    { name: "undo", text: "tb.undo", title: "tb.undoTitle" },
    { name: "redo", text: "tb.redo", title: "tb.redoTitle" },
  ],
  [
    { name: "select", text: "tb.select" },
    { name: "drawJoint", text: "tb.joint" },
    { name: "drawFrame", text: "tb.frame" },
    { name: "drawArea", text: "tb.area", title: "tb.areaTitle" },
    { name: "drawLink", text: "tb.link", title: "tb.linkTitle" },
  ],
  [
    { name: "view3d", text: "tb.3d" },
    { name: "parallel", text: "tb.parallel", title: "tb.parallelTitle" },
    { name: "viewPlan", text: "tb.plan" },
    { name: "viewXZ", text: "tb.elevXZ" },
    { name: "viewYZ", text: "tb.elevYZ" },
    { name: "viewDown", text: "tb.down", title: "tb.downTitle" },
    { name: "viewUp", text: "tb.up", title: "tb.upTitle" },
    { name: "fit", text: "tb.fit", title: "tb.fitTitle" },
    { name: "measure", text: "tb.measure", title: "tb.measureTitle" },
    { name: "extrude", text: "tb.extrude", title: "tb.extrudeTitle" },
    { name: "displayOptions", text: "tb.display", title: "tb.displayTitle" },
  ],
  [
    { name: "hideSelection", text: "tb.hide", title: "tb.hideTitle" },
    { name: "showOnlySelection", text: "tb.showOnly", title: "tb.showOnlyTitle" },
    { name: "showAll", text: "tb.showAll", title: "tb.showAllTitle" },
  ],
  [{ name: "run", text: "tb.run" }],
  [
    { name: "showModel", text: "tb.model", title: "tb.modelTitle" },
    { name: "showDeformed", text: "tb.deformed" },
    { name: "showForces", text: "tb.forces" },
    { name: "showShellForces", text: "tb.shellForces" },
    { name: "showReactions", text: "tb.reactions" },
    { name: "tables", text: "tb.tables" },
  ],
];

export function Toolbar() {
  useStore(useShallow((s) => [s.selection, s.results, s.past.length, s.future.length, s.tool, s.view, s.display, s.running, s.hidden, s.leftTab]));
  const running = useStore((s) => s.running);
  return (
    <div className="toolbar">
      {groups.map((g, i) => (
        <div key={i} className="tool-group">
          {g.map((b) => {
            const c = commands[b.name];
            const enabled = "enabled" in c && c.enabled ? c.enabled() : true;
            const checked = "checked" in c && c.checked ? c.checked() : false;
            return (
              <button
                key={b.name}
                className={checked ? "tool active" : "tool"}
                disabled={!enabled}
                title={b.title ? t(b.title) : `${cmdLabel(b.name)}${"shortcut" in c && c.shortcut ? ` (${c.shortcut})` : ""}`}
                onClick={() => c.run()}
              >
                {b.name === "run" && running ? t("tb.running") : t(b.text)}
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}
