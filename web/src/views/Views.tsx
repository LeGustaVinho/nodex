import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { api } from "../api";
import type { StructureGraph } from "../gen/StructureGraph";
import * as perf from "../perf";
import { useBlueprintReplay } from "./useBlueprintReplay";
import { useCatalog } from "../catalog";
import { callGraph } from "../graph/callGraph";
import { flowToGraph, recordedEdgeId } from "../graph/flowGraph";
import { StructureIndex, architectureGraph, isBlueprintContainer, isTypeKind, typesGraph } from "../graph/structureGraph";
import { openMenu } from "../components/ContextMenu";
import { blueprintNavigationTarget, navigationMenuItems, openNavigationMenu } from "../navigation";
import { blueprintMenuItems, contextMenuItems } from "../uml/contextMenu";
import { useShallow } from "zustand/react/shallow";
import { useUi } from "../store";
import { ReplayBar } from "../panels/RunPanel";
import { BlueprintGuide } from "../components/BlueprintGuide";
import { GraphCanvas } from "./GraphCanvas";
import { useContextSet } from "./Explorer";
import { ClassBlueprintView } from "./ClassBlueprint";
import { useBlueprintSources } from "./useBlueprintSources";
import { useReplaySource } from "./useReplaySource";
import { Loading } from "../components/Loading";

/** One index per structure graph, shared by every component that shows it. */
const indexes = new WeakMap<StructureGraph, StructureIndex>();

export function indexOf(graph: StructureGraph): StructureIndex {
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
  return { index, error: q.error, isLoading: q.isLoading, refetch: q.refetch };
}

export function ArchitectureView() {
  const { index, error, refetch } = useStructure();
  const { scope, items } = useContextSet(index);
  const { setNamespace, setView, select, selection } = useUi(
    useShallow((s) => ({ setNamespace: s.setNamespace, setView: s.setView, select: s.select, selection: s.selection })),
  );
  const graph = useMemo(() => (index ? perf.timed("architectureGraph", () => architectureGraph(index, scope ?? undefined)) : undefined), [index, scope]);
  return (
    <>
      <GraphCanvas
        key={JSON.stringify(items)}
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
        loadingText="Loading the project structure…"
        error={error}
        onRetry={() => void refetch()}
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
  const { index, error, refetch } = useStructure();
  const { scope, items } = useContextSet(index);
  const { namespace, showUses, select, selection } = useUi(
    useShallow((s) => ({ namespace: s.namespace, showUses: s.showUses, select: s.select, selection: s.selection })),
  );
  const graph = useMemo(() => (index ? typesGraph(index, namespace, showUses, scope ?? undefined) : undefined), [index, namespace, showUses, scope]);
  const selectedType = selection?.kind === "symbol" ? (index?.owningType(selection.id)?.id ?? selection.id) : null;
  return (
    <GraphCanvas
      key={`types:${namespace ?? "*"}:${JSON.stringify(items)}`}
      graph={graph}
      layoutKey={`types:${namespace ?? "*"}`}
      selectedId={selectedType}
      onNodeClick={(n) => {
        const s = index?.byId.get(n.id);
        select({ kind: "symbol", id: n.id }, s?.origins[0] ?? null);
      }}
      onNodeDoubleClick={(n) => {
        const symbol = index?.byId.get(n.id);
        if (symbol && !symbol.external && isTypeKind(symbol.kind)) useUi.getState().openFlow(symbol.id);
      }}
      onNodeContextMenu={(e, n) => {
        const symbol = index?.byId.get(n.id);
        if (symbol && !symbol.external) openMenu(e, [
          ...navigationMenuItems(symbol, index),
          ...blueprintMenuItems(symbol),
          ...contextMenuItems({ kind: "symbol", id: symbol.id }),
        ]);
      }}
      emptyText="No types in this namespace."
      loadingText="Loading the types…"
      error={error}
      onRetry={() => void refetch()}
    />
  );
}

export function CallsView() {
  const { index, error, refetch } = useStructure();
  const { callFocus, callDepth, flowSymbol, selection, select, openFlow } = useUi(
    useShallow((s) => ({
      callFocus: s.callFocus,
      callDepth: s.callDepth,
      flowSymbol: s.flowSymbol,
      selection: s.selection,
      select: s.select,
      openFlow: s.openFlow,
    })),
  );
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
      onNodeContextMenu={(e, n) => openNavigationMenu(e, index?.byId.get(n.id), index)}
      emptyText="No calls found. Calls appear once semantic analysis is ready."
      loadingText="Loading the calls…"
      error={error}
      onRetry={() => void refetch()}
    />
  );
}

export function BlueprintView() {
  const flow = useUi((s) => s.flowSymbol);
  const { index, isLoading } = useStructure();
  const symbol = flow ? index?.byId.get(flow) : undefined;
  if (flow && isLoading) return <Loading label="Loading the Blueprint…" />;
  if (index && symbol && isBlueprintContainer(symbol)) {
    return <ClassBlueprintView key={symbol.id} index={index} symbol={symbol} />;
  }
  return <FunctionBlueprintView />;
}

function FunctionBlueprintView() {
  const { index } = useStructure();
  const { flowSymbol, expanded, showDataWires, blueprintExplanations, blueprintGuide, detail, select, selection, openFlow } = useUi(
    useShallow((s) => ({
      flowSymbol: s.flowSymbol,
      expanded: s.expanded,
      showDataWires: s.showDataWires,
      blueprintExplanations: s.blueprintExplanations,
      blueprintGuide: s.blueprintGuide,
      detail: s.detail,
      select: s.select,
      selection: s.selection,
      openFlow: s.openFlow,
    })),
  );
  const catalog = useCatalog();
  const flow = useQuery({
    queryKey: ["flow", flowSymbol],
    queryFn: () => api.flow(flowSymbol!),
    enabled: !!flowSymbol,
  });
  const sources = useBlueprintSources(useMemo(() => flow.data ? [flow.data] : undefined, [flow.data]));
  const graph = useMemo(
    () => (flow.data ? flowToGraph(flow.data, expanded, showDataWires, {
      level: detail, explanations: blueprintExplanations, operations: catalog?.operations, source: sources[flow.data.span.file], structure: index,
    }) : undefined),
    [flow.data, expanded, showDataWires, blueprintExplanations, detail, catalog, sources, index],
  );
  // During replay, follow the current step in the code panel.
  const { replay } = useBlueprintReplay(index);
  const replayStep = replay?.page.steps[replay.index];
  const replayNode = replayStep?.node;
  const observedEdge = recordedEdgeId(flow.data, replayStep);
  useReplaySource(replay, replayNode ? flow.data?.nodes.find((n) => n.id === replayNode)?.span : undefined);
  const sourceIntent = useUi((s) => s.blueprintSourceIntent);
  // Focus the source on open, without replacing a later navigation destination on refresh.
  useEffect(() => {
    if (flowSymbol && flow.data?.symbol === flowSymbol) useUi.getState().focusBlueprintSource(flowSymbol, [flow.data.span]);
  }, [flowSymbol, flow.data, sourceIntent]);
  if (!flowSymbol) {
    return <div className="empty">Pick a method in the sidebar or double-click one in the Types view to open its Blueprint.</div>;
  }
  if (flow.error) return <div className="empty">This function is no longer in the project.</div>;
  // During replay the canvas follows the current step.
  const activeNode = selection?.kind === "flow-node" && selection.flow === flowSymbol ? selection.id
    : selection?.kind === "expr" && selection.flow === flowSymbol ? selection.nodeId : undefined;
  const entry = blueprintGuide && !replay ? flow.data?.nodes.find((n) => n.concept === "Entry" || n.concept === "EventEntry")?.id : undefined;
  const expressionId = activeNode && selection?.kind === "expr" ? activeNode + "/e/" + selection.path : undefined;
  const selectedId = replay ? replayNode ?? null
    : expressionId && graph?.nodes.some((n) => n.id === expressionId) ? expressionId : activeNode ?? entry ?? null;
  return (
    <div className="blueprint-workspace">
    <ReplayBar flow={flowSymbol} hasObservedTransition={!!observedEdge} />
    {blueprintGuide && flow.data && (
      <BlueprintGuide key={flowSymbol} flows={[flow.data]} activeFlow={flowSymbol} activeNode={activeNode} replaying={!!replay} sources={sources}
        onStep={(flow, node) => select({ kind: "flow-node", flow, id: node.id }, node.span)} />
    )}
    <GraphCanvas
      key={`flow:${flowSymbol}:${detail}`}
      graph={graph}
      layoutKey={`flow:${flowSymbol}:${detail}`}
      fitFirst={3}
      loadingText="Loading the Blueprint…"
      selectedId={selectedId}
      observedEdge={observedEdge}
      explore={(blueprintGuide || blueprintExplanations) && !replay && (activeNode || entry)
        ? { anchor: activeNode ?? entry ?? null, edgeMode: "all" } : undefined}
      onNodeContextMenu={(e, n) => openNavigationMenu(e, index?.byId.get(blueprintNavigationTarget(n) ?? ""), index)}
      onNodeDoubleClick={(n) => {
        // Double-clicking a call (or a function value with a Blueprint) opens that Blueprint.
        if (n.type === "expr") {
          const symbol = (n.data as { expr: { symbol: string | null } }).expr.symbol;
          if (symbol && index?.byId.get(symbol)?.has_flow) openFlow(symbol);
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
    </div>
  );
}
