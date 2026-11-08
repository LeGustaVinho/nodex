import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { api } from "../api";
import { useCatalog } from "../catalog";
import type { Symbol } from "../gen/Symbol";
import { classBlueprintGraph, classMembers, flowNodePrefix, type ClassBoxData, type MethodCardData } from "../graph/classBlueprint";
import type { BlueprintData, ExprData } from "../graph/flowGraph";
import type { StructureIndex } from "../graph/structureGraph";
import { ReplayBar } from "../panels/RunPanel";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { useSymbolInfo } from "./Explorer";
import { GraphCanvas } from "./GraphCanvas";

export function ClassBlueprintView({ index, symbol }: { index: StructureIndex; symbol: Symbol }) {
  const { classOpen, expanded, detail, showDataWires, select, selection, openFlow, setClassOpen, toggleClassMember } = useUi();
  const catalog = useCatalog();
  const symbolInfo = useSymbolInfo();
  const ids = useMemo(() => classMembers(index, symbol.id).map((m) => m.id), [index, symbol.id]);
  const open = useMemo(() => new Set(ids.filter((id) => classOpen[id] ?? ids.length <= 4)), [ids, classOpen]);
  const flows = useQuery({
    queryKey: ["flows", symbol.id, ids],
    queryFn: () => api.flows(ids),
    enabled: ids.length > 0,
  });
  const graph = useMemo(() => (flows.data || !ids.length)
    ? classBlueprintGraph(index, symbol.id, flows.data ?? [], { open, expanded, detail, showDataWires, operations: catalog?.operations, symbolInfo })
    : undefined, [index, symbol.id, ids.length, flows.data, open, expanded, detail, showDataWires, catalog, symbolInfo]);
  const replay = useTrace((s) => s.replay);
  const replayFlow = replay && open.has(replay.flow) ? replay.flow : null;
  const replayNode = replayFlow && replay ? replay.page.steps[replay.index]?.node : undefined;
  const selectionFlow = selection && (selection.kind === "flow-node" || selection.kind === "expr") ? selection.flow : null;
  const activeMember = selection?.kind === "symbol" ? selection.id : selectionFlow;
  useEffect(() => {
    const span = flows.data?.find((f) => f.symbol === replayFlow)?.nodes.find((n) => n.id === replayNode)?.span;
    if (span) useUi.getState().focusCode(span);
  }, [flows.data, replayFlow, replayNode]);
  useEffect(() => {
    const span = symbol.origins[0];
    const ui = useUi.getState();
    if (span && ui.codeFocus?.file !== span.file) ui.focusCode(span);
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
        fitSelection={false}
        selectedId={selectedId}
        loadingText="Loading the class Blueprint…"
        error={flows.error}
        onRetry={() => void flows.refetch()}
        onNodeClick={(n) => {
          if (n.type === "expr") {
            const d = n.data as unknown as ExprData;
            select({ kind: "expr", flow: d.flow, nodeId: d.ownerId, path: d.path }, d.expr.span);
          } else if (n.type === "blueprint") {
            const { flow, node } = n.data as unknown as BlueprintData;
            select({ kind: "flow-node", flow, id: node.id }, node.span);
          } else if (n.type === "methodCard" || n.type === "methodBox" || n.type === "classBox") {
            const { symbol: item } = n.data as unknown as MethodCardData | ClassBoxData;
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
