import { useEffect, useRef } from "react";

export interface MenuItem {
  label: string;
  run(): void;
  enabled?: boolean;
}

/** A right-click menu at a screen position. "-" entries are separators. */
export function ContextMenu(props: { x: number; y: number; items: (MenuItem | "-")[]; onClose(): void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const close = (e: Event) => {
      if (!ref.current?.contains(e.target as Node)) props.onClose();
    };
    const key = (e: KeyboardEvent) => e.key === "Escape" && props.onClose();
    window.addEventListener("mousedown", close, true);
    window.addEventListener("keydown", key, true);
    window.addEventListener("wheel", props.onClose, true);
    return () => {
      window.removeEventListener("mousedown", close, true);
      window.removeEventListener("keydown", key, true);
      window.removeEventListener("wheel", props.onClose, true);
    };
  }, [props]);
  // Keep the menu on screen.
  const left = Math.min(props.x, window.innerWidth - 230);
  const top = Math.min(props.y, window.innerHeight - 30 - 26 * props.items.length);
  return (
    <div className="menu-items context-menu" role="menu" ref={ref} style={{ left, top }}>
      {props.items.map((item, i) =>
        item === "-" ? (
          <div key={i} className="menu-sep" />
        ) : (
          <button
            key={item.label}
            role="menuitem"
            className="menu-item"
            disabled={item.enabled === false}
            onClick={() => {
              props.onClose();
              item.run();
            }}
          >
            <span className="check" />
            <span className="label">{item.label}</span>
          </button>
        ),
      )}
    </div>
  );
}
