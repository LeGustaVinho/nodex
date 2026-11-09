import { useQuery } from "@tanstack/react-query";
import { Fragment, type ReactNode, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api } from "../api";
import { openMenu } from "../components/ContextMenu";
import { navigationMenuItems } from "../navigation";
import { blueprintMenuItems, contextMenuItems } from "../uml/contextMenu";
import { Splitter } from "../components/Splitter";
import type { Symbol } from "../gen/Symbol";
import { type FileNode, fileTree } from "../graph/fileTree";
import { umlMenuItems } from "../uml/menu";
import { isScopeKind, isTypeKind } from "../graph/structureGraph";
import { useUi } from "../store";
import { useStructure } from "../views/Views";

const ICON: Record<string, string> = {
  Namespace: "{}",
  Module: "▤",
  TypeAlias: "τ",
  Class: "C",
  Struct: "S",
  Interface: "I",
  Trait: "T",
  Enum: "E",
  Union: "U",
  Record: "R",
  Delegate: "D",
  Method: "ƒ",
  Function: "ƒ",
  Constructor: "✦",
  Property: "◈",
  Field: "●",
  Event: "⚡",
  EnumMember: "•",
};

export function Sidebar() {
  const { index } = useStructure();
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  // Only what the tree shows: other store changes (connection, toggles…) do not re-render it.
  const ui = useUi(
    useShallow((s) => ({
      view: s.view,
      namespace: s.namespace,
      selection: s.selection,
      flowSymbol: s.flowSymbol,
      setNamespace: s.setNamespace,
      setView: s.setView,
      select: s.select,
      openFlow: s.openFlow,
      focusCode: s.focusCode,
      showUml: s.showUml,
    })),
  );
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [tab, setTab] = useState<"symbols" | "files">("symbols");
  const q = query.trim().toLowerCase();
  // Built only once the Files tab is shown.
  const files = useMemo(
    () => (tab === "files" ? fileTree((project.data?.files ?? []).map((f) => f.path)) : []),
    [project.data, tab],
  );

  const namespaces = useMemo(() => {
    if (!index) return [];
    return index.graph.symbols
      .filter((s) => isScopeKind(s.kind) && !s.external)
      .sort((a, b) => a.qualified_name.localeCompare(b.qualified_name));
  }, [index]);

  // Top-level types of each namespace (nested types appear under their parent).
  const topTypes = useMemo(() => {
    const map = new Map<string, Symbol[]>();
    if (!index) return map;
    for (const ns of namespaces) {
      map.set(ns.id, index.typesIn(ns.qualified_name).filter((t) => !t.parent || isScopeKind(index.byId.get(t.parent)?.kind ?? "Class")));
    }
    return map;
  }, [index, namespaces]);

  if (!index) return <aside className="sidebar">Loading…</aside>;

  const matches = (s: Symbol) => !q || s.name.toLowerCase().includes(q);
  const typeVisible = (t: Symbol): boolean => matches(t) || index.members(t.id).some(matches);
  const isOpen = (id: string) => (q ? true : !!open[id]);
  const toggle = (id: string) => setOpen((o) => ({ ...o, [id]: !o[id] }));

  const clickSymbol = (s: Symbol) => {
    if (isScopeKind(s.kind)) {
      ui.setNamespace(s.qualified_name);
      ui.setView("types");
      ui.select({ kind: "symbol", id: s.id }, s.origins[0] ?? null);
      toggle(s.id);
      return;
    }
    if (isTypeKind(s.kind)) {
      ui.setNamespace(index.namespaceOf(s.id));
      ui.select({ kind: "symbol", id: s.id }, s.origins[0] ?? null);
      if (ui.view === "blueprint" && !s.external) {
        if (ui.flowSymbol !== s.id) ui.openFlow(s.id);
      } else if (ui.view !== "types") {
        ui.setView("types");
      }
      return;
    }
    ui.select({ kind: "symbol", id: s.id }, s.origins[0] ?? null);
    if (s.has_flow) ui.openFlow(s.id);
  };

  // The tree is a flat list of rows, so only the visible ones are rendered (a TypeScript project
  // has a module per file: tens of thousands of rows).
  const symbolRows: VirtualRow[] = [];
  const row = (symbol: Symbol, depth: number, extra: { caret?: string; onCaret?: () => void; active?: boolean } = {}) =>
    symbolRows.push({ key: symbol.id, render: () => (
      <TreeRow symbol={symbol} owner={index.byId.get(symbol.parent ?? "")} depth={depth} {...extra} onClick={() => clickSymbol(symbol)} />
    ) });

  const pushMembers = (owner: Symbol, depth: number) => {
    for (const m of index.members(owner.id)) {
      if (!(matches(m) || matches(owner) || (index.children.get(m.id) ?? []).some(matches))) continue;
      row(m, depth, { active: ui.flowSymbol === m.id });
      for (const a of (index.children.get(m.id) ?? []).filter((c) => c.has_flow)) row(a, depth + 1, { active: ui.flowSymbol === a.id });
    }
  };

  const pushType = (t: Symbol, depth: number) => {
    if (!typeVisible(t)) return;
    row(t, depth, {
      caret: isOpen(t.id) ? "▾" : "▸",
      onCaret: () => toggle(t.id),
      active: ui.selection?.kind === "symbol" && ui.selection.id === t.id,
    });
    if (!isOpen(t.id)) return;
    pushMembers(t, depth + 1);
    for (const n of (index.children.get(t.id) ?? []).filter((c) => isTypeKind(c.kind))) pushType(n, depth + 1);
  };

  if (tab === "symbols") {
    for (const ns of namespaces) {
      const types = topTypes.get(ns.id) ?? [];
      const free = index.freeMembers(ns.id);
      if (q && !types.some(typeVisible) && !free.some(matches) && !matches(ns)) continue;
      row(ns, 0, {
        caret: isOpen(ns.id) ? "▾" : "▸",
        onCaret: () => toggle(ns.id),
        active: ui.namespace === ns.qualified_name && ui.view === "types",
      });
      if (!isOpen(ns.id)) continue;
      if (free.length > 0) pushMembers(ns, 1);
      for (const t of types) pushType(t, 1);
    }
  }

  const problems = project.data?.files.filter((f) => f.diagnostics.length > 0) ?? [];

  const fileMenu = (e: React.MouseEvent, node: FileNode) =>
    openMenu(e, [
      ...(node.folder ? contextMenuItems({ kind: "folder", path: node.path }) : []),
      ...umlMenuItems(node.folder ? { kind: "Folder", path: node.path } : { kind: "File", path: node.path }, `UML of ${node.name}`),
    ]);
  const fileRows: VirtualRow[] = [];
  const pushFile = (node: FileNode, depth: number) => {
    if (q && !node.path.toLowerCase().includes(q)) return;
    const key = `file:${node.path}`;
    fileRows.push({
      key,
      render: () => (
        <div className="row" style={{ paddingLeft: 6 + depth * 14 }} onContextMenu={(e) => fileMenu(e, node)}>
          <span className="caret" onClick={() => node.folder && toggle(key)}>
            {node.folder ? (isOpen(key) ? "▾" : "▸") : ""}
          </span>
          <button
            className="row-main"
            title={`${node.path} — right-click for UML diagrams`}
            onClick={() =>
              node.folder
                ? toggle(key)
                : ui.focusCode({ file: node.path, start_byte: 0, end_byte: 0, start_line: 1, start_col: 1, end_line: 1, end_col: 1 })
            }
          >
            <span className="sym-icon">{node.folder ? "▣" : "▤"}</span>
            <span className="sym-name">{node.name}</span>
            {node.folder && <span className="muted small">{node.files}</span>}
          </button>
        </div>
      ),
    });
    if (node.folder && isOpen(key)) for (const c of node.children) pushFile(c, depth + 1);
  };
  if (tab === "files") {
    fileRows.push({
      key: "project",
      render: () => (
        <div className="row" onContextMenu={(e) => openMenu(e, umlMenuItems({ kind: "Project" }, "UML of the project"))}>
          <span className="caret" />
          <button className="row-main" title="Right-click for UML diagrams of the whole project" onClick={() => ui.showUml({ kind: "Project" })}>
            <span className="sym-icon">◆</span>
            <span className="sym-name">{project.data?.name ?? "project"}</span>
          </button>
        </div>
      ),
    });
    for (const f of files) pushFile(f, 1);
  }

  return (
    <aside className="sidebar">
      <Splitter size="sidebar-w" axis="x" target="parent" grow={1} min={150} max={(app) => app.clientWidth * 0.45} label="Resize the sidebar" className="edge-right" />
      <div className="segmented sidebar-tabs" role="tablist">
        <button className={tab === "symbols" ? "on" : ""} aria-selected={tab === "symbols"} onClick={() => setTab("symbols")}>
          Symbols
        </button>
        <button className={tab === "files" ? "on" : ""} aria-selected={tab === "files"} onClick={() => setTab("files")} title="Folders and files; right-click for UML diagrams">
          Files
        </button>
      </div>
      <input
        className="search"
        placeholder={tab === "files" ? "Filter files…" : "Search types and members…"}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <VirtualList rows={tab === "files" ? fileRows : symbolRows} />
      {problems.length > 0 && (
        <Splitter size="problems-h" axis="y" target="next" grow={-1} min={40} max={(sidebar) => sidebar.clientHeight - 140} label="Resize the problems list" />
      )}
      {problems.length > 0 && (
        <div className="problems">
          <div className="section-title">Problems</div>
          {problems.map((f) => (
            <div key={f.path} className="problem" title={f.diagnostics.map((d) => d.message).join("\n")}>
              ⚠ {f.path}
            </div>
          ))}
        </div>
      )}
    </aside>
  );
}

/** One row of the sidebar tree. */
interface VirtualRow {
  key: string;
  render: () => ReactNode;
}

/** Every row is this tall (`.tree .row` in styles.css): the list renders only those in view. */
const ROW_HEIGHT = 22;
/** Rows rendered above and below the visible ones, so fast scrolling shows no gap. */
const OVERSCAN = 12;

function VirtualList({ rows }: { rows: VirtualRow[] }) {
  const ref = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState(0);
  const [height, setHeight] = useState(800);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver(() => setHeight(el.clientHeight));
    observer.observe(el);
    setHeight(el.clientHeight);
    return () => observer.disconnect();
  }, []);
  const first = Math.max(0, Math.floor(scroll / ROW_HEIGHT) - OVERSCAN);
  const last = Math.min(rows.length, Math.ceil((scroll + height) / ROW_HEIGHT) + OVERSCAN);
  return (
    <div className="tree" ref={ref} onScroll={(e) => setScroll(e.currentTarget.scrollTop)}>
      <div style={{ height: rows.length * ROW_HEIGHT }}>
        <div style={{ transform: `translateY(${first * ROW_HEIGHT}px)` }}>
          {rows.slice(first, last).map((r) => (
            <Fragment key={r.key}>{r.render()}</Fragment>
          ))}
        </div>
      </div>
    </div>
  );
}

function TreeRow(props: { symbol: Symbol; owner?: Symbol; depth: number; caret?: string; onCaret?: () => void; active?: boolean; onClick: () => void }) {
  const { symbol, owner, depth, caret, onCaret, active, onClick } = props;
  return (
    <div
      className={`row ${active ? "active" : ""}`}
      style={{ paddingLeft: 6 + depth * 14 }}
      onContextMenu={(e) => openMenu(e, [
        ...navigationMenuItems(symbol, owner),
        ...(!symbol.external ? contextMenuItems({ kind: "symbol", id: symbol.id }) : []),
        ...blueprintMenuItems(symbol),
        ...umlMenuItems({ kind: "Symbol", id: symbol.id }, `UML of ${symbol.name}`),
      ])}
    >
      <span
        className="caret"
        onClick={(e) => {
          e.stopPropagation();
          onCaret?.();
        }}
      >
        {caret ?? ""}
      </span>
      <button className="row-main" onClick={onClick} title={[symbol.signature ?? symbol.qualified_name, symbol.doc].filter(Boolean).join("\n\n")}>
        <span className={`sym-icon sym-${symbol.kind.toLowerCase()}`}>{ICON[symbol.kind] ?? "·"}</span>
        <span className="sym-name">
          {symbol.kind === "Namespace" ? symbol.qualified_name : symbol.kind === "Module" ? moduleLabel(symbol) : symbol.name}
        </span>
        {symbol.has_flow && <span className="sym-flow">▶</span>}
      </button>
    </div>
  );
}

/** A file module shows its file; a module written inside a file (Rust `mod x { }`) its path. */
function moduleLabel(symbol: { name: string; qualified_name: string; flavors: string[]; origins: { file: string }[] }): string {
  if (symbol.flavors.includes("inline-module")) return symbol.qualified_name;
  return symbol.origins[0]?.file ?? symbol.name;
}
