import { useEffect, useMemo } from "react";
import { useCatalog } from "../catalog";
import type { Symbol } from "../gen/Symbol";
import { blueprintNavigationTarget, openNavigationMenu } from "../navigation";
import { classBlueprintGraph, flowNodePrefix, type FieldCardData, type MethodCardData } from "../graph/classBlueprint";
import { recordedEdgeId, type BlueprintData, type ExprData } from "../graph/flowGraph";
import { isTypeKind, type StructureIndex } from "../graph/structureGraph";
import { ReplayBar } from "../panels/RunPanel";
import { BlueprintGuide } from "../components/BlueprintGuide";
import { useUi } from "../store";
import { useSymbolInfo } from "./Explorer";
import { GraphCanvas } from "./GraphCanvas";
import { useClassFlows } from "./useClassFlows";
import { useBlueprintSources } from "./useBlueprintSources";
import { useReplaySource } from "./useReplaySource";
import { useBlueprintReplay } from "./useBlueprintReplay";

export function ClassBlueprintView({ index, symbol }: { index: StructureIndex; symbol: Symbol }) {
  const { classOpen, expanded, detail, showDataWires, blueprintExplanations, blueprintGuide, select, selection, openFlow, setClassOpen, toggleClassMember } = useUi();
  const catalog = useCatalog();
  const symbolInfo = useSymbolInfo();
  const { ids, query: flows } = useClassFlows(index, symbol.id);
  const { replay, atDebugFrame } = useBlueprintReplay(index);
  const replayFlow = replay?.flow ?? null;
  const replayStep = replay?.page.steps[replay.index];
  const replayNode = replayStep?.node;
  const observedEdge = replayFlow ? recordedEdgeId(flows.data?.find((f) => f.symbol === replayFlow), replayStep, flowNodePrefix(replayFlow)) : undefined;
  const open = useMemo(() => new Set(ids.filter((id) => id === replayFlow || (classOpen[id] ?? ids.length <= 4))), [ids, classOpen, replayFlow]);
  const sources = useBlueprintSources(flows.data, open);
  const graph = useMemo(() => (flows.data || !ids.length)
    ? classBlueprintGraph(index, symbol.id, flows.data ?? [], { open, expanded, detail, showDataWires, explanations: blueprintExplanations, operations: catalog?.operations, symbolInfo, sources })
    : undefined, [index, symbol.id, ids.length, flows.data, open, expanded, detail, showDataWires, blueprintExplanations, catalog, symbolInfo, sources]);
  const selectionFlow = selection && (selection.kind === "flow-node" || selection.kind === "expr") ? selection.flow : null;
  const activeMember = selection?.kind === "symbol" ? selection.id : selectionFlow;
  useReplaySource(replayFlow ? replay : null, flows.data?.find((f) => f.symbol === replayFlow)?.nodes.find((n) => n.id === replayNode)?.span);
  const sourceIntent = useUi((s) => s.blueprintSourceIntent);
  useEffect(() => {
    useUi.getState().focusBlueprintSource(symbol.id, symbol.origins);
  }, [symbol, sourceIntent]);

  const expressionOwner = selection?.kind === "expr" && open.has(selection.flow)
    ? flowNodePrefix(selection.flow) + selection.nodeId : undefined;
  const expressionId = expressionOwner && selection?.kind === "expr" ? expressionOwner + "/e/" + selection.path : undefined;
  const selectedId = replayFlow
    ? replayNode ? flowNodePrefix(replayFlow) + replayNode : null
    : selection?.kind === "flow-node" && open.has(selection.flow)
      ? flowNodePrefix(selection.flow) + selection.id
      : expressionId && graph?.nodes.some((n) => n.id === expressionId) ? expressionId : expressionOwner ?? activeMember;
  return (
    <div className="blueprint-workspace">
      {(replayFlow || (activeMember && ids.includes(activeMember))) && <ReplayBar flow={replayFlow ?? activeMember!} hasObservedTransition={!!observedEdge} />}
      {blueprintGuide && (flows.data || !ids.length) && (
        <BlueprintGuide key={symbol.id} flows={flows.data ?? []} activeFlow={activeMember}
          activeNode={selection?.kind === "flow-node" ? selection.id : selection?.kind === "expr" ? selection.nodeId : undefined}
          replaying={!!replayFlow} sources={sources} onStep={(flow, node) => {
            setClassOpen([flow], true);
            select({ kind: "flow-node", flow, id: node.id }, node.span);
          }} />
      )}
      <GraphCanvas
        graph={graph}
        layoutKey={`class-flow:${symbol.id}:${detail}`}
        savePositions={false}
        refit
        fitSelection={atDebugFrame || !!replayNode || blueprintGuide}
        selectedId={selectedId}
        observedEdge={observedEdge}
        explore={(blueprintGuide || blueprintExplanations) && !replayFlow && selectedId && selectedId !== symbol.id
          ? { anchor: selectedId, edgeMode: "all" } : undefined}
        loadingText={symbol.kind === "Module" ? "Loading the file Blueprint…" : "Loading the class Blueprint…"}
        error={flows.error}
        onRetry={() => void flows.refetch()}
        onNodeContextMenu={(e, n) => openNavigationMenu(e, index.byId.get(blueprintNavigationTarget(n) ?? n.id), index)}
        onNodeClick={(n) => {
          if (n.type === "expr") {
            const d = n.data as unknown as ExprData;
            select({ kind: "expr", flow: d.flow, nodeId: d.ownerId, path: d.path }, d.expr.span);
          } else if (n.type === "blueprint") {
            const { flow, node } = n.data as unknown as BlueprintData;
            select({ kind: "flow-node", flow, id: node.id }, node.span);
          } else if (n.type === "methodCard" || n.type === "methodBox" || n.type === "classBox" || n.type === "fieldCard") {
            const { symbol: item } = n.data as unknown as FieldCardData;
            select({ kind: "symbol", id: item.id }, item.origins[0] ?? null);
          }
        }}
        onNodeDoubleClick={(n) => {
          if (n.type === "fieldCard") {
            const { symbol: item } = n.data as unknown as FieldCardData;
            if (isTypeKind(item.kind)) {
              select({ kind: "symbol", id: item.id }, item.origins[0] ?? null);
              openFlow(item.id);
            }
            return;
          }
          if (n.type === "methodCard" || n.type === "methodBox") {
            const d = n.data as unknown as MethodCardData;
            toggleClassMember(d.symbol.id, d.open);
            return;
          }
          const targets = n.type === "expr" ? [(n.data as unknown as ExprData).expr.symbol]
            : n.type === "blueprint" ? (n.data as unknown as BlueprintData).node.calls.map((c) => c.symbol) : [];
          const target = targets.find((id) => id && ids.includes(id)) ?? targets.find(Boolean);
          if (!target) return;
          if (ids.includes(target)) {
            setClassOpen([target], true);
          } else if (index.byId.get(target)?.has_flow) {
            openFlow(target);
          }
        }}
      />
    </div>
  );
}
