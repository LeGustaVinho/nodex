import ELK, { type ElkExtendedEdge, type ElkNode, type ElkPort } from "elkjs/lib/elk-api.js";
import elkWorkerUrl from "elkjs/lib/elk-worker.min.js?url";
import type { GGraph, GNode } from "./model";

// ELK runs in a web worker so large graphs never block the UI. The worker starts with the first
// layout (not when the page loads), and a layout that is no longer wanted is stopped with its
// own worker instead of running on in the background.
let shared: InstanceType<typeof ELK> | null = null;
const newElk = () => new ELK({ workerFactory: () => new Worker(elkWorkerUrl) });

/**
 * Keeping the model order (statements in source order) makes ELK's crossing minimization
 * explode on dense graphs: 300 import edges took 3 s, 800 took 73 s. Blueprints are sparse and
 * keep it; dense graphs (namespaces, calls) are laid out freely.
 */
const keepsModelOrder = (nodes: number, edges: number) => edges <= Math.max(150, 1.5 * nodes);

/** Node positions; nodes inside a container are relative to it. Containers also get a size. */
export type Positions = Map<string, { x: number; y: number; width?: number; height?: number }>;

const portKey = (node: string, port: string) => `${node}::${port}`;

/** Room for a container's header and border around its children. */
const CONTAINER_PADDING = "[top=34,left=12,bottom=12,right=12]";

/**
 * Lays out the layout-relevant part of a graph with ELK's layered algorithm. Aborting `signal`
 * stops the layout (its worker is terminated) and rejects with an `AbortError`.
 */
export async function layoutGraph(g: GGraph, signal?: AbortSignal): Promise<Positions> {
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

  // A cancellable layout gets a worker of its own, so stopping it leaves other layouts running.
  const elk = signal ? newElk() : (shared ??= newElk());
  const ordered = keepsModelOrder(laidOut.length, edges.length);
  const aborted = new Promise<never>((_, reject) => {
    signal?.addEventListener("abort", () => {
      elk.terminateWorker();
      reject(new DOMException("layout cancelled", "AbortError"));
    });
  });
  aborted.catch(() => {}); // Aborting after the layout finished is not an error.
  const layout = elk.layout({
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": g.direction,
      "elk.spacing.nodeNode": g.direction === "RIGHT" ? "36" : "48",
      "elk.layered.spacing.nodeNodeBetweenLayers": g.direction === "RIGHT" ? "70" : "80",
      ...(ordered
        ? { "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES", "elk.layered.crossingMinimization.forceNodeModelOrder": "true" }
        : {}),
      "elk.layered.nodePlacement.strategy": "BRANDES_KOEPF",
      "elk.separateConnectedComponents": "true",
      "elk.spacing.componentComponent": "60",
      "elk.layered.compaction.postCompaction.strategy": "LEFT",
      ...(nested ? { "elk.hierarchyHandling": "INCLUDE_CHILDREN" } : {}),
    },
    children,
    edges,
  });
  let result: ElkNode;
  try {
    result = await (signal ? Promise.race([layout, aborted]) : layout);
  } finally {
    if (signal) elk.terminateWorker();
  }

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
  if (g.aspectRatio) packComponents(g, positions, g.aspectRatio);
  // Attached nodes follow their anchor.
  for (const n of g.nodes) {
    if (!n.attach) continue;
    const anchor = positions.get(n.attach.to);
    if (anchor) positions.set(n.id, { x: anchor.x + n.attach.dx, y: anchor.y + n.attach.dy });
  }
  return positions;
}

/**
 * ELK stacks unconnected parts in one column; this re-arranges them in rows so the drawing comes
 * close to `ratio` (width / height). Parts are groups of top-level nodes linked by edges (an edge
 * between nodes inside two containers links the containers).
 */
export function packComponents(g: GGraph, positions: Positions, ratio: number, gap = 60) {
  const byId = new Map(g.nodes.map((n) => [n.id, n]));
  const rootOf = (id: string): string => {
    let cur = byId.get(id);
    while (cur?.parent && byId.has(cur.parent)) cur = byId.get(cur.parent);
    return cur?.id ?? id;
  };
  const roots = g.nodes.filter((n) => !n.attach && !(n.parent && byId.has(n.parent))).map((n) => n.id);
  const group = new Map(roots.map((r) => [r, r]));
  const find = (x: string): string => {
    let r = x;
    while (group.get(r) !== r) r = group.get(r)!;
    group.set(x, r);
    return r;
  };
  for (const e of g.edges) {
    const a = rootOf(e.source);
    const b = rootOf(e.target);
    if (group.has(a) && group.has(b)) group.set(find(a), find(b));
  }
  const parts = new Map<string, string[]>();
  for (const r of roots) {
    const key = find(r);
    parts.set(key, [...(parts.get(key) ?? []), r]);
  }
  if (parts.size < 2) return;
  const boxes = [...parts.values()].map((members) => {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const id of members) {
      const p = positions.get(id);
      const n = byId.get(id)!;
      if (!p) continue;
      x0 = Math.min(x0, p.x);
      y0 = Math.min(y0, p.y);
      x1 = Math.max(x1, p.x + (p.width ?? n.width));
      y1 = Math.max(y1, p.y + (p.height ?? n.height));
    }
    return { members, x0, y0, w: x1 - x0, h: y1 - y0 };
  });
  const area = boxes.reduce((sum, b) => sum + (b.w + gap) * (b.h + gap), 0);
  const rowWidth = Math.max(Math.sqrt(area * ratio), ...boxes.map((b) => b.w));
  let x = 0, y = 0, rowHeight = 0;
  for (const b of boxes) {
    if (x > 0 && x + b.w > rowWidth) {
      x = 0;
      y += rowHeight + gap;
      rowHeight = 0;
    }
    const dx = x - b.x0;
    const dy = y - b.y0;
    for (const id of b.members) {
      const p = positions.get(id);
      if (p) positions.set(id, { ...p, x: p.x + dx, y: p.y + dy });
    }
    x += b.w + gap;
    rowHeight = Math.max(rowHeight, b.h);
  }
}
