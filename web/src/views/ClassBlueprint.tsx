import { useEffect, useMemo, useRef } from "react";
import { useCatalog } from "../catalog";
import { useDebug } from "../debug";
import type { Symbol } from "../gen/Symbol";
import { blueprintNavigationTarget, openNavigationMenu } from "../navigation";
import { classBlueprintGraph, flowNodePrefix, type FieldCardData, type MethodCardData } from "../graph/classBlueprint";
import type { BlueprintData, ExprData } from "../graph/flowGraph";
import type { StructureIndex } from "../graph/structureGraph";
import { ReplayBar } from "../panels/RunPanel";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { useSymbolInfo } from "./Explorer";
import { GraphCanvas } from "./GraphCanvas";
import { useClassFlows } from "./useClassFlows";

export function ClassBlueprintView({ index, symbol }: { index: StructureIndex; symbol: Symbol }) {
  const { classOpen, expanded, detail, showDataWires, select, selection, openFlow, setClassOpen, toggleClassMember } = useUi();
  const catalog = useCatalog();
  const symbolInfo = useSymbolInfo();
  const { ids, query: flows } = useClassFlows(index, symbol.id);
  const replay = useTrace((s) => s.replay);
  const debugFrame = useDebug((s) => s.stopped?.frames.find((f) => f.id === s.frame));
  const atDebugFrame = !!debugFrame?.flow && selection?.kind === "flow-node" &&
    selection.flow === debugFrame.flow && selection.id === debugFrame.node;
  const replayFlow = !atDebugFrame && replay && ids.includes(replay.flow) ? replay.flow : null;
  const replayNode = replayFlow && replay ? replay.page.steps[replay.index]?.node : undefined;
  const open = useMemo(() => new Set(ids.filter((id) => id === replayFlow || (classOpen[id] ?? ids.length <= 4))), [ids, classOpen, replayFlow]);
  const graph = useMemo(() => (flows.data || !ids.length)
    ? classBlueprintGraph(index, symbol.id, flows.data ?? [], { open, expanded, detail, showDataWires, operations: catalog?.operations, symbolInfo })
    : undefined, [index, symbol.id, ids.length, flows.data, open, expanded, detail, showDataWires, catalog, symbolInfo]);
  const selectionFlow = selection && (selection.kind === "flow-node" || selection.kind === "expr") ? selection.flow : null;
  const activeMember = selection?.kind === "symbol" ? selection.id : selectionFlow;
  useEffect(() => {
    const span = flows.data?.find((f) => f.symbol === replayFlow)?.nodes.find((n) => n.id === replayNode)?.span;
    if (span) useUi.getState().focusCode(span);
  }, [flows.data, replayFlow, replayNode]);
  const openedClass = useRef<string | null>(null);
  useEffect(() => {
    const span = symbol.origins[0];
    if (!span || openedClass.current === symbol.id) return;
    openedClass.current = symbol.id;
    const ui = useUi.getState();
    if (!symbol.origins.some((origin) => origin.file === ui.codeFocus?.file)) ui.focusCode(span);
  }, [symbol]);

  const selectedId = replayFlow && replayNode
    ? flowNodePrefix(replayFlow) + replayNode
    : selection?.kind === "flow-node" && open.has(selection.flow)
      ? flowNodePrefix(selection.flow) + selection.id
      : selection?.kind === "expr" && open.has(selection.flow)
        ? `${flowNodePrefix(selection.flow)}${selection.nodeId}/e/${selection.path}`
        : activeMember;
  return (
    <>
      {(replayFlow || (activeMember && ids.includes(activeMember))) && <ReplayBar flow={replayFlow ?? activeMember!} />}
      <GraphCanvas
        graph={graph}
        layoutKey={`class-flow:${symbol.id}:${detail}`}
        savePositions={false}
        refit
        fitSelection={atDebugFrame || !!replayNode}
        selectedId={selectedId}
        loadingText="Loading the class Blueprint…"
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
    </>
  );
}
