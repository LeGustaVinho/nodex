import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  type EdgeTypes,
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
import { CallNode } from "../components/CallNode";
import { ExprNode } from "../components/ExprNode";
import { OwnershipEdge } from "../components/OwnershipEdge";
import { HierarchyGroup, HierarchyNode } from "../components/HierarchyNodes";
import { NamespaceNode, TypeCard } from "../components/StructureNodes";
import { layoutGraph } from "../graph/layout";
import type { GEdge, GGraph } from "../graph/model";

const nodeTypes: NodeTypes = {
  blueprint: BlueprintNode,
  expr: ExprNode,
  namespace: NamespaceNode,
  typeCard: TypeCard,
  callNode: CallNode,
  hnode: HierarchyNode,
  hgroup: HierarchyGroup,
};

const edgeTypes: EdgeTypes = { ownership: OwnershipEdge };

/** Colors of the knowledge relations (also used by legends). */
export const KNOWLEDGE_COLORS: Record<string, string> = {
  httpcalls: "#f0883e",
  resolvesdi: "#39c5bb",
  tests: "#a371f7",
  throws: "#e5534b",
  caughtby: "#e5534b",
  mutates: "#58a6ff",
};

/** Visual style of an edge: its kind, then its confidence and how many relations it stands for. */
export function edgeStyle(e: GEdge): Partial<Edge> {
  const base = kindStyle(e);
  const style = { ...(base.style ?? {}) };
  if (e.weight && e.weight > 1) {
    style.strokeWidth = Math.min(7, Number(style.strokeWidth ?? 1.4) + Math.log2(e.weight) * 0.9);
    base.label = base.label ? `${base.label} ×${e.weight}` : `×${e.weight}`;
  }
  if (e.confidence === "Inferred") {
    style.strokeDasharray = "7 5";
    style.opacity = 0.8;
  } else if (e.confidence === "Unresolved") {
    style.stroke = "#e5534b";
    style.strokeDasharray = "1 5";
    style.strokeLinecap = "round";
    if (base.markerEnd && typeof base.markerEnd === "object") base.markerEnd = { ...base.markerEnd, color: "#e5534b" };
  }
  return { ...base, style };
}

function kindStyle(e: GEdge): Partial<Edge> {
  const ty = (e.data as { ty?: string | null } | undefined)?.ty;
  switch (e.kind) {
    case "exec":
      return { style: { stroke: "#e8e8e8", strokeWidth: 2.4 }, className: "edge-exec" };
    case "data": {
      // Rust: how the value travels (move, copy, borrow) shapes the wire.
      const access = (e.data as { access?: string | null } | undefined)?.access;
      if (access) return { type: "ownership", data: { ty: ty ?? null, access }, className: "edge-data" };
      return { style: { stroke: typeColor(ty), strokeWidth: 1.6, opacity: 0.75 }, className: "edge-data" };
    }
    case "expr":
      return { style: { stroke: "#8fa3b8", strokeWidth: 1.3 } };
    case "inherits":
      return { type: "smoothstep", style: { stroke: "#d0d4da", strokeWidth: 1.8 }, markerEnd: { type: MarkerType.ArrowClosed, color: "#d0d4da" }, label: "extends" };
    case "implements":
      return { type: "smoothstep", style: { stroke: "#d0d4da", strokeWidth: 1.6, strokeDasharray: "6 4" }, markerEnd: { type: MarkerType.ArrowClosed, color: "#d0d4da" }, label: "implements" };
    case "compatible":
      return { type: "smoothstep", style: { stroke: "#a9b4c2", strokeWidth: 1.4, strokeDasharray: "1 5", strokeLinecap: "round" }, markerEnd: { type: MarkerType.ArrowClosed, color: "#a9b4c2" }, label: "fits" };
    case "uses":
      return { type: "smoothstep", style: { stroke: "#5d6b7d", strokeWidth: 1.2, strokeDasharray: "2 4" }, markerEnd: { type: MarkerType.Arrow, color: "#5d6b7d" } };
    case "creates":
      return { type: "smoothstep", style: { stroke: "#4fa36b", strokeWidth: 1.2, strokeDasharray: "2 4" }, markerEnd: { type: MarkerType.Arrow, color: "#4fa36b" }, label: "creates" };
    case "calls":
      return { style: { stroke: "#4fa36b", strokeWidth: 1.8 }, markerEnd: { type: MarkerType.ArrowClosed, color: "#4fa36b" } };
    case "raises":
      return { style: { stroke: "#c2185b", strokeWidth: 1.6, strokeDasharray: "6 4" }, markerEnd: { type: MarkerType.ArrowClosed, color: "#c2185b" }, label: "raises" };
    case "subscribes":
      return { style: { stroke: "#c2185b", strokeWidth: 1.6, strokeDasharray: "2 4" }, markerEnd: { type: MarkerType.ArrowClosed, color: "#c2185b" }, label: "notifies" };
    case "imports":
      return { style: { stroke: "#7b8794", strokeWidth: 1.4 }, markerEnd: { type: MarkerType.ArrowClosed, color: "#7b8794" } };
    case "httpcalls":
    case "resolvesdi":
    case "tests":
    case "throws":
    case "caughtby":
    case "mutates": {
      const color = KNOWLEDGE_COLORS[e.kind];
      const label = { httpcalls: "HTTP", resolvesdi: "provides", tests: "tests", throws: "throws", caughtby: "caught by", mutates: "changes" }[e.kind];
      const dash = e.kind === "caughtby" || e.kind === "tests" ? "4 3" : undefined;
      return { style: { stroke: color, strokeWidth: e.kind === "httpcalls" ? 2.4 : 1.7, strokeDasharray: dash }, markerEnd: { type: MarkerType.ArrowClosed, color }, label };
    }
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
  /** Remember positions the user drags nodes to (off for views whose shape changes often). */
  savePositions?: boolean;
  /** Fit the whole graph after every layout, not only the first (views that grow in place). */
  refit?: boolean;
}

function Canvas({ graph, layoutKey, selectedId, onNodeClick, onNodeDoubleClick, emptyText, fitFirst, savePositions = true, refit = false }: Props) {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [busy, setBusy] = useState(false);
  const { fitView, getInternalNode, getViewport, setCenter } = useReactFlow();
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
    // Wait for React Flow to measure nodes added by this layout (opening a box adds some).
    const fit = () => void fitView({ padding: 0.1, maxZoom: 1.1, minZoom: focus.length ? 0.6 : 0.2, nodes: focus.length ? focus : undefined });
    requestAnimationFrame(() => requestAnimationFrame(fit));
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
        const overrides = savePositions ? (saved.data ?? {}) : {};
        setNodes(
          graph.nodes.map((n) => {
            const laid = positions.get(n.id);
            // Containers take the size their children need; their own position is not saved.
            const sized = n.container && laid?.width ? { width: laid.width, height: laid.height, style: { width: laid.width, height: laid.height } } : { width: n.width };
            return {
              id: n.id,
              type: n.type,
              data: n.data as Record<string, unknown>,
              position: (!n.container && overrides[n.id]) || (laid ? { x: laid.x, y: laid.y } : { x: 0, y: 0 }),
              ...sized,
              ...(n.parent ? { parentId: n.parent } : {}),
              selected: n.id === selectedId,
            };
          }),
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
        if (fittedKey.current !== layoutKey || refit) {
          fittedKey.current = layoutKey;
          // Opened at a node (a debugger stop, a search hit): start there. Wide Blueprints: start
          // readable at the entry instead of shrinking everything.
          if (selectedId && graph.nodes.some((n) => n.id === selectedId)) pendingFit.current = [{ id: selectedId }];
          else pendingFit.current = fitFirst ? graph.nodes.slice(0, fitFirst).map((n) => ({ id: n.id })) : [];
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
    const node = getInternalNode(selectedId);
    const pane = document.querySelector(".canvas .react-flow");
    if (!node || !pane) return;
    const { x, y, zoom } = getViewport();
    const rect = pane.getBoundingClientRect();
    // Nodes inside containers have positions relative to them.
    const abs = node.internals.positionAbsolute;
    const w = (node.measured?.width ?? node.width ?? 200) * zoom;
    const h = (node.measured?.height ?? 80) * zoom;
    const sx = abs.x * zoom + x;
    const sy = abs.y * zoom + y;
    const visible = sx >= 0 && sy >= 0 && sx + w <= rect.width && sy + h <= rect.height;
    if (!visible) {
      void setCenter(abs.x + w / zoom / 2, abs.y + h / zoom / 2, { zoom: Math.max(zoom, 0.6), duration: 300 });
    }
  }, [selectedId, setNodes, getInternalNode, getViewport, setCenter]);

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
        edgeTypes={edgeTypes}
        onNodesChange={onNodesChange}
        onNodeClick={(_, n) => onNodeClick?.(n)}
        onNodeDoubleClick={(_, n) => onNodeDoubleClick?.(n)}
        onNodeDragStop={(_, __, moved) => savePositions && persist(moved)}
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
        {savePositions && Object.keys(saved.data ?? {}).length > 0 && (
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
