import { useEffect, useLayoutEffect, useRef } from "react";
import { create } from "zustand";

export interface MenuItem {
  label: string;
  hint?: string;
  disabled?: boolean;
  /** A section title (not clickable). */
  header?: boolean;
  children?: MenuItem[];
  run?: (projectRoot: string | undefined) => void;
}

interface Menu {
  x: number;
  y: number;
  items: MenuItem[];
  parent?: { menu: Menu; itemIndex: number };
}

interface MenuState {
  menu: Menu | null;
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
export function ContextMenuHost({ projectRoot }: { projectRoot?: string }) {
  const menu = useContextMenu((s) => s.menu);
  const close = useContextMenu((s) => s.close);
  const panel = useRef<HTMLDivElement>(null);
  const returnFocus = useRef<number | null>(null);

  function goBack() {
    const parent = menu?.parent;
    if (!parent) return close();
    returnFocus.current = parent.itemIndex;
    useContextMenu.setState({ menu: parent.menu });
  }

  useLayoutEffect(() => {
    const element = panel.current;
    if (!menu || !element) return;
    const { width, height } = element.getBoundingClientRect();
    element.style.left = `${Math.max(4, Math.min(menu.x, window.innerWidth - width - 4))}px`;
    element.style.top = `${Math.max(4, Math.min(menu.y, window.innerHeight - height - 4))}px`;
    const item = returnFocus.current === null
      ? element.querySelector<HTMLButtonElement>("button:not(:disabled):not([data-back])")
      : element.querySelector<HTMLButtonElement>(`[data-item="${returnFocus.current}"]`);
    returnFocus.current = null;
    (item ?? element).focus();
  }, [menu]);

  useEffect(() => {
    if (!menu) return;
    const onDown = (e: MouseEvent) => {
      if (!panel.current?.contains(e.target as Node)) close();
    };
    const onWheel = (e: WheelEvent) => {
      if (!panel.current?.contains(e.target as Node)) close();
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("wheel", onWheel, { passive: true });
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("wheel", onWheel);
      window.removeEventListener("resize", close);
    };
  }, [menu, close]);
  if (!menu) return null;
  return (
    <div
      ref={panel}
      className="context-menu"
      role="menu"
      tabIndex={-1}
      onContextMenu={(e) => e.preventDefault()}
      onKeyDown={(e) => {
        if (e.key === "Escape" || (e.key === "ArrowLeft" && menu.parent)) {
          e.preventDefault();
          e.stopPropagation();
          goBack();
        } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          const items = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"));
          const index = items.indexOf(document.activeElement as HTMLButtonElement);
          const next = e.key === "ArrowDown" ? index + 1 : index < 0 ? items.length - 1 : index - 1;
          items[(next + items.length) % items.length]?.focus();
        } else if (e.key === "ArrowRight" && e.target instanceof HTMLButtonElement && e.target.hasAttribute("aria-haspopup")) {
          e.preventDefault();
          e.target.click();
        } else if (e.key === "Tab") {
          close();
        }
      }}
    >
      {menu.parent && <button className="context-menu-item" role="menuitem" data-back onClick={goBack}>‹ Back</button>}
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
            data-item={i}
            aria-haspopup={item.children ? "menu" : undefined}
            disabled={item.disabled}
            title={item.hint}
            onClick={() => {
              if (useContextMenu.getState().menu !== menu) return;
              if (item.children) {
                useContextMenu.setState({
                  menu: { x: menu.x, y: menu.y, items: item.children, parent: { menu, itemIndex: i } },
                });
                return;
              }
              close();
              item.run?.(projectRoot);
            }}
          >
            <span>{item.label}{item.children && <span aria-hidden="true"> ›</span>}</span>
            {item.hint && <span className="context-menu-hint">{item.hint}</span>}
          </button>
        ),
      )}
    </div>
  );
}
