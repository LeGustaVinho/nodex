/**
 * The Knowledge view: the relations around one symbol (both directions, a few steps), or, with no
 * focus, the map of every knowledge relation of the project (routes reached over HTTP, DI, tests,
 * throws, writes). Relation groups and confidences filter what is followed.
 */
import type { Confidence } from "../gen/Confidence";
import type { StructEdge } from "../gen/StructEdge";
import type { StructEdgeKind } from "../gen/StructEdgeKind";
import type { Symbol } from "../gen/Symbol";
import { type HItem, type HierarchyNodeData, LEAF_HEIGHT, LEAF_WIDTH, edgeKindOf } from "./hierarchy";
import type { GEdge, GGraph, GNode } from "./model";
import { type StructureIndex, isTypeKind } from "./structureGraph";

export const KNOWLEDGE_KINDS: ReadonlySet<StructEdgeKind> = new Set(["HttpCalls", "ResolvesDi", "Tests", "Throws", "CaughtBy", "Mutates"]);
export const KNOWLEDGE_CAP = 160;

export interface KnowledgeOptions {
  kinds: ReadonlySet<StructEdgeKind>;
  confidences: ReadonlySet<Confidence>;
  depth: number;
}

/** `OrderService.PlaceOrderAsync` for members, the plain name otherwise. */
function labelOf(index: StructureIndex, s: Symbol): string {
  const owner = s.parent ? index.byId.get(s.parent) : undefined;
  return owner && isTypeKind(owner.kind) && !isTypeKind(s.kind) ? `${owner.name}.${s.name}` : s.name;
}

export function knowledgeGraph(index: StructureIndex, focus: string | null, opts: KnowledgeOptions): GGraph & { truncated: boolean } {
  const allowed = (e: StructEdge) => e.kind !== "Contains" && opts.kinds.has(e.kind) && opts.confidences.has(e.confidence ?? "Exact");
  const chosen = new Map<string, StructEdge>();
  const nodes = new Set<string>();
  let truncated = false;
  const keyOf = (e: StructEdge) => `${e.from}->${e.to}:${e.kind}`;

  if (focus && index.byId.has(focus)) {
    const out = new Map<string, StructEdge[]>();
    const into = new Map<string, StructEdge[]>();
    for (const e of index.graph.edges) {
      if (!allowed(e)) continue;
      (out.get(e.from) ?? out.set(e.from, []).get(e.from)!).push(e);
      (into.get(e.to) ?? into.set(e.to, []).get(e.to)!).push(e);
    }
    nodes.add(focus);
    let frontier = [focus];
    // A type stands for its members: their relations are the type's neighbourhood.
    const focused = index.byId.get(focus)!;
    if (isTypeKind(focused.kind)) frontier = frontier.concat(index.members(focus).map((m) => m.id));
    for (const m of frontier) nodes.add(m);
    for (let step = 0; step < opts.depth && frontier.length; step++) {
      const next: string[] = [];
      for (const id of frontier) {
        for (const e of [...(out.get(id) ?? []), ...(into.get(id) ?? [])]) {
          if (chosen.has(keyOf(e))) continue;
          if (nodes.size >= KNOWLEDGE_CAP && !(nodes.has(e.from) && nodes.has(e.to))) {
            truncated = true;
            continue;
          }
          chosen.set(keyOf(e), e);
          for (const end of [e.from, e.to]) {
            if (!nodes.has(end)) {
              nodes.add(end);
              next.push(end);
            }
          }
        }
      }
      frontier = next;
    }
    // Members of a focused type without relations add nothing.
    if (isTypeKind(focused.kind)) {
      const linked = new Set([...chosen.values()].flatMap((e) => [e.from, e.to]));
      for (const m of index.members(focus)) if (!linked.has(m.id)) nodes.delete(m.id);
    }
  } else {
    for (const e of index.graph.edges) {
      if (!KNOWLEDGE_KINDS.has(e.kind) || !allowed(e)) continue;
      if (nodes.size >= KNOWLEDGE_CAP && !(nodes.has(e.from) && nodes.has(e.to))) {
        truncated = true;
        continue;
      }
      chosen.set(keyOf(e), e);
      nodes.add(e.from);
      nodes.add(e.to);
    }
  }

  const gnodes: GNode[] = [...nodes].map((id) => {
    const s = index.byId.get(id);
    const item: HItem = {
      id,
      parent: null,
      kind: s?.kind ?? "Class",
      label: s ? labelOf(index, s) : id,
      title: s ? `${s.qualified_name}${s.external ? " (external)" : ""}` : id,
      symbol: s ?? null,
      children: [],
    };
    return { id, type: "hnode", data: { item, childCount: 0, expanded: false } satisfies HierarchyNodeData, width: LEAF_WIDTH, height: LEAF_HEIGHT, ports: [] };
  });
  const edges: GEdge[] = [...chosen.values()].map((e) => ({
    id: keyOf(e),
    source: e.from,
    target: e.to,
    kind: edgeKindOf(e.kind),
    layout: true,
    confidence: e.confidence ?? null,
    data: e,
  }));
  return { nodes: gnodes, edges, direction: "RIGHT", aspectRatio: 1.6, truncated };
}
