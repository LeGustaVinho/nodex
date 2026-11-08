/**
 * Controls shared by the explorer views (Hierarchy and Knowledge), ported from CodeGraph's
 * explorer: the stage bar (breadcrumbs, back, counts, layout, grouping, spacing, fit, export),
 * the filter panel (confidence, edges shown, third-party code, every relation with its count),
 * the hover tooltips, the legend and the keyboard shortcuts (`e`, Backspace; `f` is the canvas').
 */
import { useQuery } from "@tanstack/react-query";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api } from "../api";
import { ICON, kindLabel } from "../components/HierarchyNodes";
import type { Confidence } from "../gen/Confidence";
import type { StructEdge } from "../gen/StructEdge";
import type { StructEdgeKind } from "../gen/StructEdgeKind";
import type { SymbolInfo } from "../gen/SymbolInfo";
import { LAYOUTS, coordsOf, degrees, isVendorPath } from "../graph/explore";
import { type HierarchyNodeData, RELATION_GROUPS, RELATION_LABEL, type RolledEdge } from "../graph/hierarchy";
import type { GEdge, GGraph, GNode } from "../graph/model";
import type { StructureIndex } from "../graph/structureGraph";
import { type EdgeMode, type GroupBy, type Spacing, useUi } from "../store";
import { EDGE_IN_COLOR, EDGE_OUT_COLOR, KNOWLEDGE_COLORS, exportCanvas, fitCanvas, isTyping } from "./GraphCanvas";

const CONFIDENCES: Confidence[] = ["Exact", "Inferred", "Unresolved"];
export const CONFIDENCE_HINT: Record<Confidence, string> = {
  Exact: "Proven by the code or a compiler",
  Inferred: "Deduced from a convention or a unique match",
  Unresolved: "The code shows the relation but not its target",
};
const EDGE_MODES: { id: EdgeMode; label: string; hint: string }[] = [
  { id: "all", label: "All", hint: "Every relation on screen" },
  { id: "selection", label: "Selection", hint: "Only the relations of the selected node, in and out (key E)" },
  { id: "in", label: "Incoming", hint: "Only what points at the selected node" },
  { id: "out", label: "Outgoing", hint: "Only what the selected node points at" },
];
const GROUPS: { id: GroupBy; label: string }[] = [
  { id: "none", label: "No boxes" },
  { id: "class", label: "Class" },
  { id: "file", label: "File › class" },
  { id: "namespace", label: "Namespace › file › class" },
];
const SPACINGS: { id: Spacing; label: string }[] = [
  { id: "compact", label: "Compact" },
  { id: "normal", label: "Normal" },
  { id: "wide", label: "Wide" },
];

/** Colour of a relation in legends, bars and filters. */
export function relationColor(kind: StructEdgeKind): string {
  const k = kind.toLowerCase();
  if (KNOWLEDGE_COLORS[k]) return KNOWLEDGE_COLORS[k];
  return ({ calls: "#4fa36b", raises: "#c2185b", subscribes: "#c2185b", inherits: "#d0d4da", implements: "#d0d4da", compatible: "#a9b4c2", instantiates: "#4fa36b", references: "#5d6b7d", imports: "#7b8794" } as Record<string, string>)[k] ?? "#5d6b7d";
}

// ---------------------------------------------------------------------------- data

/** Per-symbol knowledge (entry point, test, complexity), by symbol id. */
export function useSymbolInfo(): Map<string, SymbolInfo> {
  const k = useQuery({ queryKey: ["knowledge"], queryFn: api.knowledge });
  return useMemo(() => new Map((k.data?.symbols ?? []).map((s) => [s.symbol, s])), [k.data]);
}

/** True for a Unity project (its `Assets/Plugins` and `Packages` are third-party). */
export function useUnity(): boolean {
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  return project.data?.flavors.includes("unity") ?? false;
}

/** How many relations of each kind and confidence the project has. */
export function relationCounts(index: StructureIndex) {
  const kinds = new Map<StructEdgeKind, number>();
  const confidences: Record<Confidence, number> = { Exact: 0, Inferred: 0, Unresolved: 0 };
  for (const e of index.graph.edges) {
    if (e.kind === "Contains") continue;
    kinds.set(e.kind, (kinds.get(e.kind) ?? 0) + 1);
    confidences[e.confidence ?? "Exact"]++;
  }
  return { kinds, confidences };
}

/** Adds what the node tooltips and borders show: entry point, test, third-party, degree. */
export function decorate<G extends GGraph>(g: G, info: Map<string, SymbolInfo>, isVendor: (id: string) => boolean): G {
  const deg = degrees(g.edges);
  return {
    ...g,
    nodes: g.nodes.map((n) => {
      const d = n.data as HierarchyNodeData;
      const id = d.item.symbol?.id ?? n.id;
      const i = info.get(id);
      return { ...n, data: { ...d, entry: i?.entry?.label ?? null, test: i?.test ?? false, vendor: isVendor(n.id), degree: deg.get(n.id) ?? 0 } satisfies HierarchyNodeData };
    }),
  };
}

// ---------------------------------------------------------------------------- tooltips

export function NodeTip({ node, index, dblHint }: { node: GNode; index: StructureIndex; dblHint?: string | null }) {
  const d = node.data as HierarchyNodeData;
  const s = d.item.symbol;
  return (
    <>
      <div className="tip-row">
        <span className="badge small">{kindLabel(d.item.kind)}</span>
        {d.entry && <span className="badge small entry">entry · {d.entry}</span>}
        {d.test && <span className="badge small">test</span>}
        {d.vendor && <span className="badge small">third-party</span>}
        {s?.external && <span className="badge small">external</span>}
      </div>
      <div className="tip-title">{d.item.title}</div>
      {s && !s.external && <div className="tip-mono">{coordsOf(s)}</div>}
      {s?.signature && <div className="tip-mono">{s.signature.slice(0, 160)}</div>}
      {(d.degree ?? 0) > 0 && <div className="tip-text">{d.degree} relation{d.degree === 1 ? "" : "s"} on screen</div>}
      {d.childCount > 0 && (
        <div className="tip-text">
          {d.childCount} inside · {d.expanded ? "double-click to close" : "double-click to open"}
        </div>
      )}
      {!d.childCount && dblHint && <div className="tip-text">{dblHint} · right-click for more</div>}
      {index.byId.get(d.item.id)?.doc && <div className="tip-text">{index.byId.get(d.item.id)!.doc!.slice(0, 200)}</div>}
    </>
  );
}

/** Tooltip of a relation, or of a rolled-up edge (how many links, and of what kinds). */
export function EdgeTip({ edge, label }: { edge: GEdge; label: (id: string) => string }) {
  const data = edge.data as (StructEdge & Partial<RolledEdge>) | undefined;
  if (!data) return null;
  if (data.count !== undefined && data.kinds) {
    const r = data as RolledEdge;
    return (
      <>
        <div className="tip-row">
          <ConfidenceBadge c={r.confidence} />
          <b>
            {r.count} link{r.count === 1 ? "" : "s"}
          </b>
        </div>
        <div className="tip-title">
          {label(r.source)} → {label(r.target)}
        </div>
        <div className="tip-row">
          {(Object.entries(r.kinds) as [StructEdgeKind, number][]).map(([k, n]) => (
            <span key={k} className="rel-tag" style={{ borderColor: relationColor(k) }}>
              {RELATION_LABEL[k] ?? k} · {n}
            </span>
          ))}
        </div>
        {Object.keys(r.confidences).length > 1 && (
          <div className="tip-text">
            {(Object.entries(r.confidences) as [Confidence, number][]).map(([c, n]) => `${n} ${c.toLowerCase()}`).join(" · ")}
          </div>
        )}
      </>
    );
  }
  const e = data as StructEdge;
  return (
    <>
      <div className="tip-row">
        <ConfidenceBadge c={e.confidence ?? "Exact"} />
        <span className="rel-tag" style={{ borderColor: relationColor(e.kind) }}>
          {RELATION_LABEL[e.kind] ?? e.kind}
        </span>
      </div>
      <div className="tip-title">
        {label(e.from)} → {label(e.to)}
      </div>
      {e.evidence && <div className="tip-text">{e.evidence}</div>}
      {e.site && (
        <div className="tip-mono">
          {e.site.file}:{e.site.start_line}
        </div>
      )}
      {!e.confidence && <div className="tip-text">{CONFIDENCE_HINT.Exact}</div>}
    </>
  );
}

export function ConfidenceBadge({ c }: { c: Confidence }) {
  return (
    <span className={`conf conf-${c.toLowerCase()}`} title={CONFIDENCE_HINT[c]}>
      {c.toLowerCase()}
    </span>
  );
}

// ---------------------------------------------------------------------------- stage bar

interface BarProps {
  /** Breadcrumbs (the last one is the current place). */
  crumbs: { id: string | null; label: string; title?: string }[];
  onCrumb: (id: string | null) => void;
  onBack?: () => void;
  canBack?: boolean;
  nodes: number;
  edges: number;
  children?: ReactNode;
}

export function ExplorerBar({ crumbs, onCrumb, onBack, canBack, nodes, edges, children }: BarProps) {
  const spacing = useUi((s) => s.spacing);
  const setSpacing = useUi((s) => s.setSpacing);
  return (
    <div className="stage-bar">
      <div className="crumbs">
        {onBack && (
          <button className="icon-button sm" disabled={!canBack} onClick={onBack} title="Back (Backspace)" aria-label="Back">
            ←
          </button>
        )}
        {crumbs.map((c, i) => (
          <span key={`${c.id ?? "home"}:${i}`} className="crumb-wrap">
            {i > 0 && <span className="crumb-sep">›</span>}
            <button className={`crumb-btn ${i === crumbs.length - 1 ? "current" : ""}`} title={c.title ?? c.label} onClick={() => onCrumb(c.id)}>
              {c.label}
            </button>
          </span>
        ))}
      </div>
      <div className="stage-tools">
        <span className="stage-count">
          {nodes.toLocaleString()} nodes · {edges.toLocaleString()} edges
        </span>
        {children}
        <label className="toggle" title="Room between nodes">
          spacing
          <select value={spacing} onChange={(e) => setSpacing(e.target.value as Spacing)}>
            {SPACINGS.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </select>
        </label>
        <button className="icon-button sm" onClick={fitCanvas} title="Fit the whole graph (F)" aria-label="Fit">
          ⤢
        </button>
        <button className="icon-button sm" onClick={exportCanvas} title="Save the graph as a PNG image" aria-label="Export PNG">
          ⤓
        </button>
      </div>
    </div>
  );
}

/** Layout and grouping of the Knowledge view. */
export function KnowledgeTools() {
  const { layoutMode, setLayoutMode, groupBy, setGroupBy, knowledgeFocus, knowledgeDepth, setKnowledgeDepth } = useUi(
    useShallow((s) => ({
      layoutMode: s.layoutMode,
      setLayoutMode: s.setLayoutMode,
      groupBy: s.groupBy,
      setGroupBy: s.setGroupBy,
      knowledgeFocus: s.knowledgeFocus,
      knowledgeDepth: s.knowledgeDepth,
      setKnowledgeDepth: s.setKnowledgeDepth,
    })),
  );
  return (
    <>
      {knowledgeFocus && (
        <div className="segmented" role="group" aria-label="Depth" title="How many steps of relations to follow">
          <span className="seg-label">depth</span>
          {[1, 2, 3].map((d) => (
            <button key={d} className={knowledgeDepth === d ? "on" : ""} aria-pressed={knowledgeDepth === d} onClick={() => setKnowledgeDepth(d)}>
              {d}
            </button>
          ))}
        </div>
      )}
      <label className="toggle" title={groupBy !== "none" ? "Grouped graphs are laid out in columns" : LAYOUTS.find((l) => l.id === layoutMode)?.hint}>
        layout
        <select value={layoutMode} onChange={(e) => setLayoutMode(e.target.value as typeof layoutMode)}>
          {LAYOUTS.map((l) => (
            <option key={l.id} value={l.id} title={l.hint}>
              {l.label}
            </option>
          ))}
        </select>
      </label>
      <label className="toggle" title="Draw boxes for the class, file or namespace around the nodes">
        group
        <select value={groupBy} onChange={(e) => setGroupBy(e.target.value as GroupBy)}>
          {GROUPS.map((g) => (
            <option key={g.id} value={g.id}>
              {g.label}
            </option>
          ))}
        </select>
      </label>
    </>
  );
}

/** Opening, closing and links of the Hierarchy view. */
export function HierarchyTools({ onExpandLevel }: { onExpandLevel: () => void }) {
  const { collapseTree, treeLinks, setTreeLinks } = useUi(useShallow((s) => ({ collapseTree: s.collapseTree, treeLinks: s.treeLinks, setTreeLinks: s.setTreeLinks })));
  return (
    <>
      <button onClick={onExpandLevel} title="Open one more level in every box">
        Open level
      </button>
      <button onClick={collapseTree} title="Close every box">
        Collapse all
      </button>
      <button className={treeLinks ? "on" : ""} aria-pressed={treeLinks} onClick={() => setTreeLinks(!treeLinks)} title="Show the relations rolled up onto the boxes">
        links
      </button>
    </>
  );
}

// ---------------------------------------------------------------------------- filters

interface FilterProps {
  index: StructureIndex;
  /** Edges of the selected node in the current graph (null = nothing selected). */
  selectionEdges: { in: number; out: number } | null;
}

export function FilterPanel({ index, selectionEdges }: FilterProps) {
  const ui = useUi(
    useShallow((s) => ({
      confidences: s.confidences,
      toggleConfidence: s.toggleConfidence,
      edgeMode: s.edgeMode,
      setEdgeMode: s.setEdgeMode,
      includeVendor: s.includeVendor,
      setIncludeVendor: s.setIncludeVendor,
      relationKinds: s.relationKinds,
      toggleRelations: s.toggleRelations,
    })),
  );
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  const unity = project.data?.flavors.includes("unity") ?? false;
  const vendorFiles = useMemo(() => (project.data?.files ?? []).filter((f) => isVendorPath(f.path, unity)).length, [project.data, unity]);
  const counts = useMemo(() => relationCounts(index), [index]);
  const [open, setOpen] = useState(() => {
    try {
      return window.localStorage.getItem("nodex.filtersOpen") !== "0";
    } catch {
      return true;
    }
  });
  const [groupsOpen, setGroupsOpen] = useState<Record<string, boolean>>({});
  const toggleOpen = () => {
    setOpen(!open);
    try {
      window.localStorage.setItem("nodex.filtersOpen", open ? "0" : "1");
    } catch {
      // Remembered for this visit only.
    }
  };
  if (!open) {
    return (
      <button className="filters-toggle" onClick={toggleOpen} title="Show the filters">
        Filters ▸
      </button>
    );
  }
  const modeCount = (m: EdgeMode) => (!selectionEdges ? null : m === "in" ? selectionEdges.in : m === "out" ? selectionEdges.out : m === "selection" ? selectionEdges.in + selectionEdges.out : null);
  return (
    <aside className="filters">
      <button className="filters-head" onClick={toggleOpen} title="Hide the filters">
        Filters ◂
      </button>
      <div className="section-title">Confidence</div>
      {CONFIDENCES.map((c) => (
        <button key={c} className={`filter ${ui.confidences.includes(c) ? "" : "off"}`} onClick={() => ui.toggleConfidence(c)} title={CONFIDENCE_HINT[c]}>
          <span className="checkbox">{ui.confidences.includes(c) ? "✓" : ""}</span>
          <span className={`line-swatch ${c.toLowerCase()}`} />
          <span className="filter-label">{c.toLowerCase()}</span>
          <span className="filter-count">{counts.confidences[c].toLocaleString()}</span>
        </button>
      ))}
      <div className="section-title">Edges shown</div>
      {EDGE_MODES.map((m) => (
        <button key={m.id} className={`filter ${ui.edgeMode === m.id ? "" : "off"}`} onClick={() => ui.setEdgeMode(m.id)} title={m.hint}>
          <span className={`radio ${ui.edgeMode === m.id ? "on" : ""}`} />
          {m.id === "in" && <span className="line-swatch" style={{ borderColor: EDGE_IN_COLOR }} />}
          {m.id === "out" && <span className="line-swatch" style={{ borderColor: EDGE_OUT_COLOR }} />}
          <span className="filter-label">{m.label}</span>
          {modeCount(m.id) !== null && <span className="filter-count">{modeCount(m.id)}</span>}
        </button>
      ))}
      <div className="section-title">Scope</div>
      <button className={`filter ${ui.includeVendor ? "" : "off"}`} onClick={() => ui.setIncludeVendor(!ui.includeVendor)} title="ThirdParty/, vendor/, External/ folders (and Unity's Assets/Plugins and Packages)">
        <span className="checkbox">{ui.includeVendor ? "✓" : ""}</span>
        <span className="filter-label">third-party code</span>
        <span className="filter-count">{vendorFiles.toLocaleString()} files</span>
      </button>
      <div className="section-title">Relations</div>
      {RELATION_GROUPS.map((g) => {
        const on = g.kinds.filter((k) => ui.relationKinds.includes(k)).length;
        const total = g.kinds.reduce((sum, k) => sum + (counts.kinds.get(k) ?? 0), 0);
        const expanded = groupsOpen[g.id] ?? false;
        return (
          <div key={g.id} className="filter-group">
            <div className="filter-group-head">
              <button className="caret-btn" onClick={() => setGroupsOpen({ ...groupsOpen, [g.id]: !expanded })} aria-label={expanded ? "Hide the relations" : "Show the relations"}>
                {expanded ? "▾" : "▸"}
              </button>
              <button className={`filter ${on ? "" : "off"}`} onClick={() => ui.toggleRelations(g.kinds)} title={`Show or hide every ${g.label} relation`}>
                <span className="checkbox">{on === g.kinds.length ? "✓" : on ? "–" : ""}</span>
                <span className="filter-label">{g.label}</span>
                <span className="filter-count">{total.toLocaleString()}</span>
              </button>
            </div>
            {expanded &&
              g.kinds.map((k) => (
                <button key={k} className={`filter sub ${ui.relationKinds.includes(k) ? "" : "off"}`} onClick={() => ui.toggleRelations([k])}>
                  <span className="checkbox">{ui.relationKinds.includes(k) ? "✓" : ""}</span>
                  <span className="swatch" style={{ background: relationColor(k) }} />
                  <span className="filter-label">{RELATION_LABEL[k] ?? k}</span>
                  <span className="filter-count">{(counts.kinds.get(k) ?? 0).toLocaleString()}</span>
                </button>
              ))}
          </div>
        );
      })}
    </aside>
  );
}

// ---------------------------------------------------------------------------- legend

const EDGE_LEGEND: { kind: StructEdgeKind; label: string; dash?: string }[] = [
  { kind: "Calls", label: "calls" },
  { kind: "Raises", label: "raises", dash: "6 4" },
  { kind: "Subscribes", label: "notifies", dash: "2 4" },
  { kind: "Inherits", label: "extends" },
  { kind: "Implements", label: "implements", dash: "6 4" },
  { kind: "Instantiates", label: "creates", dash: "2 4" },
  { kind: "References", label: "uses", dash: "2 4" },
  { kind: "Imports", label: "imports" },
  { kind: "HttpCalls", label: "HTTP" },
  { kind: "ResolvesDi", label: "provides (DI)" },
  { kind: "DependsOn", label: "depends on (DI)", dash: "4 3" },
  { kind: "Tests", label: "tests", dash: "4 3" },
  { kind: "Throws", label: "throws" },
  { kind: "CaughtBy", label: "caught by", dash: "4 3" },
  { kind: "Mutates", label: "changes" },
];

const Line = ({ color, dash, cap }: { color: string; dash?: string; cap?: boolean }) => (
  <svg width="22" height="8">
    <line x1="0" y1="4" x2="22" y2="4" stroke={color} strokeWidth="2" strokeDasharray={dash} strokeLinecap={cap ? "round" : undefined} />
  </svg>
);

/** Node kinds and relations on screen, how confidence is drawn, the selection's colours. */
export function Legend({ kinds, graph }: { kinds: ReadonlySet<StructEdgeKind>; graph?: GGraph }) {
  const { open, setOpen, edgeMode } = useUi(useShallow((s) => ({ open: s.legendOpen, setOpen: s.setLegendOpen, edgeMode: s.edgeMode })));
  const nodeKinds = useMemo(() => {
    // One entry per label: a declared namespace and a namespace box read the same.
    const seen = new Map<string, string>();
    for (const n of graph?.nodes ?? []) {
      const kind = (n.data as HierarchyNodeData).item.kind;
      if (!seen.has(kindLabel(kind))) seen.set(kindLabel(kind), kind);
    }
    return [...seen.values()].sort((a, b) => kindLabel(a).localeCompare(kindLabel(b)));
  }, [graph]);
  const marks = useMemo(() => {
    const d = (graph?.nodes ?? []).map((n) => n.data as HierarchyNodeData);
    return { entry: d.some((x) => x.entry), vendor: d.some((x) => x.vendor), external: d.some((x) => x.item.symbol?.external) };
  }, [graph]);
  if (!open) {
    return (
      <button className="legend legend-closed" onClick={() => setOpen(true)} title="Show the legend">
        legend ▴
      </button>
    );
  }
  const used = new Set((graph?.edges ?? []).flatMap((e) => {
    const d = e.data as (StructEdge & Partial<RolledEdge>) | undefined;
    return d?.kinds ? (Object.keys(d.kinds) as StructEdgeKind[]) : d?.kind ? [d.kind] : [];
  }));
  return (
    <div className="legend">
      <button className="legend-x" onClick={() => setOpen(false)} title="Hide the legend" aria-label="Hide the legend">
        ▾
      </button>
      {nodeKinds.map((k) => (
        <span key={k} className={`legend-item hkind-${k.toLowerCase()}`}>
          <span className="hnode-icon">{ICON[k] ?? "·"}</span>
          {kindLabel(k)}
        </span>
      ))}
      {marks.entry && (
        <span className="legend-item">
          <span className="legend-box entry" /> entry point
        </span>
      )}
      {marks.vendor && (
        <span className="legend-item">
          <span className="legend-box vendor" /> third-party
        </span>
      )}
      {marks.external && (
        <span className="legend-item">
          <span className="legend-box external" /> outside the project
        </span>
      )}
      <span className="legend-sep" />
      {EDGE_LEGEND.filter((l) => kinds.has(l.kind) && (used.size === 0 || used.has(l.kind))).map((l) => (
        <span key={l.kind} className="legend-item">
          <Line color={relationColor(l.kind)} dash={l.dash} />
          {l.label}
        </span>
      ))}
      <span className="legend-item" title={CONFIDENCE_HINT.Inferred}>
        <Line color="#c9ced6" dash="7 5" />
        inferred
      </span>
      <span className="legend-item" title={CONFIDENCE_HINT.Unresolved}>
        <Line color="#e5534b" dash="1 5" cap />
        unresolved
      </span>
      {edgeMode !== "all" && (
        <>
          <span className="legend-sep" />
          {edgeMode !== "out" && (
            <span className="legend-item">
              <Line color={EDGE_IN_COLOR} /> into the selection
            </span>
          )}
          {edgeMode !== "in" && (
            <span className="legend-item">
              <Line color={EDGE_OUT_COLOR} /> out of the selection
            </span>
          )}
        </>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------- keys

/** `e` switches between all edges and the selection's; Backspace goes back. */
export function useExplorerKeys(onBack?: () => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey || isTyping(e.target) || document.querySelector(".palette-backdrop")) return;
      if (e.key === "e") {
        const ui = useUi.getState();
        ui.setEdgeMode(ui.edgeMode === "all" ? "selection" : "all");
      } else if (e.key === "Backspace" && onBack) {
        e.preventDefault();
        onBack();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack]);
}

/** In and out edges of `anchor` (a box counts with its contents) in a graph. */
export function selectionEdgeCounts(g: GGraph | undefined, anchor: string | null): { in: number; out: number } | null {
  if (!g || !anchor || !g.nodes.some((n) => n.id === anchor)) return null;
  const parent = new Map(g.nodes.map((n) => [n.id, n.parent]));
  const inside = (id: string) => {
    for (let cur: string | undefined = id; cur; cur = parent.get(cur)) if (cur === anchor) return true;
    return false;
  };
  let i = 0;
  let o = 0;
  for (const e of g.edges) {
    if (inside(e.source)) o++;
    else if (inside(e.target)) i++;
  }
  return { in: i, out: o };
}
