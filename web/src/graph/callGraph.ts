import type { StructEdgeKind } from "../gen/StructEdgeKind";
import type { Symbol } from "../gen/Symbol";
import type { GEdge, GGraph, GNode } from "./model";
import { type StructureIndex, isTypeKind } from "./structureGraph";

export interface CallNodeData {
  symbol: Symbol;
  owner: string | null;
  focus: boolean;
  /** Callers (negative) or callees (positive) distance from the focus. */
  distance: number;
}

const FLOW_KINDS: StructEdgeKind[] = ["Calls", "Raises", "Subscribes"];
const EDGE_KIND: Partial<Record<StructEdgeKind, string>> = { Calls: "calls", Raises: "raises", Subscribes: "subscribes" };
export const CALL_NODE_WIDTH = 230;
const MAX_NODES = 120;

/**
 * Call graph around `focus`: callers to the left, callees to the right, `depth` steps each way.
 * Events sit in the middle of a chain: `ShipAsync → raises OrderShipped → subscribed by Main`.
 * A type as focus means all its members.
 */
export function callGraph(index: StructureIndex, focus: string, depth: number): GGraph {
  const edges = index.graph.edges.filter((e) => FLOW_KINDS.includes(e.kind));
  const out = new Map<string, typeof edges>();
  const inc = new Map<string, typeof edges>();
  for (const e of edges) {
    (out.get(e.from) ?? out.set(e.from, []).get(e.from)!).push(e);
    (inc.get(e.to) ?? inc.set(e.to, []).get(e.to)!).push(e);
  }

  const focusSymbol = index.byId.get(focus);
  const roots = focusSymbol && isTypeKind(focusSymbol.kind) ? index.members(focus).map((m) => m.id) : [focus];
  const distance = new Map<string, number>(roots.map((r) => [r, 0]));
  const used = new Set<string>();

  const walk = (dir: 1 | -1) => {
    let frontier = [...roots];
    for (let step = 1; step <= depth && frontier.length && distance.size < MAX_NODES; step++) {
      const next: string[] = [];
      for (const id of frontier) {
        for (const e of (dir === 1 ? out.get(id) : inc.get(id)) ?? []) {
          const other = dir === 1 ? e.to : e.from;
          used.add(`${e.from}->${e.to}:${e.kind}`);
          if (!distance.has(other) && distance.size < MAX_NODES) {
            distance.set(other, dir * step);
            next.push(other);
          }
        }
      }
      frontier = next;
    }
  };
  walk(1);
  walk(-1);

  const nodes: GNode[] = [];
  for (const [id, d] of distance) {
    const symbol = index.byId.get(id);
    if (!symbol) continue;
    const owner = symbol.parent ? (index.owningType(symbol.parent)?.name ?? null) : null;
    nodes.push({
      id,
      type: "callNode",
      data: { symbol, owner, focus: roots.includes(id), distance: d } satisfies CallNodeData,
      width: CALL_NODE_WIDTH,
      height: 58,
      ports: [],
    });
  }
  const gedges: GEdge[] = [];
  for (const e of edges) {
    const key = `${e.from}->${e.to}:${e.kind}`;
    if (!used.has(key) || !distance.has(e.from) || !distance.has(e.to) || e.from === e.to) continue;
    // Calls deduced from dependency injection keep their confidence (drawn dashed).
    gedges.push({ id: key, source: e.from, target: e.to, kind: EDGE_KIND[e.kind] ?? "calls", layout: true, confidence: e.confidence ?? null, data: e });
  }
  return { nodes, edges: gedges, direction: "RIGHT" };
}
