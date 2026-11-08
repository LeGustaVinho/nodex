import { useEffect, useMemo, useRef, useState } from "react";
import type { Symbol } from "../gen/Symbol";
import { rankSymbols } from "../graph/search";
import { type View, useUi } from "../store";
import { UML_KINDS } from "../uml/uml";
import { revealInHierarchy, useHierarchy } from "../views/hierarchyAccess";
import { useStructure } from "../views/Views";

interface Command {
  title: string;
  hint: string;
  run: () => void;
}

const VIEW_COMMANDS: [View, string, string][] = [
  ["overview", "Overview", "What the project is: entry points, routes, hotspots"],
  ["hierarchy", "Hierarchy", "Folders, namespaces, types and members as nested boxes"],
  ["knowledge", "Knowledge", "HTTP calls, DI, tests, throws and writes"],
  ["uml", "UML", "Class, package and component diagrams"],
  ["architecture", "Architecture", "Namespaces, modules and packages"],
  ["types", "Types", "Classes, interfaces and how they relate"],
  ["calls", "Calls", "Callers and callees of a function"],
  ["timeline", "Timeline", "Function calls of the last run"],
  ["blueprint", "Blueprint", "The steps inside one function"],
];

/** True when a key press targets a text field (the palette shortcut `/` must not steal it). */
function typing(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable || !!el.closest(".monaco-editor"));
}

/** Command palette (Ctrl+K or `/`): jump to any symbol or view. */
export function Palette() {
  const open = useUi((s) => s.paletteOpen);
  const setOpen = useUi((s) => s.setPaletteOpen);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen(!useUi.getState().paletteOpen);
      } else if (e.key === "/" && !typing(e.target) && !useUi.getState().paletteOpen) {
        e.preventDefault();
        setOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setOpen]);

  if (!open) return null;
  return <PaletteDialog onClose={() => setOpen(false)} />;
}

function PaletteDialog({ onClose }: { onClose: () => void }) {
  const { index } = useStructure();
  const h = useHierarchy();
  const ui = useUi();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);

  const commands: Command[] = useMemo(() => {
    const list: Command[] = VIEW_COMMANDS.map(([id, title, hint]) => ({ title: `Go to ${title}`, hint, run: () => useUi.getState().setView(id) }));
    list.push({ title: "Close every box in the Hierarchy", hint: "Collapse all", run: () => useUi.getState().collapseTree() });
    // UML diagrams of the project, and of the selected symbol when there is one.
    const sel = useUi.getState().selection;
    for (const k of UML_KINDS.filter((k) => k.ready)) {
      list.push({ title: `UML: ${k.label} of the project`, hint: k.question, run: () => useUi.getState().showUml({ kind: "Project" }, k.id) });
      if (sel?.kind === "symbol") {
        const id = sel.id;
        list.push({ title: `UML: ${k.label} of the selection`, hint: id, run: () => useUi.getState().showUml({ kind: "Symbol", id }, k.id) });
      }
    }
    const q = query.trim().toLowerCase();
    return q ? list.filter((c) => c.title.toLowerCase().includes(q) || c.hint.toLowerCase().includes(q)) : list;
  }, [query]);
  const symbols: Symbol[] = useMemo(() => (index ? rankSymbols(index.graph.symbols, query) : []), [index, query]);
  const total = symbols.length + commands.length;

  const choose = (i: number, alt: boolean) => {
    if (i < 0 || i >= total) return;
    if (i < symbols.length) {
      const s = symbols[i];
      onClose();
      if (alt && s.has_flow) ui.openFlow(s.id);
      else if (h && index && !s.external) revealInHierarchy(h, index, s.id);
      else ui.select({ kind: "symbol", id: s.id }, s.origins[0] ?? null);
      return;
    }
    const c = commands[i - symbols.length];
    if (c) {
      onClose();
      c.run();
    }
  };

  return (
    <div className="palette-backdrop" onMouseDown={onClose}>
      <div className="palette" role="dialog" aria-label="Command palette" onMouseDown={(e) => e.stopPropagation()}>
        <input
          ref={input}
          className="palette-input"
          placeholder="Search symbols and commands…"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            else if (e.key === "ArrowDown") {
              e.preventDefault();
              setActive((a) => Math.max(0, Math.min(total - 1, a + 1)));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setActive((a) => Math.max(0, a - 1));
            } else if (e.key === "Enter") {
              e.preventDefault();
              choose(active, e.shiftKey);
            }
          }}
        />
        <ul className="palette-list">
          {symbols.map((s, i) => (
            <li key={s.id}>
              <button className={`palette-item ${i === active ? "active" : ""}`} onMouseEnter={() => setActive(i)} onClick={(e) => choose(i, e.shiftKey)}>
                <span className="palette-kind">{s.kind}</span>
                <span className="palette-title">{s.name}</span>
                <span className="palette-hint">{s.external ? "external" : s.qualified_name}</span>
              </button>
            </li>
          ))}
          {commands.map((c, j) => {
            const i = symbols.length + j;
            return (
              <li key={c.title}>
                <button className={`palette-item ${i === active ? "active" : ""}`} onMouseEnter={() => setActive(i)} onClick={() => choose(i, false)}>
                  <span className="palette-kind">command</span>
                  <span className="palette-title">{c.title}</span>
                  <span className="palette-hint">{c.hint}</span>
                </button>
              </li>
            );
          })}
          {total === 0 && <li className="palette-empty">Nothing matches.</li>}
        </ul>
        <div className="palette-foot">
          <kbd>↑↓</kbd> move · <kbd>Enter</kbd> show in Hierarchy · <kbd>Shift</kbd>+<kbd>Enter</kbd> open Blueprint · <kbd>Esc</kbd> close
        </div>
      </div>
    </div>
  );
}
