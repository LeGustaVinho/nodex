import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import type { OperationInfo } from "../gen/OperationInfo";
import type { Symbol } from "../gen/Symbol";
import type { SymbolInfo } from "../gen/SymbolInfo";
import { type DetailLevel, flowToGraph } from "./flowGraph";
import type { GEdge, GGraph, GNode } from "./model";
import type { StructureIndex } from "./structureGraph";

export const CLASS_IN = "class-in";
export const CLASS_OUT = "class-out";
export const flowNodePrefix = (flow: string) => `${flow}::`;

const isAccessorOwner = (s: Symbol) => s.kind === "Property" || s.kind === "Event";

function writtenMember(node: FlowNode, receiverName: "this" | "self"): string | undefined {
  const expr = node.expr;
  if (!expr || (expr.concept !== "Assign" && !expr.flavors.some((f) => f === "prefix" || f === "postfix"))) return;
  const target = expr.children[0];
  if (target?.concept === "Var" && node.flavors.includes("implicit-this")) return target.label;
  if (target?.concept === "GetMember" && target.label.startsWith(".")) {
    const receiver = target.children[0];
    if (receiver?.concept === "Var" && receiver.label === receiverName) return target.label.slice(1);
  }
}

export function classMembers(index: StructureIndex, typeId: string): Symbol[] {
  return index.members(typeId)
    .flatMap((s) => [...(s.has_flow ? [s] : []), ...(isAccessorOwner(s) ? (index.children.get(s.id) ?? []).filter((a) => a.has_flow) : [])])
    .sort((a, b) => (a.origins[0]?.start_line ?? Infinity) - (b.origins[0]?.start_line ?? Infinity));
}

export interface ClassBoxData {
  symbol: Symbol;
  bases: string[];
  members: string[];
}

export interface MethodCardData {
  symbol: Symbol;
  open: boolean;
  complexity?: number;
  calls: number;
}

export interface FieldsCardData {
  fields: Symbol[];
}

interface ClassBlueprintOptions {
  open: ReadonlySet<string>;
  expanded: Record<string, boolean>;
  showDataWires: boolean;
  detail?: DetailLevel;
  operations?: Record<string, OperationInfo>;
  symbolInfo?: ReadonlyMap<string, SymbolInfo>;
}

/** Prefix canvas IDs and reparent the flow, preserving IDs used by runtime and source overlays. */
export function prefixGraph(graph: GGraph, prefix: string, parent: string): GGraph {
  return {
    ...graph,
    nodes: graph.nodes.map((n) => ({
      ...n,
      id: prefix + n.id,
      parent: n.parent ? prefix + n.parent : parent,
      attach: n.attach && { ...n.attach, to: prefix + n.attach.to },
    })),
    edges: graph.edges.map((e) => ({
      ...e,
      id: prefix + e.id,
      source: prefix + e.source,
      target: prefix + e.target,
    })),
    layoutLinks: graph.layoutLinks?.map((e) => ({ source: prefix + e.source, target: prefix + e.target })),
  };
}

export function classBlueprintGraph(index: StructureIndex, typeId: string, flows: readonly FlowGraph[], opts: ClassBlueprintOptions): GGraph {
  const nodes: GNode[] = [];
  const edges: GEdge[] = [];
  const graph: GGraph = { nodes, edges, direction: "DOWN" };
  const type = index.byId.get(typeId);
  if (!type) return graph;

  const members = classMembers(index, typeId);
  const byMember = new Map(flows.map((f) => [f.symbol, f]));
  const bases = [...new Set(index.graph.edges
    .filter((e) => e.from === typeId && (e.kind === "Inherits" || e.kind === "Implements"))
    .map((e) => index.byId.get(e.to)?.name ?? e.to))];
  const fields = index.members(typeId).filter((s) => s.kind === "Field" || s.kind === "Event" ||
    (s.kind === "Property" && !s.has_flow && !(index.children.get(s.id) ?? []).some((a) => a.has_flow)));
  const fieldNames = new Set(fields.map((s) => s.name));
  const fieldsId = `fields:${typeId}`;
  const ordered = members.map((s) => s.id);
  if (fields.length) ordered.push(fieldsId);
  graph.layoutLinks = ordered.slice(1).map((target, i) => ({ source: ordered[i], target }));
  nodes.push({
    id: typeId, type: "classBox", container: true, direction: "DOWN", headerHeight: 46, width: 380, height: 76, ports: [],
    data: { symbol: type, bases, members: members.map((s) => s.id) } satisfies ClassBoxData,
  });
  if (fields.length) {
    nodes.push({ id: fieldsId, parent: typeId, type: "fieldsCard", width: 320, height: 34 + fields.length * 24, ports: [], data: { fields } satisfies FieldsCardData });
  }

  const entries = new Map<string, string>();
  const renderedFlows = new Set<string>();
  for (const symbol of members) {
    const flow = byMember.get(symbol.id);
    const open = !!flow?.nodes.length && opts.open.has(symbol.id);
    const data: MethodCardData = {
      symbol, open, complexity: opts.symbolInfo?.get(symbol.id)?.complexity,
      calls: flow?.nodes.reduce((n, step) => n + step.calls.length, 0) ?? 0,
    };
    nodes.push({
      id: symbol.id, parent: typeId, type: open ? "methodBox" : "methodCard", container: open,
      direction: open ? "RIGHT" : undefined, headerHeight: 38,
      width: 320, height: open ? 52 : 64, ports: [], data,
    });
    let entry = symbol.id;
    if (open && flow) {
      renderedFlows.add(symbol.id);
      const prefix = flowNodePrefix(symbol.id);
      const inner = prefixGraph(flowToGraph(flow, opts.expanded, opts.showDataWires, { level: opts.detail, operations: opts.operations }), prefix, symbol.id);
      nodes.push(...inner.nodes);
      edges.push(...inner.edges);
      entry = prefix + (flow.nodes.find((n) => n.concept === "Entry" || n.concept === "EventEntry") ?? flow.nodes[0]).id;
    }
    entries.set(symbol.id, entry);
  }

  const seen = new Set<string>();
  const wire = (source: string, target: string, kind: "calls" | "changes") => {
    const id = JSON.stringify([kind, source, target]);
    if (seen.has(id)) return;
    seen.add(id);
    edges.push({ id, source, target, sourceHandle: CLASS_OUT, targetHandle: CLASS_IN, kind, layout: false });
  };
  for (const { id, language } of members) {
    for (const node of byMember.get(id)?.nodes ?? []) {
      const source = renderedFlows.has(id) ? flowNodePrefix(id) + node.id : id;
      for (const { symbol } of node.calls) {
        if (!symbol || symbol === id) continue;
        const target = entries.get(symbol);
        if (target) wire(source, target, "calls");
      }
      const field = writtenMember(node, language === "rust" ? "self" : "this");
      if (field && fieldNames.has(field)) {
        wire(source, fieldsId, "changes");
      }
    }
  }
  return graph;
}
