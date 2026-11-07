import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { api } from "../api";
import type { Symbol } from "../gen/Symbol";
import { isTypeKind } from "../graph/structureGraph";
import { useUi } from "../store";
import { useStructure } from "../views/Views";

const ICON: Record<string, string> = {
  Namespace: "{}",
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
  const q = query.trim().toLowerCase();

  const namespaces = useMemo(() => {
    if (!index) return [];
    return index.graph.symbols
      .filter((s) => s.kind === "Namespace" && !s.external)
      .sort((a, b) => a.qualified_name.localeCompare(b.qualified_name));
  }, [index]);

  if (!index) return <aside className="sidebar">Loading…</aside>;

  const matches = (s: Symbol) => !q || s.name.toLowerCase().includes(q);
  const typeVisible = (t: Symbol): boolean => matches(t) || index.members(t.id).some(matches);
  const isOpen = (id: string) => (q ? true : !!open[id]);
  const toggle = (id: string) => setOpen((o) => ({ ...o, [id]: !o[id] }));

  const clickSymbol = (s: Symbol) => {
    if (s.kind === "Namespace") {
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

  return (
    <aside className="sidebar">
      <input className="search" placeholder="Search types and members…" value={query} onChange={(e) => setQuery(e.target.value)} />
      <div className="tree">
        {namespaces.map((ns) => {
          const types = index.typesIn(ns.qualified_name).filter((t) => !t.parent || index.byId.get(t.parent)?.kind === "Namespace");
          if (q && !types.some(typeVisible)) return null;
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
              {isOpen(ns.id) && types.map((t) => renderType(t, 1))}
            </div>
          );
        })}
      </div>
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
    <div className={`row ${active ? "active" : ""}`} style={{ paddingLeft: 6 + depth * 14 }}>
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
        <span className="sym-name">{symbol.kind === "Namespace" ? symbol.qualified_name : symbol.name}</span>
        {symbol.has_flow && <span className="sym-flow">▶</span>}
      </button>
    </div>
  );
}
