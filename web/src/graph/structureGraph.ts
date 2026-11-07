import type { StructureGraph } from "../gen/StructureGraph";
import type { Symbol } from "../gen/Symbol";
import type { SymbolKind } from "../gen/SymbolKind";
import { type GEdge, type GGraph, type GNode } from "./model";

const TYPE_KINDS: SymbolKind[] = ["Class", "Struct", "Interface", "Trait", "Enum", "Union", "Record", "Delegate"];
export const isTypeKind = (k: SymbolKind) => TYPE_KINDS.includes(k);

/** Indexes over a structure graph, shared by views. */
export class StructureIndex {
  byId = new Map<string, Symbol>();
  children = new Map<string, Symbol[]>();

  constructor(public graph: StructureGraph) {
    for (const s of graph.symbols) this.byId.set(s.id, s);
    for (const s of graph.symbols) {
      if (!s.parent) continue;
      const list = this.children.get(s.parent) ?? [];
      list.push(s);
      this.children.set(s.parent, list);
    }
  }

  /** The nearest enclosing type (or the symbol itself if it is a type). */
  owningType(id: string): Symbol | undefined {
    let cur = this.byId.get(id);
    while (cur && !isTypeKind(cur.kind)) cur = cur.parent ? this.byId.get(cur.parent) : undefined;
    return cur;
  }

  namespaceOf(id: string): string | null {
    let cur = this.byId.get(id);
    while (cur && cur.kind !== "Namespace") cur = cur.parent ? this.byId.get(cur.parent) : undefined;
    return cur ? cur.qualified_name : null;
  }

  /** Members (non-type children) of a type, in declaration order. */
  members(typeId: string): Symbol[] {
    return (this.children.get(typeId) ?? []).filter((s) => !isTypeKind(s.kind));
  }

  /** Types declared in a namespace, including nested types. */
  typesIn(ns: string | null): Symbol[] {
    return this.graph.symbols.filter((s) => isTypeKind(s.kind) && !s.external && (ns === null || this.namespaceOf(s.id) === ns));
  }
}

export interface NamespaceData {
  symbol: Symbol;
  types: number;
  external: boolean;
}

/** Architecture view: namespaces, external namespaces and packages, with import edges. */
export function architectureGraph(index: StructureIndex): GGraph {
  const nodes: GNode[] = [];
  const edges: GEdge[] = [];
  const included = new Set<string>();
  for (const s of index.graph.symbols) {
    if (s.kind !== "Namespace" && s.kind !== "Package") continue;
    const types = s.kind === "Namespace" ? index.typesIn(s.qualified_name).length : 0;
    nodes.push({
      id: s.id,
      type: "namespace",
      data: { symbol: s, types, external: s.external } satisfies NamespaceData,
      width: 220,
      height: s.external ? 48 : 64,
      ports: [],
    });
    included.add(s.id);
  }
  for (const e of index.graph.edges) {
    if (e.kind !== "Imports" || !included.has(e.from) || !included.has(e.to)) continue;
    edges.push({ id: `${e.from}->${e.to}`, source: e.from, target: e.to, kind: "imports", layout: true });
  }
  return { nodes, edges, direction: "RIGHT" };
}

export interface TypeCardData {
  symbol: Symbol;
  members: Symbol[];
  namespace: string | null;
}

const MAX_MEMBERS = 14;

export function typeCardHeight(memberCount: number): number {
  const shown = Math.min(memberCount, MAX_MEMBERS) + (memberCount > MAX_MEMBERS ? 1 : 0);
  return 52 + shown * 20 + (memberCount === 0 ? 0 : 8);
}

export { MAX_MEMBERS };

/** Types view: type cards with inheritance, implementation and (optionally) usage edges. */
export function typesGraph(index: StructureIndex, namespace: string | null, showUses: boolean): GGraph {
  const nodes: GNode[] = [];
  const edges: GEdge[] = [];
  const included = new Set<string>();
  const addType = (s: Symbol) => {
    if (included.has(s.id)) return;
    included.add(s.id);
    const members = index.members(s.id);
    nodes.push({
      id: s.id,
      type: "typeCard",
      data: { symbol: s, members, namespace: index.namespaceOf(s.id) } satisfies TypeCardData,
      width: 260,
      height: typeCardHeight(members.length),
      ports: [],
    });
  };
  for (const t of index.typesIn(namespace)) addType(t);

  const seen = new Set<string>();
  for (const e of index.graph.edges) {
    let kind: string | null = null;
    if (e.kind === "Inherits") kind = "inherits";
    else if (e.kind === "Implements") kind = "implements";
    else if (showUses && (e.kind === "References" || e.kind === "Instantiates")) kind = e.kind === "Instantiates" ? "creates" : "uses";
    if (!kind) continue;
    const from = index.owningType(e.from);
    const to = index.byId.get(e.to);
    if (!from || !to || from.id === to.id || !included.has(from.id)) continue;
    if (!included.has(to.id)) {
      // Pull in external bases so inheritance stays visible; skip external "uses".
      if (kind === "inherits" || kind === "implements") addType(to);
      else continue;
    }
    const key = `${from.id}->${to.id}:${kind}`;
    if (seen.has(key)) continue;
    // A "creates" edge makes a parallel "uses" edge redundant.
    if (kind === "uses" && seen.has(`${from.id}->${to.id}:creates`)) continue;
    seen.add(key);
    edges.push({ id: key, source: from.id, target: to.id, kind, layout: true });
  }
  return { nodes, edges, direction: "DOWN" };
}
