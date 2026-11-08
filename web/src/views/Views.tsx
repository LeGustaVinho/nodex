import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { api } from "../api";
import type { StructureGraph } from "../gen/StructureGraph";
import * as perf from "../perf";
import { useTrace } from "../trace";
import { useCatalog } from "../catalog";
import { callGraph } from "../graph/callGraph";
import { flowToGraph } from "../graph/flowGraph";
import { StructureIndex, architectureGraph, typesGraph } from "../graph/structureGraph";
import { useUi } from "../store";
import { ReplayBar } from "../panels/RunPanel";
import { GraphCanvas } from "./GraphCanvas";

/** One index per structure graph, shared by every component that shows it. */
const indexes = new WeakMap<StructureGraph, StructureIndex>();

function indexOf(graph: StructureGraph): StructureIndex {
  let index = indexes.get(graph);
  if (!index) {
    index = perf.timed("StructureIndex", () => new StructureIndex(graph));
    indexes.set(graph, index);
  }
  return index;
}

export function useStructure() {
  const q = useQuery({ queryKey: ["structure"], queryFn: api.structure });
  const index = useMemo(() => (q.data ? indexOf(q.data) : undefined), [q.data]);
  return { index, error: q.error };
}

export function ArchitectureView() {
  const { index } = useStructure();
  const { setNamespace, setView, select, selection } = useUi();
  const graph = useMemo(() => (index ? perf.timed("architectureGraph", () => architectureGraph(index)) : undefined), [index]);
  return (
    <>
      <GraphCanvas
        key="architecture"
        graph={graph}
        layoutKey="architecture"
        selectedId={selection?.kind === "symbol" ? selection.id : null}
        onNodeClick={(n) => {
          // Groups of namespaces are not symbols.
          const s = index?.byId.get(n.id);
          if (s) select({ kind: "symbol", id: n.id }, s.origins[0] ?? null);
        }}
        onNodeDoubleClick={(n) => {
          const s = index?.byId.get(n.id);
          if (s && (s.kind === "Namespace" || s.kind === "Module") && !s.external) {
            setNamespace(s.qualified_name);
            setView("types");
          }
        }}
        emptyText="No namespaces found."
      />
      {graph && graph.grouped > 0 && (
        <div className="canvas-note">
          {graph.grouped} namespaces and modules, grouped by path. The Hierarchy view opens them one by one.
        </div>
      )}
    </>
  );
}

export function TypesView() {
  const { index } = useStructure();
  const { namespace, showUses, select, selection } = useUi();
  const graph = useMemo(() => (index ? typesGraph(index, namespace, showUses) : undefined), [index, namespace, showUses]);
  const selectedType = selection?.kind === "symbol" ? (index?.owningType(selection.id)?.id ?? selection.id) : null;
  return (
    <GraphCanvas
      key={`types:${namespace ?? "*"}`}
      graph={graph}
      layoutKey={`types:${namespace ?? "*"}`}
      selectedId={selectedType}
      onNodeClick={(n) => {
        const s = index?.byId.get(n.id);
        select({ kind: "symbol", id: n.id }, s?.origins[0] ?? null);
      }}
      emptyText="No types in this namespace."
    />
  );
}

export function CallsView() {
  const { index } = useStructure();
  const { callFocus, callDepth, flowSymbol, selection, select, openFlow } = useUi();
  const focus = callFocus ?? flowSymbol ?? (selection?.kind === "symbol" ? selection.id : null);
  const graph = useMemo(() => (index && focus ? callGraph(index, focus, callDepth) : undefined), [index, focus, callDepth]);
  if (!focus) {
    return <div className="empty">Select a function, then choose “Show calls” in the inspector to see who calls it and what it calls.</div>;
  }
  return (
    <GraphCanvas
      key={`calls:${focus}:${callDepth}`}
      graph={graph}
      layoutKey={`calls:${focus}`}
      selectedId={selection?.kind === "symbol" ? selection.id : null}
      onNodeClick={(n) => {
        const s = index?.byId.get(n.id);
        select({ kind: "symbol", id: n.id }, s?.origins[0] ?? null);
      }}
      onNodeDoubleClick={(n) => {
        if (index?.byId.get(n.id)?.has_flow) openFlow(n.id);
      }}
      emptyText="No calls found. Calls appear once semantic analysis is ready."
    />
  );
}

export function BlueprintView() {
  const { flowSymbol, expanded, showDataWires, detail, select, selection, openFlow } = useUi();
  const catalog = useCatalog();
  const flow = useQuery({
    queryKey: ["flow", flowSymbol],
    queryFn: () => api.flow(flowSymbol!),
    enabled: !!flowSymbol,
  });
  const graph = useMemo(
    () => (flow.data ? flowToGraph(flow.data, expanded, showDataWires, { level: detail, operations: catalog?.operations }) : undefined),
    [flow.data, expanded, showDataWires, detail, catalog],
  );
  // During replay, follow the current step in the code panel.
  const replayNode = useTrace((s) => (s.replay?.flow === flowSymbol ? s.replay.page.steps[s.replay.index]?.node : undefined));
  useEffect(() => {
    const span = replayNode ? flow.data?.nodes.find((n) => n.id === replayNode)?.span : undefined;
    if (span) useUi.getState().focusCode(span);
  }, [replayNode, flow.data]);
  // Show the function's code when a Blueprint opens from a link or the sidebar.
  useEffect(() => {
    const span = flow.data?.span;
    const ui = useUi.getState();
    if (span && ui.codeFocus?.file !== span.file) ui.focusCode(span);
  }, [flow.data?.span]);
  if (!flowSymbol) {
    return <div className="empty">Pick a method in the sidebar or double-click one in the Types view to open its Blueprint.</div>;
  }
  if (flow.error) return <div className="empty">This function is no longer in the project.</div>;
  // During replay the canvas follows the current step.
  const selectedId =
    replayNode ??
    (selection?.kind === "flow-node" ? selection.id : selection?.kind === "expr" ? `${selection.nodeId}/e/${selection.path}` : null);
  return (
    <>
    <ReplayBar flow={flowSymbol} />
    <GraphCanvas
      key={`flow:${flowSymbol}:${detail}`}
      graph={graph}
      layoutKey={`flow:${flowSymbol}:${detail}`}
      fitFirst={3}
      selectedId={selectedId}
      onNodeDoubleClick={(n) => {
        // Double-clicking a call (or a function value with a Blueprint) opens that Blueprint.
        if (n.type === "expr") {
          const symbol = (n.data as { expr: { symbol: string | null } }).expr.symbol;
          if (symbol) openFlow(symbol);
          return;
        }
        const target = flow.data?.nodes.find((x) => x.id === n.id)?.calls.find((c) => c.symbol);
        if (target?.symbol) openFlow(target.symbol);
      }}
      onNodeClick={(n) => {
        if (!flow.data) return;
        if (n.type === "expr") {
          const d = n.data as { ownerId: string; path: string; expr: { span: import("../gen/Span").Span } };
          select({ kind: "expr", flow: flow.data.symbol, nodeId: d.ownerId, path: d.path }, d.expr.span);
        } else {
          const node = flow.data.nodes.find((x) => x.id === n.id);
          select({ kind: "flow-node", flow: flow.data.symbol, id: n.id }, node?.span ?? null);
        }
      }}
    />
    </>
  );
}
