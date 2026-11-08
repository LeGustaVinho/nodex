import { useQuery } from "@tanstack/react-query";
import { type ReactNode, useCallback, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api } from "../api";
import { openMenu } from "../components/ContextMenu";
import { Loading } from "../components/Loading";
import type { Confidence } from "../gen/Confidence";
import type { StructEdgeKind } from "../gen/StructEdgeKind";
import { SPACING_FACTOR, coordsOf, isVendorPath, isVendorSymbol, resolveLayout, shortName, unresolvedEdges } from "../graph/explore";
import { HIERARCHY_CAP, type HierarchyNodeData, RELATION_LABEL, hierarchyGraph } from "../graph/hierarchy";
import { KNOWLEDGE_CAP, groupGraph, knowledgeGraph } from "../graph/knowledgeGraph";
import type { GGraph } from "../graph/model";
import { isTypeKind } from "../graph/structureGraph";
import * as perf from "../perf";
import { type OverviewList, useUi } from "../store";
import { useToasts } from "../toast";
import { umlMenuItems } from "../uml/menu";
import { UML_KINDS } from "../uml/uml";
import {
  CONFIDENCE_HINT,
  ConfidenceBadge,
  EdgeTip,
  ExplorerBar,
  FilterPanel,
  HierarchyTools,
  KnowledgeTools,
  Legend,
  NodeTip,
  decorate,
  relationColor,
  relationCounts,
  selectionEdgeCounts,
  useExplorerKeys,
  useSymbolInfo,
  useUnity,
} from "./Explorer";
import { GraphCanvas } from "./GraphCanvas";
import { openSubtree, revealInHierarchy, useHierarchy, useKnowledge } from "./hierarchyAccess";
import { useStructure } from "./Views";

/** Edges left on screen once the edge mode has hidden the others. */
function shownEdgeCount(graph: GGraph | undefined, anchor: string | null, mode: string): number {
  if (!graph) return 0;
  if (mode === "all") return graph.edges.length;
  const c = selectionEdgeCounts(graph, anchor);
  if (!c) return 0;
  return mode === "in" ? c.in : mode === "out" ? c.out : c.in + c.out;
}

export function HierarchyView() {
  const { index, error, refetch } = useStructure();
  const h = useHierarchy();
  const info = useSymbolInfo();
  const unity = useUnity();
  const ui = useUi(
    useShallow((s) => ({
      treeExpanded: s.treeExpanded,
      treeAnchor: s.treeAnchor,
      treeLinks: s.treeLinks,
      relationKinds: s.relationKinds,
      confidences: s.confidences,
      edgeMode: s.edgeMode,
      includeVendor: s.includeVendor,
      spacing: s.spacing,
      selection: s.selection,
      select: s.select,
      toggleTree: s.toggleTree,
      expandTree: s.expandTree,
      collapseTree: s.collapseTree,
      openFlow: s.openFlow,
    })),
  );
  const expanded = useMemo(() => new Set(Object.keys(ui.treeExpanded).filter((k) => ui.treeExpanded[k])), [ui.treeExpanded]);
  const kinds = useMemo(() => new Set(ui.relationKinds), [ui.relationKinds]);
  const vendor = useMemo(() => h?.vendorItems((p) => isVendorPath(p, unity)) ?? new Set<string>(), [h, unity]);
  const hidden = ui.includeVendor ? undefined : vendor;
  const graph = useMemo(
    () =>
      h && index
        ? decorate(
            hierarchyGraph(h, index, expanded, kinds, {
              confidences: new Set(ui.confidences as Confidence[]),
              hidden,
              links: ui.treeLinks,
              spacing: SPACING_FACTOR[ui.spacing],
            }),
            info,
            (id) => vendor.has(id),
          )
        : undefined,
    [h, index, expanded, kinds, ui.confidences, hidden, ui.treeLinks, ui.spacing, info, vendor],
  );
  // A hidden selection is shown on its nearest visible ancestor.
  const selectedItem = ui.selection?.kind === "symbol" || ui.selection?.kind === "container" ? ui.selection.id : null;
  const selectedId = useMemo(() => {
    if (!selectedItem || !graph || !h) return null;
    const shown = new Set(graph.nodes.map((n) => n.id));
    let cur: string | null = selectedItem;
    while (cur && !shown.has(cur)) cur = h.parentOf(cur);
    return cur;
  }, [selectedItem, graph, h]);
  // A click on the empty canvas turns the highlight off until the next selection.
  const [cleared, setCleared] = useState<string | null>(null);
  const anchor = selectedId && selectedId !== cleared ? selectedId : null;

  const selectItem = useCallback(
    (id: string) => {
      const item = h?.get(id);
      if (!item) return;
      setCleared(null);
      if (item.symbol) ui.select({ kind: "symbol", id: item.symbol.id }, item.symbol.origins[0] ?? null);
      else ui.select({ kind: "container", id }, null);
    },
    [h, ui],
  );
  const expandLevel = () => {
    if (!h) return;
    const { open, capped } = h.nextLevel(expanded, HIERARCHY_CAP, hidden);
    if (open.length) ui.expandTree(open);
    if (capped) useToasts.getState().push("info", `Some boxes stay closed: the view shows at most ${HIERARCHY_CAP} items.`);
  };
  useExplorerKeys();

  const crumbs = [
    { id: null, label: "Hierarchy", title: "Close every box" },
    ...(selectedItem && h ? [...h.ancestors(selectedItem).reverse(), selectedItem].map((id) => ({ id, label: h.get(id)?.label ?? id, title: h.get(id)?.title })) : []),
  ];
  const label = (id: string) => h?.get(id)?.label ?? id;

  return (
    <div className="explorer">
      <ExplorerBar
        crumbs={crumbs}
        onCrumb={(id) => (id ? selectItem(id) : ui.collapseTree())}
        nodes={graph?.nodes.length ?? 0}
        edges={shownEdgeCount(graph, anchor, ui.edgeMode)}
      >
        <HierarchyTools onExpandLevel={expandLevel} />
      </ExplorerBar>
      <div className="explorer-stage">
        <GraphCanvas
          key="hierarchy"
          graph={graph}
          layoutKey="hierarchy"
          savePositions={false}
          refit
          anchorId={ui.treeAnchor}
          selectedId={selectedId}
          explore={{ anchor, edgeMode: ui.edgeMode }}
          onPaneClick={() => setCleared(selectedId)}
          nodeTip={(n) => {
            const d = n.data as HierarchyNodeData;
            return <NodeTip node={n} index={index!} dblHint={d.item.symbol?.has_flow ? "Double-click to open its Blueprint" : null} />;
          }}
          edgeTip={(e) => <EdgeTip edge={e} label={label} />}
          exportName="hierarchy"
          onNodeClick={(n) => selectItem(n.id)}
          onNodeDoubleClick={(n) => {
            const { item, childCount } = n.data as unknown as HierarchyNodeData;
            if (childCount > 0) ui.toggleTree(item.id);
            else if (item.symbol?.has_flow) ui.openFlow(item.symbol.id);
          }}
          onNodeContextMenu={(e, n) => {
            const { item } = n.data as unknown as HierarchyNodeData;
            // Folder (`dir:`) and namespace (`grp:ns:`) boxes are scopes too.
            const id = item.symbol?.id ?? item.id;
            if (item.symbol?.external) return;
            openMenu(e, [
              ...umlMenuItems({ kind: "Symbol", id }, `UML of ${item.symbol?.name ?? id.replace(/^(dir:|grp:ns:)/, "")}`),
              ...(item.symbol ? [{ label: "Show in Knowledge", run: () => useUi.getState().showKnowledge(item.symbol!.id, item.label) }] : []),
              ...(item.symbol?.has_flow ? [{ label: "Open Blueprint", run: () => ui.openFlow(item.symbol!.id) }] : []),
              ...(h && h.shownChildren(item.id, hidden).length > 0
                ? [{ label: "Open everything inside", run: () => openSubtree(h, item.id, expanded, hidden) }]
                : []),
            ]);
          }}
          emptyText="No declarations found."
          loadingText="Loading the project structure…"
          error={error}
          onRetry={() => void refetch()}
        />
        {index && <FilterPanel index={index} selectionEdges={selectionEdgeCounts(graph, anchor)} />}
        {graph?.truncated && <div className="canvas-note">Showing the first {HIERARCHY_CAP} items. Close some boxes to see the rest.</div>}
        {ui.edgeMode !== "all" && !anchor && (graph?.edges.length ?? 0) > 0 && <div className="canvas-note hint">Select a node to see its relations.</div>}
        <Legend kinds={kinds} graph={graph} />
      </div>
    </div>
  );
}

/** Relations around one symbol, or every knowledge relation of the project. */
export function KnowledgeView() {
  const { index, error, refetch } = useStructure();
  const h = useHierarchy();
  const info = useSymbolInfo();
  const unity = useUnity();
  const ui = useUi(
    useShallow((s) => ({
      knowledgeFocus: s.knowledgeFocus,
      knowledgeDepth: s.knowledgeDepth,
      knowledgeHistory: s.knowledgeHistory,
      relationKinds: s.relationKinds,
      confidences: s.confidences,
      edgeMode: s.edgeMode,
      layoutMode: s.layoutMode,
      groupBy: s.groupBy,
      spacing: s.spacing,
      includeVendor: s.includeVendor,
      selection: s.selection,
      select: s.select,
      showKnowledge: s.showKnowledge,
      knowledgeBack: s.knowledgeBack,
      knowledgeCrumb: s.knowledgeCrumb,
    })),
  );
  const focus = ui.knowledgeFocus;
  const kinds = useMemo(() => new Set(ui.relationKinds), [ui.relationKinds]);
  const graph = useMemo(() => {
    if (!index) return undefined;
    const vendor = (id: string) => {
      const s = index.byId.get(id);
      return !!s && !s.external && isVendorSymbol(s, unity);
    };
    const base = knowledgeGraph(index, focus, {
      kinds,
      confidences: new Set(ui.confidences as Confidence[]),
      depth: ui.knowledgeDepth,
      hidden: ui.includeVendor ? undefined : vendor,
    });
    const grouped = groupGraph(base, index, ui.groupBy);
    const nested = grouped.nodes.some((n) => n.parent);
    const laid = { ...grouped, layout: resolveLayout(ui.layoutMode, focus, nested), focus, spacing: SPACING_FACTOR[ui.spacing] };
    return decorate(laid, info, vendor);
  }, [index, focus, kinds, ui.confidences, ui.knowledgeDepth, ui.includeVendor, ui.groupBy, ui.layoutMode, ui.spacing, info, unity]);
  const selectedId = ui.selection?.kind === "symbol" ? ui.selection.id : focus;
  const [cleared, setCleared] = useState<string | null>(null);
  const anchor = selectedId && selectedId !== cleared ? selectedId : null;
  const back = ui.knowledgeHistory.length > 0 ? ui.knowledgeBack : undefined;
  useExplorerKeys(back);

  const empty = focus
    ? "Nothing related with the relations shown. Turn on more relations or confidences in the filters."
    : "No knowledge relations yet: routes reached over HTTP, dependency injection, tests, throws and data writes appear here.";
  const crumbs = [
    { id: null, label: "Whole project", title: "Every knowledge relation of the project" },
    ...ui.knowledgeHistory.slice(-4).map((h) => ({ id: h.id, label: h.label, title: index?.byId.get(h.id)?.qualified_name })),
  ];
  const label = (id: string) => (index ? shortName(index, id) : id);
  const center = (id: string) => index && ui.showKnowledge(id, shortName(index, id));

  return (
    <div className="explorer">
      <ExplorerBar
        crumbs={crumbs}
        onCrumb={ui.knowledgeCrumb}
        onBack={ui.knowledgeBack}
        canBack={ui.knowledgeHistory.length > 0}
        nodes={graph?.nodes.length ?? 0}
        edges={shownEdgeCount(graph, anchor, ui.edgeMode)}
      >
        <KnowledgeTools />
      </ExplorerBar>
      <div className="explorer-stage">
        <GraphCanvas
          key={`knowledge:${focus ?? "*"}`}
          graph={graph}
          layoutKey={`knowledge:${focus ?? "*"}`}
          selectedId={selectedId}
          explore={{ anchor, edgeMode: ui.edgeMode }}
          onPaneClick={() => setCleared(selectedId)}
          nodeTip={(n) => <NodeTip node={n} index={index!} dblHint={(n.data as HierarchyNodeData).item.symbol?.external ? null : "Double-click to center on it"} />}
          edgeTip={(e) => <EdgeTip edge={e} label={label} />}
          exportName={`knowledge-${focus ? label(focus) : "project"}`}
          onNodeClick={(n) => {
            const { item } = n.data as unknown as HierarchyNodeData;
            setCleared(null);
            if (item.symbol) ui.select({ kind: "symbol", id: item.symbol.id }, item.symbol.origins[0] ?? null);
          }}
          onNodeDoubleClick={(n) => {
            const { item } = n.data as unknown as HierarchyNodeData;
            if (item.symbol && !item.symbol.external) center(item.symbol.id);
          }}
          onNodeContextMenu={(e, n) => {
            const { item } = n.data as unknown as HierarchyNodeData;
            if (!item.symbol || item.symbol.external) return;
            const id = item.symbol.id;
            openMenu(e, [
              { label: "Center on it", run: () => center(id) },
              ...(h && index ? [{ label: "Show in Hierarchy", run: () => revealInHierarchy(h, index, id) }] : []),
              ...(item.symbol.has_flow ? [{ label: "Open Blueprint", run: () => useUi.getState().openFlow(id) }] : []),
              ...umlMenuItems({ kind: "Symbol", id }, `UML of ${item.symbol.name}`),
            ]);
          }}
          emptyText={empty}
          loadingText="Loading the knowledge graph…"
          error={error}
          onRetry={() => void refetch()}
        />
        {index && <FilterPanel index={index} selectionEdges={selectionEdgeCounts(graph, anchor)} />}
        {graph?.truncated && <div className="canvas-note">Showing the first {KNOWLEDGE_CAP} symbols. Focus on one or lower the depth.</div>}
        {ui.edgeMode !== "all" && !anchor && (graph?.edges.length ?? 0) > 0 && <div className="canvas-note hint">Select a node to see its relations.</div>}
        <Legend kinds={kinds} graph={graph} />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------- overview

/** Most rows a full list draws (the filter narrows the rest). */
const LIST_CAP = 500;

const LANGUAGE_COLORS: Record<string, string> = { csharp: "#8b5cf6", typescript: "#3b82f6", javascript: "#eab308", rust: "#f97316" };
const CONFIDENCE_COLORS: Record<Confidence, string> = { Exact: "#8fa3b8", Inferred: "#c9ced6", Unresolved: "#e5534b" };

const LISTS: { id: OverviewList; label: string }[] = [
  { id: "summary", label: "Summary" },
  { id: "entries", label: "Entry points" },
  { id: "routes", label: "Routes" },
  { id: "hotspots", label: "Hotspots" },
  { id: "tests", label: "Tests" },
  { id: "models", label: "Data models" },
  { id: "unresolved", label: "Unresolved" },
];

interface Row {
  key: string;
  /** Text the filter searches. */
  text: string;
  node: ReactNode;
}

/** Overview: what the project is, at a glance, and every list in full. */
export function OverviewView() {
  const { index } = useStructure();
  const h = useHierarchy();
  const knowledge = useKnowledge();
  const unity = useUnity();
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  const diagrams = useQuery({ queryKey: ["uml-availability", JSON.stringify({ kind: "Project" })], queryFn: () => api.umlAvailability({ kind: "Project" }), retry: false });
  const { select, openFlow, focusCode, overviewList, setOverviewList, includeVendor } = useUi(
    useShallow((s) => ({ select: s.select, openFlow: s.openFlow, focusCode: s.focusCode, overviewList: s.overviewList, setOverviewList: s.setOverviewList, includeVendor: s.includeVendor })),
  );
  const [filter, setFilter] = useState("");
  const k = knowledge.data;

  const data = useMemo(() => {
    if (!index) return undefined;
    const vendor = (id: string) => {
      const s = index.byId.get(id);
      return !includeVendor && !!s && !s.external && isVendorSymbol(s, unity);
    };
    const counts = relationCounts(index);
    const symbols = (k?.symbols ?? []).filter((s) => !vendor(s.symbol));
    const isCase = (id: string) => {
      const kind = index.byId.get(id)?.kind ?? "Class";
      return !isTypeKind(kind) && kind !== "Module" && kind !== "Namespace";
    };
    return {
      counts,
      relations: [...counts.kinds.values()].reduce((a, b) => a + b, 0),
      byKind: [...counts.kinds.entries()].sort((a, b) => b[1] - a[1]),
      declarations: index.graph.symbols.filter((s) => !s.external).length,
      entries: symbols.filter((s) => s.entry && !s.test),
      tests: symbols.filter((s) => s.test && isCase(s.symbol)),
      hotspots: symbols.filter((s) => s.complexity > 1).sort((a, b) => b.complexity - a.complexity),
      routes: (k?.routes ?? []).filter((r) => !vendor(r.handler)),
      models: (k?.models ?? []).filter((m) => !vendor(m.symbol)),
      unresolved: unresolvedEdges(index).filter((e) => !vendor(e.from)),
    };
  }, [index, k, includeVendor, unity]);
  if (!index || !data) return <Loading label="Loading the project overview…" />;
  perf.firstView("overview");

  const name = (id: string) => index.byId.get(id)?.qualified_name ?? id;
  const pick = (id: string) => {
    const s = index.byId.get(id);
    if (h && s && !s.external) revealInHierarchy(h, index, id);
    else select({ kind: "symbol", id }, s?.origins[0] ?? null);
  };
  const files = project.data?.files ?? [];
  const languages = [...files.reduce((m, f) => m.set(f.language, (m.get(f.language) ?? 0) + 1), new Map<string, number>()).entries()].sort((a, b) => b[1] - a[1]);
  const maxKind = data.byKind[0]?.[1] ?? 1;
  const maxLang = languages[0]?.[1] ?? 1;
  const totalRel = Math.max(1, data.relations);
  const countOf: Record<OverviewList, number> = {
    summary: 0,
    entries: data.entries.length,
    routes: data.routes.length,
    hotspots: data.hotspots.length,
    tests: data.tests.length,
    models: data.models.length,
    unresolved: data.unresolved.length,
  };
  const viewAll = (list: OverviewList) =>
    countOf[list] > 0 && (
      <button className="link-btn" onClick={() => setOverviewList(list)}>
        view all {countOf[list].toLocaleString()}
      </button>
    );

  const symbolRow = (id: string, badge: ReactNode, sub: ReactNode, extra?: string): Row => ({
    key: id + (extra ?? ""),
    text: `${name(id)} ${extra ?? ""} ${typeof sub === "string" ? sub : ""}`,
    node: (
      <button className="ov-row" onClick={() => pick(id)} onDoubleClick={() => index.byId.get(id)?.has_flow && openFlow(id)} title={`${name(id)}\nDouble-click to open its Blueprint`}>
        {badge}
        <span className="ov-main">{shortName(index, id)}</span>
        <span className="ov-sub">{sub}</span>
      </button>
    ),
  });
  const rowsOf = (list: OverviewList): Row[] => {
    switch (list) {
      case "entries":
        return data.entries.map((s) => symbolRow(s.symbol, <span className="badge small">{s.entry!.kind}</span>, s.entry!.label, s.entry!.label));
      case "routes":
        return data.routes.map((r) => ({
          key: `${r.method} ${r.path} ${r.handler}`,
          text: `${r.method} ${r.path} ${r.framework} ${name(r.handler)}`,
          node: (
            <button className="ov-row" onClick={() => pick(r.handler)} title={name(r.handler)}>
              <span className={`method method-${r.method.toLowerCase()}`}>{r.method}</span>
              <span className="ov-main mono">{r.path}</span>
              <span className="ov-sub">
                {shortName(index, r.handler)} · {r.framework}
              </span>
            </button>
          ),
        }));
      case "hotspots":
        return data.hotspots.map((s) => symbolRow(s.symbol, <span className="cc">{s.complexity}</span>, s.capabilities.slice(0, 3).join(" · ")));
      case "tests":
        return data.tests.map((s) => {
          const subjects = index.graph.edges.filter((e) => e.kind === "Tests" && e.from === s.symbol).map((e) => shortName(index, e.to));
          return symbolRow(s.symbol, <span className="badge small">test</span>, subjects.length ? `tests ${subjects.slice(0, 3).join(", ")}` : coordsOf(index.byId.get(s.symbol)!));
        });
      case "models":
        return data.models.map((m) => symbolRow(m.symbol, <span className="badge small">{m.model.framework}</span>, `${m.model.fields.length} fields${m.model.table ? ` · ${m.model.table}` : ""}`));
      case "unresolved":
        return data.unresolved.map((e, i) => ({
          key: `${e.from}>${e.to}:${e.kind}:${i}`,
          text: `${name(e.from)} ${e.to} ${e.kind} ${e.evidence ?? ""}`,
          node: (
            <div className="ov-row unresolved-row">
              <span className="rel-tag" style={{ borderColor: relationColor(e.kind) }}>
                {RELATION_LABEL[e.kind] ?? e.kind}
              </span>
              <span className="ov-main">
                <button className="link" onClick={() => pick(e.from)}>
                  {shortName(index, e.from)}
                </button>{" "}
                → <span className="muted">? {e.to.replace(/^ext:/, "")}</span>
                {e.evidence && <span className="ov-evidence">{e.evidence}</span>}
              </span>
              {e.site && (
                <button className="link muted ov-sub" onClick={() => focusCode(e.site!)}>
                  {e.site.file}:{e.site.start_line}
                </button>
              )}
            </div>
          ),
        }));
      default:
        return [];
    }
  };

  const nav = (
    <nav className="ov-nav">
      {LISTS.map((l) => (
        <button key={l.id} className={overviewList === l.id ? "on" : ""} onClick={() => setOverviewList(l.id)}>
          {l.label}
          {l.id !== "summary" && <span className="count">{countOf[l.id].toLocaleString()}</span>}
        </button>
      ))}
    </nav>
  );

  if (overviewList !== "summary") {
    const q = filter.trim().toLowerCase();
    const all = rowsOf(overviewList);
    const rows = q ? all.filter((r) => r.text.toLowerCase().includes(q)) : all;
    return (
      <div className="overview">
        {nav}
        <div className="ov-list">
          <div className="ov-list-head">
            <h2>{LISTS.find((l) => l.id === overviewList)?.label}</h2>
            <span className="muted">
              {rows.length.toLocaleString()}
              {q ? ` of ${all.length.toLocaleString()}` : ""}
            </span>
            <input className="search" placeholder="Filter…" value={filter} onChange={(e) => setFilter(e.target.value)} autoFocus />
          </div>
          {rows.length === 0 && <p className="muted small">Nothing here{q ? " matches the filter" : ""}.</p>}
          {rows.slice(0, LIST_CAP).map((r) => (
            <div key={r.key}>{r.node}</div>
          ))}
          {rows.length > LIST_CAP && <p className="muted small">+{(rows.length - LIST_CAP).toLocaleString()} more: type in the filter to narrow the list.</p>}
        </div>
      </div>
    );
  }

  const top = (list: OverviewList, n: number) => rowsOf(list).slice(0, n).map((r) => <div key={r.key}>{r.node}</div>);
  const conf = data.counts.confidences;
  return (
    <div className="overview">
      {nav}
      <div className="kpis">
        <Kpi label="files" value={files.length} />
        <Kpi label="declarations" value={data.declarations} />
        <Kpi label="relations" value={data.relations} />
        <Kpi label="entry points" value={data.entries.length} onClick={() => setOverviewList("entries")} />
        <Kpi label="routes" value={data.routes.length} onClick={() => setOverviewList("routes")} />
        <Kpi label="tests" value={data.tests.length} onClick={() => setOverviewList("tests")} />
      </div>
      <div className="ov-grid">
        <section className="ov-card">
          <h3>Confidence</h3>
          <div className="stack">
            {(["Exact", "Inferred", "Unresolved"] as Confidence[]).map((c) => (
              <span key={c} style={{ width: `${(100 * conf[c]) / totalRel}%`, background: CONFIDENCE_COLORS[c] }} title={`${conf[c]} ${c.toLowerCase()}`} />
            ))}
          </div>
          {(["Exact", "Inferred", "Unresolved"] as Confidence[]).map((c) => (
            <div key={c} className="stack-legend">
              <ConfidenceBadge c={c} />
              <span className="ov-main muted small">{CONFIDENCE_HINT[c]}</span>
              <span className="bar-value">
                {conf[c].toLocaleString()} · {Math.round((100 * conf[c]) / totalRel)}%
              </span>
            </div>
          ))}
          {conf.Unresolved > 0 && viewAll("unresolved")}
        </section>
        <section className="ov-card">
          <h3>Relations</h3>
          {data.byKind.map(([kind, n]) => (
            <div key={kind} className="bar-row">
              <span className="bar-label">{RELATION_LABEL[kind as StructEdgeKind] ?? kind}</span>
              <span className="bar">
                <span style={{ width: `${(100 * n) / maxKind}%`, background: relationColor(kind) }} />
              </span>
              <span className="bar-value">{n.toLocaleString()}</span>
            </div>
          ))}
        </section>
        <section className="ov-card">
          <h3>Languages</h3>
          {languages.map(([lang, n]) => (
            <div key={lang} className="bar-row">
              <span className="bar-label">{lang}</span>
              <span className="bar">
                <span style={{ width: `${(100 * n) / maxLang}%`, background: LANGUAGE_COLORS[lang] ?? "#5d6b7d" }} />
              </span>
              <span className="bar-value">{n.toLocaleString()}</span>
            </div>
          ))}
          <p className="muted small">Files per language.</p>
        </section>
        <section className="ov-card">
          <h3>
            Entry points <span className="count">{data.entries.length.toLocaleString()}</span>
            {viewAll("entries")}
          </h3>
          {data.entries.length === 0 && <p className="muted small">No entry points found: no Main, routes, commands, jobs, message consumers, UI handlers, server start or framework hooks.</p>}
          {top("entries", 12)}
        </section>
        <section className="ov-card">
          <h3>
            Routes <span className="count">{data.routes.length.toLocaleString()}</span>
            {viewAll("routes")}
          </h3>
          {data.routes.length === 0 && <p className="muted small">No HTTP routes declared.</p>}
          {top("routes", 14)}
        </section>
        <section className="ov-card">
          <h3>
            Hotspots <span className="count">{data.hotspots.length.toLocaleString()}</span>
            {viewAll("hotspots")}
          </h3>
          <p className="muted small">The functions with the most decisions (cyclomatic complexity).</p>
          {top("hotspots", 10)}
        </section>
        {data.models.length > 0 && (
          <section className="ov-card">
            <h3>
              Data models <span className="count">{data.models.length.toLocaleString()}</span>
              {viewAll("models")}
            </h3>
            {top("models", 10)}
          </section>
        )}
        {data.unresolved.length > 0 && (
          <section className="ov-card">
            <h3>
              Unresolved <span className="count">{data.unresolved.length.toLocaleString()}</span>
              {viewAll("unresolved")}
            </h3>
            <p className="muted small">Relations the code shows whose target Nodex could not find.</p>
            {top("unresolved", 6)}
          </section>
        )}
        <section className="ov-card">
          <h3>Diagrams</h3>
          <p className="muted small">UML views of the whole project. Right-click a folder, a file or a symbol for its own.</p>
          {UML_KINDS.map((kind) => {
            const a = diagrams.data?.kinds.find((x) => x.kind === kind.id);
            const suggested = diagrams.data?.suggested === kind.id;
            return (
              <button key={kind.id} className="ov-row" disabled={!a?.available} onClick={() => useUi.getState().showUml({ kind: "Project" }, kind.id)} title={kind.question}>
                <span className={`badge small ${suggested ? "suggested" : ""}`}>{suggested ? "suggested" : "UML"}</span>
                <span className="ov-main">{kind.label}</span>
                <span className="ov-sub">{a?.reason ?? kind.question}</span>
              </button>
            );
          })}
        </section>
        {(k?.state_machines.length ?? 0) > 0 && (
          <section className="ov-card">
            <h3>State machines</h3>
            {k!.state_machines.map((m) => (
              <button key={`${m.owner}:${m.name}`} className="ov-row" onClick={() => pick(m.owner)} title={name(m.owner)}>
                <span className="ov-main">{m.name}</span>
                <span className="ov-sub">
                  {m.framework} · {m.states.length} states · {m.transitions.length} transitions
                </span>
              </button>
            ))}
          </section>
        )}
      </div>
    </div>
  );
}

function Kpi({ label, value, onClick }: { label: string; value: number; onClick?: () => void }) {
  const body = (
    <>
      <span className="kpi-value">{value.toLocaleString()}</span>
      <span className="kpi-label">{label}</span>
    </>
  );
  return onClick ? (
    <button className="kpi clickable" onClick={onClick} title={`Every ${label}`}>
      {body}
    </button>
  ) : (
    <div className="kpi">{body}</div>
  );
}
