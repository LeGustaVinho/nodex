import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  MarkerType,
  MiniMap,
  type Node,
  type NodeTypes,
  ReactFlow,
  ReactFlowProvider,
  useNodesInitialized,
  useNodesState,
  useReactFlow,
} from "@xyflow/react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, type Positions as SavedPositions } from "../api";
import { typeColor } from "../catalog";
import { BlueprintNode } from "../components/BlueprintNode";
import { ExprNode } from "../components/ExprNode";
import { NamespaceNode, TypeCard } from "../components/StructureNodes";
import { layoutGraph } from "../graph/layout";
import type { GEdge, GGraph } from "../graph/model";

const nodeTypes: NodeTypes = {
  blueprint: BlueprintNode,
  expr: ExprNode,
  namespace: NamespaceNode,
  typeCard: TypeCard,
};

/** Visual style of each edge kind. */
export function edgeStyle(e: GEdge): Partial<Edge> {
  const ty = (e.data as { ty?: string | null } | undefined)?.ty;
  switch (e.kind) {
    case "exec":
      return { style: { stroke: "#e8e8e8", strokeWidth: 2.4 }, className: "edge-exec" };
    case "data":
      return { style: { stroke: typeColor(ty), strokeWidth: 1.6, opacity: 0.75 }, className: "edge-data" };
    case "expr":
      return { style: { stroke: "#8fa3b8", strokeWidth: 1.3 } };
    case "inherits":
      return { type: "smoothstep", style: { stroke: "#d0d4da", strokeWidth: 1.8 }, markerEnd: { type: MarkerType.ArrowClosed, color: "#d0d4da" }, label: "extends" };
    case "implements":
      return { type: "smoothstep", style: { stroke: "#d0d4da", strokeWidth: 1.6, strokeDasharray: "6 4" }, markerEnd: { type: MarkerType.ArrowClosed, color: "#d0d4da" }, label: "implements" };
    case "uses":
      return { type: "smoothstep", style: { stroke: "#5d6b7d", strokeWidth: 1.2, strokeDasharray: "2 4" }, markerEnd: { type: MarkerType.Arrow, color: "#5d6b7d" } };
    case "creates":
      return { type: "smoothstep", style: { stroke: "#4fa36b", strokeWidth: 1.2, strokeDasharray: "2 4" }, markerEnd: { type: MarkerType.Arrow, color: "#4fa36b" }, label: "creates" };
    case "imports":
      return { style: { stroke: "#7b8794", strokeWidth: 1.4 }, markerEnd: { type: MarkerType.ArrowClosed, color: "#7b8794" } };
    default:
      return {};
  }
}

interface Props {
  graph: GGraph | undefined;
  /** Key under which manual positions are saved (`flow:<symbol>`, `types:<ns>`, ...). */
  layoutKey: string;
  selectedId: string | null;
  onNodeClick?: (node: Node) => void;
  onNodeDoubleClick?: (node: Node) => void;
  emptyText?: string;
  /** Fit only the first N nodes initially (keeps long flows readable). */
  fitFirst?: number;
}

function Canvas({ graph, layoutKey, selectedId, onNodeClick, onNodeDoubleClick, emptyText, fitFirst }: Props) {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [busy, setBusy] = useState(false);
  const { fitView, getNode, getViewport, setCenter } = useReactFlow();
  const queryClient = useQueryClient();
  const saved = useQuery({ queryKey: ["layout", layoutKey], queryFn: () => api.layout(layoutKey) });
  const manual = useRef<SavedPositions>({});
  const fittedKey = useRef<string | null>(null);
  /** Nodes to fit once React Flow has measured them (empty array = fit everything). */
  const pendingFit = useRef<{ id: string }[] | null>(null);
  const initialized = useNodesInitialized();

  useEffect(() => {
    if (!initialized || !pendingFit.current) return;
    const focus = pendingFit.current;
    pendingFit.current = null;
    void fitView({ padding: 0.1, maxZoom: 1.1, minZoom: focus.length ? 0.6 : 0.2, nodes: focus.length ? focus : undefined });
  }, [initialized, nodes, fitView]);

  useEffect(() => {
    manual.current = { ...(saved.data ?? {}) };
  }, [saved.data]);

  useEffect(() => {
    if (!graph || !saved.isFetched) return;
    let cancelled = false;
    setBusy(true);
    layoutGraph(graph)
      .then((positions) => {
        if (cancelled) return;
        const overrides = saved.data ?? {};
        setNodes(
          graph.nodes.map((n) => ({
            id: n.id,
            type: n.type,
            data: n.data as Record<string, unknown>,
            position: overrides[n.id] ?? positions.get(n.id) ?? { x: 0, y: 0 },
            width: n.width,
            selected: n.id === selectedId,
          })),
        );
        setEdges(
          graph.edges.map((e) => ({
            id: e.id,
            source: e.source,
            target: e.target,
            sourceHandle: e.sourceHandle,
            targetHandle: e.targetHandle,
            ...edgeStyle(e),
          })),
        );
        if (fittedKey.current !== layoutKey) {
          fittedKey.current = layoutKey;
          // Wide Blueprints: start readable at the entry instead of shrinking everything.
          pendingFit.current = fitFirst ? graph.nodes.slice(0, fitFirst).map((n) => ({ id: n.id })) : [];
        }
      })
      .catch((err) => console.error("layout failed", err))
      .finally(() => !cancelled && setBusy(false));
    return () => {
      cancelled = true;
    };
    // selectedId is applied by the effect below; re-layout only when the graph changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [graph, saved.isFetched, layoutKey]);

  useEffect(() => {
    setNodes((ns) => ns.map((n) => (n.selected === (n.id === selectedId) ? n : { ...n, selected: n.id === selectedId })));
    // Bring the selected node into view (for selections made from the code or the sidebar).
    if (!selectedId) return;
    const node = getNode(selectedId);
    const pane = document.querySelector(".canvas .react-flow");
    if (!node || !pane) return;
    const { x, y, zoom } = getViewport();
    const rect = pane.getBoundingClientRect();
    const w = (node.measured?.width ?? node.width ?? 200) * zoom;
    const h = (node.measured?.height ?? 80) * zoom;
    const sx = node.position.x * zoom + x;
    const sy = node.position.y * zoom + y;
    const visible = sx >= 0 && sy >= 0 && sx + w <= rect.width && sy + h <= rect.height;
    if (!visible) {
      void setCenter(node.position.x + w / zoom / 2, node.position.y + h / zoom / 2, { zoom: Math.max(zoom, 0.6), duration: 300 });
    }
  }, [selectedId, setNodes, getNode, getViewport, setCenter]);

  const persist = useCallback(
    (moved: Node[]) => {
      for (const n of moved) manual.current[n.id] = { x: Math.round(n.position.x), y: Math.round(n.position.y) };
      void api.saveLayout(layoutKey, manual.current).then(() => queryClient.setQueryData(["layout", layoutKey], { ...manual.current }));
    },
    [layoutKey, queryClient],
  );

  const resetLayout = async () => {
    manual.current = {};
    await api.saveLayout(layoutKey, {});
    fittedKey.current = null;
    await queryClient.invalidateQueries({ queryKey: ["layout", layoutKey] });
  };

  if (!graph) return <div className="empty">{emptyText ?? "Loading…"}</div>;
  if (graph.nodes.length === 0) return <div className="empty">{emptyText ?? "Nothing to show."}</div>;

  return (
    <div className="canvas">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onNodeClick={(_, n) => onNodeClick?.(n)}
        onNodeDoubleClick={(_, n) => onNodeDoubleClick?.(n)}
        onNodeDragStop={(_, __, moved) => persist(moved)}
        nodesConnectable={false}
        zoomOnDoubleClick={false}
        edgesFocusable={false}
        minZoom={0.1}
        maxZoom={2}
        onlyRenderVisibleElements
        proOptions={{ hideAttribution: true }}
        colorMode="dark"
      >
        <Background variant={BackgroundVariant.Lines} gap={32} color="#23272f" />
        <Controls showInteractive={false} />
        <MiniMap pannable zoomable nodeColor="#3a4150" maskColor="rgba(10,12,16,0.7)" />
      </ReactFlow>
      <div className="canvas-tools">
        {busy && <span className="muted">laying out…</span>}
        {Object.keys(saved.data ?? {}).length > 0 && (
          <button onClick={resetLayout} title="Discard manually moved positions">
            Reset layout
          </button>
        )}
      </div>
    </div>
  );
}

export function GraphCanvas(props: Props) {
  return (
    <ReactFlowProvider>
      <Canvas {...props} />
    </ReactFlowProvider>
  );
}
