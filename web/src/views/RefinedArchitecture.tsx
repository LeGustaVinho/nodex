import { useMemo, useState } from "react";
import type { ArchitectureKind } from "../gen/ArchitectureKind";
import type { GGraph } from "../graph/model";
import { saveFile } from "../desktop";
import { useUi } from "../store";
import { errorText, toast } from "../toast";
import type { Refined, RefinedEdge, RefinedNode } from "../uml/refinement";
import type { ArchitectureNodeData } from "../components/ArchitectureNode";
import { exportCanvas, GraphCanvas } from "./GraphCanvas";
import { RefinementEvidence, RefinementSummary } from "./DiagramRefinement";

const kinds = new Set<string>(["Layer", "Module", "System", "Application", "Store", "Component", "External", "Actor"]);

export function refinedProjection(diagram: Refined, focused: string | null, showAll: boolean, limit: number) {
  const byId = new Map(diagram.nodes.map((n) => [n.id, n]));
  const parent = (n: RefinedNode) => {
    const container = n.parent && byId.get(n.parent);
    return container && (container.kind === "System" || container.kind === "Application") ? null : n.parent;
  };
  const pathTo = (id: string | null) => {
    const path = new Set<string>();
    for (let n = id === null ? undefined : byId.get(id); n && !path.has(n.id); n = byId.get(parent(n) ?? "")) {
      path.add(n.id);
    }
    return [...path].reverse();
  };
  const priority = new Set(diagram.suggestions.filter((r) => r.suggestion.kind === "highlight").map((r) => r.suggestion.target));
  const visible = diagram.nodes.filter((n) => parent(n) === focused);
  const list = visible.sort((a, b) => Number(priority.has(b.id)) - Number(priority.has(a.id))).slice(0, limit);
  const ids = new Set(list.map((n) => n.id));
  const ancestors = new Set<string | null>([null, ...pathTo(focused)]);
  ancestors.delete(focused);
  const representative = (id: string) => {
    const visited = new Set<string>();
    for (let n = byId.get(id); n; n = byId.get(parent(n) ?? "")) {
      if (visited.has(n.id)) break;
      visited.add(n.id);
      const boundary = parent(n);
      if (boundary === focused) return ids.has(n.id) ? n.id : undefined;
      if (ancestors.has(boundary)) return n.id;
    }
    return undefined;
  };
  const projected = diagram.edges.flatMap((edge) => {
    const from = representative(edge.from);
    const to = representative(edge.to);
    if (!from || !to || from === to || (!ids.has(from) && !ids.has(to))) return [];
    return [{ edge, from, to }];
  });
  const featured = projected.some(({ edge }) => priority.has(edge.id));
  const outside = new Set<string>();
  const edges = new Map<string, { edge: RefinedEdge; originals: string[] }>();
  for (const { edge: e, from, to } of projected) {
    const crossesBoundary = !ids.has(from) || !ids.has(to);
    if (!showAll && featured && !priority.has(e.id) && !crossesBoundary) continue;
    if (!ids.has(from)) outside.add(from);
    if (!ids.has(to)) outside.add(to);
    const key = JSON.stringify([from, to, e.kind, e.label, e.confidence]);
    const rolled = edges.get(key);
    if (rolled) {
      rolled.originals.push(e.id);
      rolled.edge.count += e.count;
      rolled.edge.evidence.push(...e.evidence);
    } else {
      edges.set(key, { edge: { ...e, id: key, from, to, evidence: [...e.evidence] }, originals: [e.id] });
    }
  }
  return {
    nodes: [...list, ...[...outside].map((id) => byId.get(id)!)], byId, pathTo,
    edges: [...edges.values()], omitted: visible.length - list.length,
    priority, outside, hasChildren: new Set(diagram.nodes.map(parent)), featured,
  };
}

export function RefinedArchitecture({ diagram, historical, follow }: {
  diagram: Refined; historical: boolean; follow: (node: RefinedNode) => boolean;
}) {
  const [path, setPath] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [all, setAll] = useState(false);
  const [limit, setLimit] = useState(60);
  const focused = path.at(-1) ?? null;
  const projection = useMemo(() => refinedProjection(diagram, focused, all, limit), [diagram, focused, all, limit]);
  const show = (next: string[]) => { setPath(next); setSelected(null); setLimit(60); };
  const enter = (node: RefinedNode) => {
    if (projection.hasChildren.has(node.id)) show(projection.pathTo(node.id));
    else if (!historical && follow(node)) setSelected(null);
    else setSelected(node.id);
  };
  const selectedEdge = projection.edges.find((e) => e.edge.id === selected);
  const selectedNode = selected ? projection.byId.get(selected) : undefined;
  const targetIds = selectedEdge?.originals ?? (selected ? [selected] : []);
  const explanations = diagram.suggestions.filter((r) => targetIds.includes(r.suggestion.target));
  const evidence = [...(selectedEdge?.edge.evidence ?? selectedNode?.evidence ?? []), ...explanations.flatMap((r) => r.suggestion.evidence)];
  const graph: GGraph = {
    direction: "RIGHT",
    nodes: projection.nodes.map((n) => ({
      id: n.id, type: "architecture", width: 230, height: 62, ports: [],
      data: {
        node: {
          id: n.id, name: n.label, kind: (kinds.has(n.kind) ? n.kind : "Component") as ArchitectureKind,
          parent: null, provenance: n.declared ? "Declared" : "Inferred",
          technology: projection.outside.has(n.id) ? "Outside group"
            : projection.priority.has(n.id) ? "Highlighted" : n.kind === "Group" ? "Group" : null,
          path: null, layer: n.layer, code: null, evidence: [],
        },
        boundary: projection.hasChildren.has(n.id), vertical: false,
        action: projection.hasChildren.has(n.id) || (!historical && (n.kind === "System" || n.kind === "Application"))
          ? { label: "Open " + n.label, run: () => enter(n) } : undefined,
      } satisfies ArchitectureNodeData,
    })),
    edges: projection.edges.map(({ edge }) => ({
      id: edge.id, source: edge.from, target: edge.to, kind: "architecture", layout: true,
      confidence: edge.confidence, weight: edge.count,
      data: { label: edge.label, http: edge.kind === "HTTP", offset: 0 },
    })),
  };
  const exportMermaid = async () => {
    const ident = new Map(projection.nodes.map((n, i) => [n.id, "n" + i]));
    const safe = (s: string) => s.replace(/[#"<>`\\|;]/g, (c) => "#" + c.charCodeAt(0) + ";").replace(/[\r\n]/g, " ");
    const text = ["flowchart LR", ...projection.nodes.map((n) => `  ${ident.get(n.id)}["${safe(n.label)}"]`),
      ...projection.edges.map(({ edge }) => `  ${ident.get(edge.from)} ${edge.confidence === "Exact" ? "-->" : "-.->"}|"${safe(edge.label)}"| ${ident.get(edge.to)}`)].join("\n");
    try { await saveFile("architecture-refined.mmd", "text/plain", text); }
    catch (error) { toast.error(errorText(error)); }
  };
  return <>
    <div className="uml-bar architecture-toolbar">
      <nav aria-label="Refined diagram hierarchy">
        <button onClick={() => show([])}>Overview</button>
        {path.map((part, index) => <button key={part} onClick={() => show(path.slice(0, index + 1))}>{projection.byId.get(part)?.label}</button>)}
      </nav>
      <span className="spacer" />
      {projection.featured && <button onClick={() => setAll(!all)}>{all ? "Key relationships" : "All relationships"}</button>}
      <button onClick={() => void exportMermaid()}>.mmd</button>
      <button onClick={exportCanvas}>.png</button>
    </div>
    <RefinementSummary diagram={diagram} historical={historical} />
    {projection.omitted > 0 && <p className="muted small"><button onClick={() => setLimit((n) => n + 60)}>Show more</button> {projection.omitted} more elements in this group.</p>}
    <div className="architecture-content">
      <GraphCanvas graph={graph} layoutKey={JSON.stringify(["refined", path])} selectedId={selected}
        onNodeClick={(n) => setSelected(n.id)} onNodeDoubleClick={(n) => { const node = projection.byId.get(n.id); if (node) enter(node); }}
        onEdgeClick={setSelected} onPaneClick={() => setSelected(null)} savePositions={false} renderWholeGraph refit />
    </div>
    {(selected || diagram.pending.length > 0) && <aside className="refinement-inspector" aria-label="Refinement evidence">
      {selectedNode && <strong>{selectedNode.label}</strong>}
      {selectedEdge && <strong>{selectedEdge.edge.label} · {selectedEdge.edge.confidence}</strong>}
      {explanations.map((r) => <p key={r.id}>{r.suggestion.text}</p>)}
      {selectedNode?.code && !historical && <button onClick={() => useUi.getState().showUml(selectedNode.code!, "Class")}>Open code diagram</button>}
      {evidence.length > 0 && <RefinementEvidence diagram={diagram} ids={evidence} historical={historical} />}
      {!selected && diagram.pending.length > 0 && <details><summary>Needs confirmation ({diagram.pending.length})</summary>
        <RefinementEvidence diagram={diagram} ids={diagram.pending} historical={historical} />
      </details>}
    </aside>}
  </>;
}
