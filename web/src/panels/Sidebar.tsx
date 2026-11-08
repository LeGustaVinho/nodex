import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { api } from "../api";
import { openMenu } from "../components/ContextMenu";
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
  const ui = useUi();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [tab, setTab] = useState<"symbols" | "files">("symbols");
  const q = query.trim().toLowerCase();
  const files = useMemo(() => fileTree((project.data?.files ?? []).map((f) => f.path)), [project.data]);

  const namespaces = useMemo(() => {
    if (!index) return [];
    return index.graph.symbols
      .filter((s) => isScopeKind(s.kind) && !s.external)
      .sort((a, b) => a.qualified_name.localeCompare(b.qualified_name));
  }, [index]);

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
      if (ui.view !== "types") ui.setView("types");
      ui.select({ kind: "symbol", id: s.id }, s.origins[0] ?? null);
      return;
    }
    ui.select({ kind: "symbol", id: s.id }, s.origins[0] ?? null);
    if (s.has_flow) ui.openFlow(s.id);
  };

  const renderMembers = (owner: Symbol, depth: number) =>
    index
      .members(owner.id)
      .filter((m) => matches(m) || matches(owner) || (index.children.get(m.id) ?? []).some(matches))
      .map((m) => {
        const accessors = (index.children.get(m.id) ?? []).filter((c) => c.has_flow);
        return (
          <div key={m.id}>
            <TreeRow symbol={m} depth={depth} active={ui.flowSymbol === m.id} onClick={() => clickSymbol(m)} />
            {accessors.map((a) => (
              <TreeRow key={a.id} symbol={a} depth={depth + 1} active={ui.flowSymbol === a.id} onClick={() => clickSymbol(a)} />
            ))}
          </div>
        );
      });

  const renderType = (t: Symbol, depth: number): React.ReactNode => {
    if (!typeVisible(t)) return null;
    const nested = (index.children.get(t.id) ?? []).filter((c) => isTypeKind(c.kind));
    return (
      <div key={t.id}>
        <TreeRow
          symbol={t}
          depth={depth}
          caret={isOpen(t.id) ? "▾" : "▸"}
          onCaret={() => toggle(t.id)}
          active={ui.selection?.kind === "symbol" && ui.selection.id === t.id}
          onClick={() => clickSymbol(t)}
        />
        {isOpen(t.id) && (
          <>
            {renderMembers(t, depth + 1)}
            {nested.map((n) => renderType(n, depth + 1))}
          </>
        )}
      </div>
    );
  };

  const problems = project.data?.files.filter((f) => f.diagnostics.length > 0) ?? [];

  const fileMenu = (e: React.MouseEvent, node: FileNode) =>
    openMenu(e, umlMenuItems(node.folder ? { kind: "Folder", path: node.path } : { kind: "File", path: node.path }, `UML of ${node.name}`));
  const renderFile = (node: FileNode, depth: number): React.ReactNode => {
    if (q && !node.path.toLowerCase().includes(q)) return null;
    const key = `file:${node.path}`;
    return (
      <div key={key}>
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
        {node.folder && isOpen(key) && node.children.map((c) => renderFile(c, depth + 1))}
      </div>
    );
  };

  return (
    <aside className="sidebar">
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
      {tab === "files" && (
        <div className="tree">
          <div className="row" onContextMenu={(e) => openMenu(e, umlMenuItems({ kind: "Project" }, "UML of the project"))}>
            <span className="caret" />
            <button className="row-main" title="Right-click for UML diagrams of the whole project" onClick={() => ui.showUml({ kind: "Project" })}>
              <span className="sym-icon">◆</span>
              <span className="sym-name">{project.data?.name ?? "project"}</span>
            </button>
          </div>
          {files.map((f) => renderFile(f, 1))}
        </div>
      )}
      {tab === "symbols" && <div className="tree">
        {namespaces.map((ns) => {
          const types = index
            .typesIn(ns.qualified_name)
            .filter((t) => !t.parent || isScopeKind(index.byId.get(t.parent)?.kind ?? "Class"));
          const free = index.freeMembers(ns.id);
          if (q && !types.some(typeVisible) && !free.some(matches) && !matches(ns)) return null;
          return (
            <div key={ns.id}>
              <TreeRow
                symbol={ns}
                depth={0}
                caret={isOpen(ns.id) ? "▾" : "▸"}
                onCaret={() => toggle(ns.id)}
                active={ui.namespace === ns.qualified_name && ui.view === "types"}
                onClick={() => clickSymbol(ns)}
              />
              {isOpen(ns.id) && (
                <>
                  {free.length > 0 && renderMembers(ns, 1)}
                  {types.map((t) => renderType(t, 1))}
                </>
              )}
            </div>
          );
        })}
      </div>}
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

function TreeRow(props: { symbol: Symbol; depth: number; caret?: string; onCaret?: () => void; active?: boolean; onClick: () => void }) {
  const { symbol, depth, caret, onCaret, active, onClick } = props;
  return (
    <div
      className={`row ${active ? "active" : ""}`}
      style={{ paddingLeft: 6 + depth * 14 }}
      onContextMenu={(e) => openMenu(e, umlMenuItems({ kind: "Symbol", id: symbol.id }, `UML of ${symbol.name}`))}
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
