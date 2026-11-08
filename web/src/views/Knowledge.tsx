import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../api";
import type { StructEdgeKind } from "../gen/StructEdgeKind";
import { HIERARCHY_CAP, Hierarchy, type HierarchyNodeData, hierarchyGraph, kindsOf } from "../graph/hierarchy";
import { type StructureIndex, isTypeKind } from "../graph/structureGraph";
import { useUi } from "../store";
import { KNOWLEDGE_COLORS } from "./GraphCanvas";
import { GraphCanvas } from "./GraphCanvas";
import { useStructure } from "./Views";

export function useKnowledge() {
  return useQuery({ queryKey: ["knowledge"], queryFn: api.knowledge });
}

const hierarchies = new WeakMap<StructureIndex, Hierarchy>();

/** The containment hierarchy of the current structure (built once per structure graph). */
export function useHierarchy(): Hierarchy | undefined {
  const { index } = useStructure();
  return useMemo(() => {
    if (!index) return undefined;
    let h = hierarchies.get(index);
    if (!h) {
      h = new Hierarchy(index);
      hierarchies.set(index, h);
    }
    return h;
  }, [index]);
}

/** Opens the ancestors of `id` in the Hierarchy view, shows the view and selects the symbol. */
export function revealInHierarchy(h: Hierarchy, index: StructureIndex, id: string) {
  const ui = useUi.getState();
  ui.expandTree(h.ancestors(id));
  ui.setView("hierarchy");
  ui.select({ kind: "symbol", id }, index.byId.get(id)?.origins[0] ?? null);
}

export function HierarchyView() {
  const { index } = useStructure();
  const h = useHierarchy();
  const { treeExpanded, relations, selection, select, toggleTree, openFlow } = useUi();
  const expanded = useMemo(() => new Set(Object.keys(treeExpanded).filter((k) => treeExpanded[k])), [treeExpanded]);
  const kinds = useMemo(() => kindsOf(relations), [relations]);
  const graph = useMemo(() => (h && index ? hierarchyGraph(h, index, expanded, kinds) : undefined), [h, index, expanded, kinds]);
  // A hidden selection is shown on its nearest visible ancestor.
  const selectedId = useMemo(() => {
    if (selection?.kind !== "symbol" || !graph || !h) return null;
    const shown = new Set(graph.nodes.map((n) => n.id));
    let cur: string | null = selection.id;
    while (cur && !shown.has(cur)) cur = h.parentOf(cur);
    return cur;
  }, [selection, graph, h]);
  return (
    <>
      <GraphCanvas
        key="hierarchy"
        graph={graph}
        layoutKey="hierarchy"
        savePositions={false}
        selectedId={selectedId}
        onNodeClick={(n) => {
          const { item } = n.data as unknown as HierarchyNodeData;
          if (item.symbol) select({ kind: "symbol", id: item.symbol.id }, item.symbol.origins[0] ?? null);
        }}
        onNodeDoubleClick={(n) => {
          const { item, childCount } = n.data as unknown as HierarchyNodeData;
          if (childCount > 0) toggleTree(item.id);
          else if (item.symbol?.has_flow) openFlow(item.symbol.id);
        }}
        emptyText="No declarations found."
      />
      {graph?.truncated && <div className="canvas-note">Showing the first {HIERARCHY_CAP} items. Close some boxes to see the rest.</div>}
      <Legend kinds={kinds} />
    </>
  );
}

const LEGEND: { kind: StructEdgeKind; label: string; color: string; dash?: string }[] = [
  { kind: "Calls", label: "calls", color: "#4fa36b" },
  { kind: "Inherits", label: "extends", color: "#d0d4da" },
  { kind: "Implements", label: "implements", color: "#d0d4da", dash: "6 4" },
  { kind: "References", label: "uses", color: "#5d6b7d", dash: "2 4" },
  { kind: "Imports", label: "imports", color: "#7b8794" },
  { kind: "HttpCalls", label: "HTTP", color: KNOWLEDGE_COLORS.httpcalls },
  { kind: "ResolvesDi", label: "provides (DI)", color: KNOWLEDGE_COLORS.resolvesdi },
  { kind: "Tests", label: "tests", color: KNOWLEDGE_COLORS.tests, dash: "4 3" },
  { kind: "Throws", label: "throws", color: KNOWLEDGE_COLORS.throws },
  { kind: "Mutates", label: "changes", color: KNOWLEDGE_COLORS.mutates },
];

/** Edge colors of the relations shown, plus how confidence is drawn. */
export function Legend({ kinds }: { kinds: ReadonlySet<StructEdgeKind> }) {
  const shown = LEGEND.filter((l) => kinds.has(l.kind));
  return (
    <div className="legend">
      {shown.map((l) => (
        <span key={l.kind} className="legend-item">
          <svg width="22" height="8">
            <line x1="0" y1="4" x2="22" y2="4" stroke={l.color} strokeWidth="2" strokeDasharray={l.dash} />
          </svg>
          {l.label}
        </span>
      ))}
      <span className="legend-item" title="Deduced from a convention or a unique match">
        <svg width="22" height="8">
          <line x1="0" y1="4" x2="22" y2="4" stroke="#c9ced6" strokeWidth="2" strokeDasharray="7 5" />
        </svg>
        inferred
      </span>
      <span className="legend-item" title="The code shows the relation but not its target">
        <svg width="22" height="8">
          <line x1="0" y1="4" x2="22" y2="4" stroke="#e5534b" strokeWidth="2" strokeDasharray="1 5" strokeLinecap="round" />
        </svg>
        unresolved
      </span>
    </div>
  );
}

const KIND_LABEL: Partial<Record<StructEdgeKind, string>> = {
  Calls: "calls",
  Inherits: "extends",
  Implements: "implements",
  Instantiates: "creates",
  References: "uses",
  Imports: "imports",
  Subscribes: "notifies",
  Raises: "raises",
  Compatible: "fits",
  HttpCalls: "HTTP calls",
  ResolvesDi: "DI provides",
  Tests: "tests",
  Throws: "throws",
  CaughtBy: "caught by",
  Mutates: "changes",
};

/** Overview: what the project is, at a glance. */
export function OverviewView() {
  const { index } = useStructure();
  const h = useHierarchy();
  const knowledge = useKnowledge();
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  const { select, openFlow } = useUi();
  const k = knowledge.data;
  const stats = useMemo(() => {
    if (!index) return undefined;
    const byKind = new Map<StructEdgeKind, number>();
    for (const e of index.graph.edges) if (e.kind !== "Contains") byKind.set(e.kind, (byKind.get(e.kind) ?? 0) + 1);
    const inferred = index.graph.edges.filter((e) => e.confidence === "Inferred").length;
    const unresolved = index.graph.edges.filter((e) => e.confidence === "Unresolved").length;
    return {
      symbols: index.graph.symbols.filter((s) => !s.external).length,
      relations: [...byKind.values()].reduce((a, b) => a + b, 0),
      byKind: [...byKind.entries()].sort((a, b) => b[1] - a[1]),
      inferred,
      unresolved,
    };
  }, [index]);
  if (!index || !stats) return <div className="empty">Loading…</div>;

  const name = (id: string) => index.byId.get(id)?.qualified_name ?? id;
  const pick = (id: string) => {
    const s = index.byId.get(id);
    if (h && s) revealInHierarchy(h, index, id);
    else select({ kind: "symbol", id }, s?.origins[0] ?? null);
  };
  const entries = (k?.symbols ?? []).filter((s) => s.entry && !s.test);
  // Test cases only: classes of tests are marked too.
  const tests = (k?.symbols ?? []).filter((s) => s.test && !isTypeKind(index.byId.get(s.symbol)?.kind ?? "Class")).length;
  const hotspots = [...(k?.symbols ?? [])].filter((s) => s.complexity > 1).sort((a, b) => b.complexity - a.complexity).slice(0, 10);
  const maxKind = stats.byKind[0]?.[1] ?? 1;

  return (
    <div className="overview">
      <div className="kpis">
        <Kpi label="files" value={project.data?.files.length ?? 0} />
        <Kpi label="declarations" value={stats.symbols} />
        <Kpi label="relations" value={stats.relations} />
        <Kpi label="entry points" value={entries.length} />
        <Kpi label="routes" value={k?.routes.length ?? 0} />
        <Kpi label="tests" value={tests} />
      </div>
      <div className="ov-grid">
        <section className="ov-card">
          <h3>Relations</h3>
          {stats.byKind.map(([kind, n]) => (
            <div key={kind} className="bar-row">
              <span className="bar-label">{KIND_LABEL[kind] ?? kind}</span>
              <span className="bar">
                <span style={{ width: `${(100 * n) / maxKind}%`, background: KNOWLEDGE_COLORS[kind.toLowerCase()] ?? "#5d6b7d" }} />
              </span>
              <span className="bar-value">{n}</span>
            </div>
          ))}
          {(stats.inferred > 0 || stats.unresolved > 0) && (
            <p className="muted small">
              {stats.inferred} inferred · {stats.unresolved} unresolved. Every other relation is exact.
            </p>
          )}
        </section>
        <section className="ov-card">
          <h3>Entry points</h3>
          {entries.length === 0 && <p className="muted small">No entry points found: no Main, routes, server start or framework hooks.</p>}
          {entries.slice(0, 12).map((s) => (
            <button key={s.symbol} className="ov-row" onClick={() => pick(s.symbol)} title={name(s.symbol)}>
              <span className="badge small">{s.entry!.kind}</span>
              <span className="ov-main">{s.entry!.label}</span>
              <span className="ov-sub">{index.byId.get(s.symbol)?.name}</span>
            </button>
          ))}
          {entries.length > 12 && <p className="muted small">+{entries.length - 12} more</p>}
        </section>
        <section className="ov-card">
          <h3>Routes</h3>
          {(k?.routes.length ?? 0) === 0 && <p className="muted small">No HTTP routes declared.</p>}
          {k?.routes.slice(0, 14).map((r) => (
            <button key={`${r.method} ${r.path} ${r.handler}`} className="ov-row" onClick={() => pick(r.handler)} title={name(r.handler)}>
              <span className={`method method-${r.method.toLowerCase()}`}>{r.method}</span>
              <span className="ov-main mono">{r.path}</span>
              <span className="ov-sub">{r.framework}</span>
            </button>
          ))}
          {(k?.routes.length ?? 0) > 14 && <p className="muted small">+{k!.routes.length - 14} more</p>}
        </section>
        <section className="ov-card">
          <h3>Hotspots</h3>
          <p className="muted small">The functions with the most decisions (cyclomatic complexity).</p>
          {hotspots.map((s) => (
            <button
              key={s.symbol}
              className="ov-row"
              onClick={() => pick(s.symbol)}
              onDoubleClick={() => index.byId.get(s.symbol)?.has_flow && openFlow(s.symbol)}
              title={`${name(s.symbol)}\nDouble-click to open its Blueprint`}
            >
              <span className="cc">{s.complexity}</span>
              <span className="ov-main">{index.byId.get(s.symbol)?.name ?? s.symbol}</span>
              <span className="ov-sub">{s.capabilities.slice(0, 3).join(" · ")}</span>
            </button>
          ))}
        </section>
        {(k?.models.length ?? 0) > 0 && (
          <section className="ov-card">
            <h3>Data models</h3>
            {k!.models.map((m) => (
              <button key={m.symbol} className="ov-row" onClick={() => pick(m.symbol)} title={name(m.symbol)}>
                <span className="ov-main">{index.byId.get(m.symbol)?.name ?? m.symbol}</span>
                <span className="ov-sub">
                  {m.model.framework} · {m.model.fields.length} fields
                </span>
              </button>
            ))}
          </section>
        )}
      </div>
    </div>
  );
}

function Kpi({ label, value }: { label: string; value: number }) {
  return (
    <div className="kpi">
      <span className="kpi-value">{value.toLocaleString()}</span>
      <span className="kpi-label">{label}</span>
    </div>
  );
}
