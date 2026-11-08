import ELK, { type ElkExtendedEdge, type ElkNode, type ElkPort } from "elkjs/lib/elk-api.js";
import elkWorkerUrl from "elkjs/lib/elk-worker.min.js?url";
import type { GGraph, GNode } from "./model";

// ELK runs in a web worker so large graphs never block the UI.
const elk = new ELK({ workerFactory: () => new Worker(elkWorkerUrl) });

/** Node positions; nodes inside a container are relative to it. Containers also get a size. */
export type Positions = Map<string, { x: number; y: number; width?: number; height?: number }>;

const portKey = (node: string, port: string) => `${node}::${port}`;

/** Room for a container's header and border around its children. */
const CONTAINER_PADDING = "[top=34,left=12,bottom=12,right=12]";

/** Lays out the layout-relevant part of a graph with ELK's layered algorithm. */
export async function layoutGraph(g: GGraph): Promise<Positions> {
  const laidOut = g.nodes.filter((n) => !n.attach);
  const nodeIds = new Set(laidOut.map((n) => n.id));
  const kids = new Map<string | undefined, GNode[]>();
  for (const n of laidOut) {
    const parent = n.parent && nodeIds.has(n.parent) ? n.parent : undefined;
    const list = kids.get(parent) ?? [];
    list.push(n);
    kids.set(parent, list);
  }
  const nested = laidOut.some((n) => n.parent && nodeIds.has(n.parent));

  const toElk = (n: GNode): ElkNode => {
    const children = kids.get(n.id);
    if (n.container && children?.length) {
      return {
        id: n.id,
        children: children.map(toElk),
        layoutOptions: { "elk.padding": CONTAINER_PADDING, "elk.nodeSize.minimum": `(${n.width}, ${n.height})`, "elk.nodeSize.constraints": "MINIMUM_SIZE" },
      };
    }
    // ELK orders FIXED_ORDER ports clockwise: east top→bottom, then west bottom→top.
    const east = n.ports.filter((p) => p.side === "EAST");
    const west = n.ports.filter((p) => p.side === "WEST").reverse();
    const ports: ElkPort[] = [...east, ...west].map((p, i) => ({
      id: portKey(n.id, p.id),
      width: 8,
      height: 8,
      layoutOptions: { "elk.port.side": p.side, "elk.port.index": String(i) },
    }));
    return {
      id: n.id,
      width: n.box?.width ?? n.width,
      height: n.box?.height ?? n.height,
      ports,
      layoutOptions: ports.length ? { "elk.portConstraints": "FIXED_ORDER" } : ({} as Record<string, string>),
    };
  };
  const children = (kids.get(undefined) ?? []).map(toElk);
  const portIds = new Set<string>();
  const collectPorts = (list: ElkNode[]) => {
    for (const c of list) {
      for (const p of c.ports ?? []) portIds.add(p.id);
      collectPorts(c.children ?? []);
    }
  };
  collectPorts(children);
  const edges: ElkExtendedEdge[] = g.edges
    .filter((e) => e.layout && nodeIds.has(e.source) && nodeIds.has(e.target))
    .map((e) => {
      const s = e.sourceHandle ? portKey(e.source, e.sourceHandle) : e.source;
      const t = e.targetHandle ? portKey(e.target, e.targetHandle) : e.target;
      return { id: e.id, sources: [portIds.has(s) ? s : e.source], targets: [portIds.has(t) ? t : e.target] };
    });

  const result = await elk.layout({
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": g.direction,
      "elk.spacing.nodeNode": g.direction === "RIGHT" ? "36" : "48",
      "elk.layered.spacing.nodeNodeBetweenLayers": g.direction === "RIGHT" ? "70" : "80",
      "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES",
      "elk.layered.crossingMinimization.forceNodeModelOrder": "true",
      "elk.layered.nodePlacement.strategy": "BRANDES_KOEPF",
      "elk.separateConnectedComponents": "true",
      "elk.spacing.componentComponent": "60",
      "elk.layered.compaction.postCompaction.strategy": "LEFT",
      ...(nested ? { "elk.hierarchyHandling": "INCLUDE_CHILDREN" } : {}),
    },
    children,
    edges,
  });

  const positions: Positions = new Map();
  const byId = new Map(g.nodes.map((n) => [n.id, n]));
  const read = (list: ElkNode[]) => {
    for (const c of list) {
      const n = byId.get(c.id);
      const offset = n?.box?.offsetX ?? 0;
      const sized = n?.container && c.children?.length;
      positions.set(c.id, { x: (c.x ?? 0) + offset, y: c.y ?? 0, ...(sized ? { width: c.width, height: c.height } : {}) });
      read(c.children ?? []);
    }
  };
  read(result.children ?? []);
  // Attached nodes follow their anchor.
  for (const n of g.nodes) {
    if (!n.attach) continue;
    const anchor = positions.get(n.attach.to);
    if (anchor) positions.set(n.id, { x: anchor.x + n.attach.dx, y: anchor.y + n.attach.dy });
  }
  return positions;
}
