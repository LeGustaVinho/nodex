import { useEffect } from "react";
import { create } from "zustand";

export interface MenuItem {
  label: string;
  hint?: string;
  disabled?: boolean;
  /** A section title (not clickable). */
  header?: boolean;
  run?: () => void;
}

interface MenuState {
  menu: { x: number; y: number; items: MenuItem[] } | null;
  open: (x: number, y: number, items: MenuItem[]) => void;
  close: () => void;
}

export const useContextMenu = create<MenuState>((set) => ({
  menu: null,
  open: (x, y, items) => set({ menu: { x, y, items } }),
  close: () => set({ menu: null }),
}));

/** Opens `items` at the mouse position of a right-click. */
export function openMenu(e: { clientX: number; clientY: number; preventDefault: () => void; stopPropagation?: () => void }, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation?.();
  useContextMenu.getState().open(e.clientX, e.clientY, items);
}

/** The one context menu of the app; closes on Escape, a click elsewhere or scrolling. */
export function ContextMenuHost() {
  const menu = useContextMenu((s) => s.menu);
  const close = useContextMenu((s) => s.close);
  useEffect(() => {
    if (!menu) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    const onDown = (e: MouseEvent) => {
      if (!(e.target as HTMLElement | null)?.closest(".context-menu")) close();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", onDown);
    window.addEventListener("wheel", close, { passive: true });
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("wheel", close);
      window.removeEventListener("resize", close);
    };
  }, [menu, close]);
  if (!menu) return null;
  // Keep the menu on screen.
  const x = Math.min(menu.x, window.innerWidth - 260);
  const y = Math.min(menu.y, window.innerHeight - 28 * menu.items.length - 12);
  return (
    <div className="context-menu" role="menu" style={{ left: Math.max(4, x), top: Math.max(4, y) }} onContextMenu={(e) => e.preventDefault()}>
      {menu.items.map((item, i) =>
        item.header ? (
          <div key={i} className="context-menu-header">
            {item.label}
          </div>
        ) : (
          <button
            key={i}
            role="menuitem"
            className="context-menu-item"
            disabled={item.disabled}
            title={item.hint}
            onClick={() => {
              close();
              item.run?.();
            }}
          >
            {item.label}
            {item.hint && <span className="context-menu-hint">{item.hint}</span>}
          </button>
        ),
      )}
    </div>
  );
}
