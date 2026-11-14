import { useEffect, useRef, useState } from "react";

import { useShallow } from "zustand/react/shallow";

import { useStore } from "../state/store";
import { t } from "../i18n";
import { cmdLabel, commands, menus, type Command } from "./commands";

export function MenuBar() {
  const [open, setOpen] = useState<string | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  // Re-render when state that enables or checks items changes.
  useStore(useShallow((s) => [s.selection, s.results, s.past.length, s.future.length, s.tool, s.view, s.display, s.running, s.hidden, s.leftTab]));

  useEffect(() => {
    const close = (e: MouseEvent) => {
      if (!bar.current?.contains(e.target as Node)) setOpen(null);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, []);

  return (
    <div className="menubar" ref={bar} role="menubar">
      {menus.map((m) => (
        <div key={m.title} className="menu">
          <button
            className={open === m.title ? "menu-title open" : "menu-title"}
            onClick={() => setOpen(open === m.title ? null : m.title)}
            onMouseEnter={() => open && setOpen(m.title)}
          >
            {t(m.title)}
          </button>
          {open === m.title && (
            <div className="menu-items" role="menu">
              {m.items.map((name, i) => {
                if (name === "-") return <div key={i} className="menu-sep" />;
                const c: Command = commands[name];
                const enabled = c.enabled ? c.enabled() : true;
                return (
                  <button
                    key={name}
                    role="menuitem"
                    className="menu-item"
                    disabled={!enabled}
                    onClick={() => {
                      setOpen(null);
                      c.run();
                    }}
                  >
                    <span className="check">{c.checked?.() ? "✓" : ""}</span>
                    <span className="label">{cmdLabel(name)}</span>
                    <span className="shortcut">{c.shortcut ?? ""}</span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
