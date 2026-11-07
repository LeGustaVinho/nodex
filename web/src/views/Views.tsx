import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { api } from "../api";
import { callGraph } from "../graph/callGraph";
import { flowToGraph } from "../graph/flowGraph";
import { StructureIndex, architectureGraph, typesGraph } from "../graph/structureGraph";
import { useUi } from "../store";
import { GraphCanvas } from "./GraphCanvas";

export function useStructure() {
  const q = useQuery({ queryKey: ["structure"], queryFn: api.structure });
  const index = useMemo(() => (q.data ? new StructureIndex(q.data) : undefined), [q.data]);
  return { index, error: q.error };
}

export function ArchitectureView() {
  const { index } = useStructure();
  const { setNamespace, setView, select, selection } = useUi();
  const graph = useMemo(() => (index ? architectureGraph(index) : undefined), [index]);
  return (
    <GraphCanvas
      key="architecture"
      graph={graph}
      layoutKey="architecture"
      selectedId={selection?.kind === "symbol" ? selection.id : null}
      onNodeClick={(n) => {
        const s = index?.byId.get(n.id);
        select({ kind: "symbol", id: n.id }, s?.origins[0] ?? null);
      }}
      onNodeDoubleClick={(n) => {
        const s = index?.byId.get(n.id);
        if (s && s.kind === "Namespace" && !s.external) {
          setNamespace(s.qualified_name);
          setView("types");
        }
      }}
      emptyText="No namespaces found."
    />
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
  const { flowSymbol, expanded, showDataWires, select, selection, openFlow } = useUi();
  const flow = useQuery({
    queryKey: ["flow", flowSymbol],
    queryFn: () => api.flow(flowSymbol!),
    enabled: !!flowSymbol,
  });
  const graph = useMemo(
    () => (flow.data ? flowToGraph(flow.data, expanded, showDataWires) : undefined),
    [flow.data, expanded, showDataWires],
  );
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
  const selectedId =
    selection?.kind === "flow-node" ? selection.id : selection?.kind === "expr" ? `${selection.nodeId}/e/${selection.path}` : null;
  return (
    <GraphCanvas
      key={`flow:${flowSymbol}`}
      graph={graph}
      layoutKey={`flow:${flowSymbol}`}
      fitFirst={3}
      selectedId={selectedId}
      onNodeDoubleClick={(n) => {
        // Double-clicking a call jumps into the called function's Blueprint.
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
  );
}
