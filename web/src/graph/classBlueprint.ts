import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import { languageDetailFlavors } from "./blueprintExplanation";
import type { Expr } from "../gen/Expr";
import type { OperationInfo } from "../gen/OperationInfo";
import type { Symbol } from "../gen/Symbol";
import type { SymbolInfo } from "../gen/SymbolInfo";
import type { BlueprintSources } from "./blueprintSource";
import { type DetailLevel, flowToGraph } from "./flowGraph";
import type { GEdge, GGraph, GNode } from "./model";
import { isFileModule, isTypeKind, type StructureIndex } from "./structureGraph";

export const CLASS_IN = "class-in";
export const CLASS_OUT = "class-out";
export const FIELD_OUT = "field-out";
export const flowNodePrefix = (flow: string) => `${flow}::`;

const isAccessorOwner = (s: Symbol) => s.kind === "Property" || s.kind === "Event";

function qualifiedReceiver(expr: Expr): { name: string; root: string } | undefined {
  if (expr.concept === "Var") return { name: expr.label, root: expr.label };
  if (expr.concept === "GetMember" && expr.label.startsWith(".") && expr.children[0]) {
    const left = qualifiedReceiver(expr.children[0]);
    if (left) return { name: left.name + expr.label, root: left.root };
  }
}

function typeQualifiers(index: StructureIndex, owner: Symbol): ReadonlySet<string> {
  const names = new Set([owner.name, owner.qualified_name]);
  if (owner.language === "csharp") {
    // C# IDs encode generic arity. Build the source spelling from each declaring type's name.
    const types = [owner.name];
    for (let id = owner.parent; id;) {
      const parent = index.byId.get(id);
      if (!parent || !isTypeKind(parent.kind)) break;
      types.unshift(parent.name);
      id = parent.parent;
    }
    const local = types.join(".");
    const namespace = index.namespaceOf(owner.id);
    const qualified = namespace && namespace !== "(global)" ? `${namespace}.${local}` : local;
    names.add(local);
    names.add(qualified);
    names.add(`global::${qualified}`);
  }
  return new Set([...names].map((name) => name.replace(/\s+/g, "")));
}

function writtenMember(node: FlowNode, owner: Symbol, fields: ReadonlyMap<string, Symbol>, qualifiers: ReadonlySet<string>): string | undefined {
  const expr = node.expr;
  if (!expr || (expr.concept !== "Assign" && !expr.flavors.some((f) => f === "prefix" || f === "postfix"))) return;
  const target = expr.children[0];
  if (target?.concept === "Var" && node.flavors.includes("implicit-this")) return target.label;
  if (target?.concept === "GetMember" && target.label.startsWith(".")) {
    const receiver = target.children[0];
    if (!receiver) return;
    const name = target.label.slice(1);
    if (receiver.concept === "Var" && receiver.label === (owner.language === "rust" ? "self" : "this")) return name;
    if (!fields.get(name)?.flavors.includes("static")) return;
    const qualifier = qualifiedReceiver(receiver);
    // A local receiver already has a data pin, even when it shadows the type name.
    if (qualifier && qualifiers.has(qualifier.name.replace(/\s+/g, "")) &&
        !node.inputs.some((p) => p.id === `use:${qualifier.root}`)) return name;
  }
}

function moduleDeclarations(index: StructureIndex, ownerId: string): Symbol[] {
  return (index.children.get(ownerId) ?? []).flatMap((s) =>
    s.kind === "Namespace" || s.kind === "Field" || (s.kind === "Module" && s.flavors.includes("inline-module"))
      ? [s, ...moduleDeclarations(index, s.id)] : [s],
  );
}

export function classMembers(index: StructureIndex, typeId: string): Symbol[] {
  const owner = index.byId.get(typeId);
  const declarations = owner && isFileModule(owner) ? moduleDeclarations(index, typeId) : index.members(typeId);
  return declarations
    .flatMap((s) => [...(s.has_flow ? [s] : []), ...(isAccessorOwner(s) ? (index.children.get(s.id) ?? []).filter((a) => a.has_flow) : [])])
    .sort((a, b) => (a.origins[0]?.start_line ?? Infinity) - (b.origins[0]?.start_line ?? Infinity));
}

export interface ClassBoxData {
  symbol: Symbol;
  bases: string[];
  members: string[];
  declaration?: string;
  note?: string;
}

export interface MethodCardData {
  symbol: Symbol;
  open: boolean;
  complexity?: number;
  calls: number;
  note?: string;
}

export interface FieldCardData {
  symbol: Symbol;
  note?: string;
  declaration?: string;
  role?: "contract" | "variant";
}

function receiverNote(symbol: Symbol): string | undefined {
  if (symbol.language !== "rust") return undefined;
  if (symbol.flavors.includes("&mut self")) return "Borrows receiver to change it";
  if (symbol.flavors.includes("&self")) return "Borrows receiver for shared access";
  if (symbol.flavors.includes("self")) return "Receives receiver by value";
}

function declarationNote(symbol: Symbol): string | undefined {
  const flavors = new Set(symbol.flavors);
  const labels: string[] = [];
  if (symbol.kind === "Property" && symbol.language === "csharp") {
    if (flavors.has("accessor:get") || flavors.has("computed")) labels.push("Read");
    if (flavors.has("accessor:set")) labels.push("Set");
  }
  if (symbol.language === "typescript" && (symbol.kind === "Property" || symbol.kind === "Field")) {
    labels.push(flavors.has("optional") ? "Optional" : "Required");
  }
  return labels.length ? labels.join(" · ") : undefined;
}

interface ClassBlueprintOptions {
  open: ReadonlySet<string>;
  expanded: Record<string, boolean>;
  showDataWires: boolean;
  explanations?: boolean;
  detail?: DetailLevel;
  operations?: Record<string, OperationInfo>;
  symbolInfo?: ReadonlyMap<string, SymbolInfo>;
  sources?: BlueprintSources;
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
  // Expression-bodied properties already have their own flow card. Properties with
  // accessors have separate accessor symbols, so the declaration gets a card as well.
  const fields = index.members(typeId).filter((s) => !s.has_flow && (s.kind === "Field" || isAccessorOwner(s)));
  const fieldsByName = new Map(fields.map((s) => [s.name, s]));
  const declaredTypes = isFileModule(type)
    ? moduleDeclarations(index, typeId).filter((s) => isTypeKind(s.kind) && !s.external && !s.flavors.includes("synthesized"))
    : [];
  const contracts = (index.children.get(typeId) ?? []).filter((s) => !s.has_flow && (
    s.kind === "EnumMember" || s.flavors.includes("associated-type") ||
    (["Method", "Function", "Constructor"].includes(s.kind) &&
      (type.kind === "Interface" || type.kind === "Trait" || s.flavors.some((f) => ["abstract", "declare", "extern", "required"].includes(f))))
  ));
  const discriminant = type.flavors.find((f) => f.startsWith("discriminated:"))?.slice("discriminated:".length);
  const declaration = type.kind === "Delegate" || type.kind === "TypeAlias" || discriminant ? type.signature ?? type.type_text ?? undefined : undefined;
  const typeNote = opts.explanations === false ? undefined
    : discriminant ? `Variant identified by ${discriminant}` : members.length === 0 ? "Declarations and contracts" : undefined;
  const headerHeight = 46 + (typeNote ? 20 : 0)
    + (opts.explanations !== false && (type.syntax_sites?.length || languageDetailFlavors(type.flavors).length) ? 20 : 0);
  const qualifiers = typeQualifiers(index, type);
  const ordered = members.map((s) => s.id);
  graph.layoutLinks = ordered.slice(1).map((target, i) => ({ source: ordered[i], target }));
  nodes.push({
    id: typeId, type: "classBox", container: true, direction: "DOWN", headerHeight, width: 380, height: Math.max(76, headerHeight), ports: [],
    data: { symbol: type, bases, members: members.map((s) => s.id), declaration, note: typeNote } satisfies ClassBoxData,
  });
  for (const symbol of [...fields, ...declaredTypes, ...contracts]) {
    const isContract = contracts.includes(symbol);
    const role = isContract ? (symbol.kind === "EnumMember" ? "variant" : "contract") : undefined;
    const contractKind = symbol.flavors.includes("associated-type") ? "Associated type"
      : symbol.flavors.includes("required") ? "Required contract" : "Declaration";
    const note = opts.explanations === false || role === "variant" ? undefined : isContract
      ? [contractKind, receiverNote(symbol)].filter(Boolean).join(" · ") : declarationNote(symbol);
    const declaration = isContract ? symbol.signature ?? symbol.type_text ?? undefined : undefined;
    nodes.push({
      id: symbol.id, parent: typeId, type: "fieldCard", layoutSide: "right",
      width: 260, height: 48 + (note ? 17 : 0) + (opts.explanations !== false && (symbol.syntax_sites?.length || languageDetailFlavors(symbol.flavors).length) ? 20 : 0),
      ports: [], data: { symbol, note, declaration, role } satisfies FieldCardData,
    });
  }

  const entries = new Map<string, string>();
  const renderedFlows = new Set<string>();
  for (const symbol of members) {
    const flow = byMember.get(symbol.id);
    const open = !!flow?.nodes.length && opts.open.has(symbol.id);
    const notes = [symbol.language === "rust" && symbol.flavors.includes("default") ? "Default implementation" : undefined,
      receiverNote(symbol) ?? declarationNote(symbol)].filter(Boolean);
    const note = opts.explanations === false || notes.length === 0 ? undefined : notes.join(" · ");
    const data: MethodCardData = {
      symbol, open, complexity: opts.symbolInfo?.get(symbol.id)?.complexity,
      calls: flow?.nodes.reduce((n, step) => n + step.calls.length, 0) ?? 0,
      note,
    };
    const detailsHeight = opts.explanations !== false && (symbol.syntax_sites?.length || languageDetailFlavors(symbol.flavors).length) ? 20 : 0;
    nodes.push({
      id: symbol.id, parent: typeId, type: open ? "methodBox" : "methodCard", container: open,
      direction: open ? "RIGHT" : undefined, headerHeight: 38 + (note ? 17 : 0) + detailsHeight,
      width: 320, height: (open ? 52 : 64) + (note ? 17 : 0) + detailsHeight, ports: [], data,
    });
    let entry = symbol.id;
    if (open && flow) {
      renderedFlows.add(symbol.id);
      const prefix = flowNodePrefix(symbol.id);
      const inner = prefixGraph(flowToGraph(flow, opts.expanded, opts.showDataWires, {
        level: opts.detail, operations: opts.operations, explanations: opts.explanations, source: opts.sources?.[flow.span.file], structure: index,
      }), prefix, symbol.id);
      nodes.push(...inner.nodes);
      edges.push(...inner.edges);
      entry = prefix + (flow.nodes.find((n) => n.concept === "Entry" || n.concept === "EventEntry") ?? flow.nodes[0]).id;
    }
    entries.set(symbol.id, entry);
  }

  const seen = new Set<string>();
  const wire = (source: string, target: string, kind: "calls" | "changes", sourceHandle = CLASS_OUT) => {
    const id = JSON.stringify([kind, source, target]);
    if (seen.has(id)) return;
    seen.add(id);
    edges.push({ id, source, target, sourceHandle, targetHandle: CLASS_IN, kind, layout: false });
  };
  for (const { id } of members) {
    for (const node of byMember.get(id)?.nodes ?? []) {
      const source = renderedFlows.has(id) ? flowNodePrefix(id) + node.id : id;
      for (const { symbol } of node.calls) {
        if (!symbol || symbol === id) continue;
        const target = entries.get(symbol);
        if (target) wire(source, target, "calls");
      }
      const name = writtenMember(node, type, fieldsByName, qualifiers);
      const field = name === undefined ? undefined : fieldsByName.get(name);
      if (field) {
        wire(source, field.id, "changes", renderedFlows.has(id) ? CLASS_OUT : FIELD_OUT);
      }
    }
  }
  return graph;
}
