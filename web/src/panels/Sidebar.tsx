import { useQuery } from "@tanstack/react-query";
import { Fragment, type ReactNode, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api } from "../api";
import { openMenu } from "../components/ContextMenu";
import { fileMenuItems } from "../fileMenu";
import { navigationMenuItems } from "../navigation";
import { blueprintMenuItems, contextMenuItems } from "../uml/contextMenu";
import { Splitter } from "../components/Splitter";
import type { Symbol } from "../gen/Symbol";
import { type FileNode, fileTree } from "../graph/fileTree";
import { sidebarSymbolRows, sidebarSymbolsInFile } from "../graph/sidebarSymbols";
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
      openCodeFile: s.openCodeFile,
      sidebarReveal: s.sidebarReveal,
      showUml: s.showUml,
    })),
  );
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [tab, setTab] = useState<"symbols" | "files">("symbols");
  const [revealed, setRevealed] = useState<{ tab: typeof tab; key: string } | null>(null);
  const handledReveal = useRef<typeof ui.sidebarReveal>(null);
  const q = query.trim().toLowerCase();
  // Built only once the Files tab is shown.
  const files = useMemo(
    () => (tab === "files" ? fileTree((project.data?.files ?? []).map((f) => f.path)) : []),
    [project.data, tab],
  );

  useLayoutEffect(() => {
    const target = ui.sidebarReveal;
    if (!target || handledReveal.current === target || !index || !project.data) return;
    handledReveal.current = target;
    let key: string;
    let ancestors: string[];
    if (target.tab === "files") {
      if (!project.data.files.some((file) => file.path === target.file)) return;
      key = `file:${target.file}`;
      const parts = target.file.split("/");
      ancestors = parts.slice(0, -1).map((_, i) => `file:${parts.slice(0, i + 1).join("/")}`);
    } else {
      const row = sidebarSymbolsInFile(index, target.file)?.get(target.id);
      if (!row) return;
      ({ key, ancestors } = row);
    }
    setOpen((current) => ({ ...current, ...Object.fromEntries(ancestors.map((id) => [id, true])) }));
    setQuery("");
    setTab(target.tab);
    setRevealed({ tab: target.tab, key });
  }, [ui.sidebarReveal, index, project.data]);

  if (!index) return <aside className="sidebar">Loading…</aside>;

  const isOpen = (id: string) => (q ? true : !!open[id]);
  const toggle = (id: string) => setOpen((o) => ({ ...o, [id]: !o[id] }));

  const selectType = (type: Symbol, origin = type.origins[0]) => {
    ui.setNamespace(index.namespaceOf(type.id));
    ui.select({ kind: "symbol", id: type.id }, origin ?? null);
    if (ui.view === "blueprint" && !type.external) {
      if (ui.flowSymbol !== type.id) ui.openFlow(type.id);
    } else if (ui.view !== "types") {
      ui.setView("types");
    }
  };

  const openFileBlueprint = (file: string, preservePosition = false) => {
    const target = index.fileBlueprint(file, ui.view === "blueprint");
    if (!target) return false;
    ui.setNamespace(index.namespaceOf(target.id));
    ui.select({ kind: "symbol", id: target.id }, preservePosition ? undefined : target.origins.find((origin) => origin.file === file));
    if (ui.view !== "blueprint" || ui.flowSymbol !== target.id) ui.openFlow(target.id);
    return true;
  };

  const clickSymbol = (s: Symbol) => {
    setRevealed(null);
    const origin = s.origins[0];
    if (isScopeKind(s.kind)) {
      if (ui.view === "blueprint" && s.kind === "Module" && origin && !s.flavors.includes("inline-module") && openFileBlueprint(origin.file)) return;
      ui.setNamespace(s.qualified_name);
      ui.setView("types");
      ui.select({ kind: "symbol", id: s.id }, origin ?? null);
      toggle(s.id);
      return;
    }
    if (isTypeKind(s.kind)) {
      selectType(s);
      return;
    }
    ui.select({ kind: "symbol", id: s.id }, origin ?? null);
    if (s.has_flow) ui.openFlow(s.id);
  };

  // The tree is a flat list of rows, so only the visible ones are rendered (a TypeScript project
  // has a module per file: tens of thousands of rows).
  const symbolRows: VirtualRow[] = tab !== "symbols" ? [] : sidebarSymbolRows(index, q, isOpen).map(({ key, symbol, kind, ancestors }) => {
    const active = kind === "scope"
      ? (ui.namespace === symbol.qualified_name && ui.view === "types") || (ui.view === "blueprint" && ui.flowSymbol === symbol.id)
      : kind === "type" ? ui.selection?.kind === "symbol" && ui.selection.id === symbol.id : ui.flowSymbol === symbol.id;
    const highlighted = revealed?.tab === "symbols" && revealed.key === key;
    return { key, render: () => (
      <TreeRow
        rowKey={key}
        symbol={symbol}
        owner={index.byId.get(symbol.parent ?? "")}
        depth={ancestors.length}
        caret={kind === "member" ? undefined : isOpen(symbol.id) ? "▾" : "▸"}
        onCaret={kind === "member" ? undefined : () => toggle(symbol.id)}
        active={revealed?.tab === "symbols" ? highlighted : active}
        onClick={() => clickSymbol(symbol)}
      />
    ) };
  });

  const problems = project.data?.files.filter((f) => f.diagnostics.length > 0) ?? [];

  const fileMenu = (e: React.MouseEvent, node: FileNode) =>
    openMenu(e, [
      ...(node.folder ? contextMenuItems({ kind: "folder", path: node.path }) : fileMenuItems(node.path)),
      ...umlMenuItems(node.folder ? { kind: "Folder", path: node.path } : { kind: "File", path: node.path }, `UML of ${node.name}`),
    ]);
  const fileRows: VirtualRow[] = [];
  const pushFile = (node: FileNode, depth: number) => {
    if (q && !node.path.toLowerCase().includes(q)) return;
    const key = `file:${node.path}`;
    fileRows.push({
      key,
      render: () => (
        <div className={`row ${revealed?.key === key ? "active" : ""}`} style={{ paddingLeft: 6 + depth * 14 }} onContextMenu={(e) => fileMenu(e, node)}>
          <span className="caret" onClick={() => node.folder && toggle(key)}>
            {node.folder ? (isOpen(key) ? "▾" : "▸") : ""}
          </span>
          <button
            className="row-main"
            data-sidebar-row={key}
            title={`${node.path} — right-click for more actions`}
            onClick={() => {
              if (node.folder) toggle(key);
              else {
                setRevealed({ tab: "files", key });
                const alreadyOpen = useUi.getState().codeTabs.includes(node.path);
                ui.openCodeFile(node.path);
                openFileBlueprint(node.path, alreadyOpen);
              }
            }}
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
        <button className={tab === "files" ? "on" : ""} aria-selected={tab === "files"} onClick={() => setTab("files")} title="Folders and files; right-click for more actions">
          Files
        </button>
      </div>
      <input
        className="search"
        placeholder={tab === "files" ? "Filter files…" : "Search types and members…"}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <VirtualList rows={tab === "files" ? fileRows : symbolRows} reveal={revealed?.tab === tab ? revealed : null} />
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

function VirtualList({ rows, reveal }: { rows: VirtualRow[]; reveal: { key: string } | null }) {
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
  const focused = useRef<typeof reveal>(null);
  useLayoutEffect(() => {
    const list = ref.current;
    if (!list || !reveal || focused.current === reveal) return;
    const index = rows.findIndex((row) => row.key === reveal.key);
    if (index < 0) return;
    const top = index * ROW_HEIGHT;
    if (top < list.scrollTop || top + ROW_HEIGHT > list.scrollTop + list.clientHeight) {
      list.scrollTop = Math.max(0, top - Math.floor(list.clientHeight / 2));
      setScroll(list.scrollTop);
    }
    const button = [...list.querySelectorAll<HTMLButtonElement>("[data-sidebar-row]")].find((el) => el.dataset.sidebarRow === reveal.key);
    if (button) {
      focused.current = reveal;
      button.focus({ preventScroll: true });
    }
  }, [rows, reveal, first, last]);
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

function TreeRow(props: { rowKey: string; symbol: Symbol; owner?: Symbol; depth: number; caret?: string; onCaret?: () => void; active?: boolean; onClick: () => void }) {
  const { rowKey, symbol, owner, depth, caret, onCaret, active, onClick } = props;
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
      <button className="row-main" data-sidebar-row={rowKey} onClick={onClick} title={[symbol.signature ?? symbol.qualified_name, symbol.doc].filter(Boolean).join("\n\n")}>
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
