import type { StructureGraph } from "../gen/StructureGraph";
import type { Symbol } from "../gen/Symbol";
import type { SymbolKind } from "../gen/SymbolKind";
import { type GEdge, type GGraph, type GNode } from "./model";

const TYPE_KINDS: SymbolKind[] = ["Class", "Struct", "Interface", "Trait", "Enum", "Union", "Record", "Delegate", "TypeAlias"];
export const isTypeKind = (k: SymbolKind) => TYPE_KINDS.includes(k);
/** Containers of declarations: namespaces (C#, TypeScript `namespace`) and file modules (TypeScript, Rust). */
export const isScopeKind = (k: SymbolKind) => k === "Namespace" || k === "Module";

export const isFileModule = (s: Symbol) =>
  s.kind === "Module" && (s.language === "typescript" || s.language === "rust") &&
  !s.external && !s.flavors.includes("inline-module");

export const isBlueprintContainer = (s: Symbol) => isTypeKind(s.kind) || isFileModule(s);

/** Indexes over a structure graph, shared by views. */
export class StructureIndex {
  byId = new Map<string, Symbol>();
  children = new Map<string, Symbol[]>();
  private modulesByFile = new Map<string, Symbol>();
  private subscribers?: Map<string, Symbol[]>;

  constructor(public graph: StructureGraph) {
    for (const s of graph.symbols) {
      this.byId.set(s.id, s);
      if (isFileModule(s)) {
        for (const origin of s.origins) this.modulesByFile.set(origin.file, s);
      }
    }
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

  /** Subscription targets discovered in source. The live list can depend on execution. */
  eventSubscribers(event: string): readonly Symbol[] {
    if (!this.subscribers) {
      this.subscribers = new Map();
      for (const edge of this.graph.edges) {
        if (edge.kind !== "Subscribes") continue;
        const subscriber = this.byId.get(edge.to);
        if (!subscriber) continue;
        const list = this.subscribers.get(edge.from) ?? [];
        list.push(subscriber);
        this.subscribers.set(edge.from, list);
      }
    }
    return this.subscribers.get(event) ?? [];
  }

  namespaceOf(id: string): string | null {
    return this.namespaceSymbolOf(id)?.qualified_name ?? null;
  }

  namespaceSymbolOf(id: string): Symbol | undefined {
    let cur = this.byId.get(id);
    while (cur && !isScopeKind(cur.kind)) cur = cur.parent ? this.byId.get(cur.parent) : undefined;
    return cur;
  }

  /** Members (non-type children) of a type, in declaration order. */
  members(typeId: string): Symbol[] {
    return (this.children.get(typeId) ?? []).filter((s) => !isTypeKind(s.kind));
  }

  /** Functions and variables declared directly in a scope (TypeScript modules and namespaces). */
  freeMembers(scopeId: string): Symbol[] {
    return (this.children.get(scopeId) ?? []).filter((s) => !isTypeKind(s.kind) && !isScopeKind(s.kind));
  }

  private allTypes?: Symbol[];
  private typesByNs?: Map<string | null, Symbol[]>;

  /**
   * Types declared in a namespace, including nested types, in graph order (`null`: all of them).
   * Indexed once, since the sidebar and the Architecture view ask for every namespace. The
   * returned array is shared: do not modify it.
   */
  typesIn(ns: string | null): Symbol[] {
    if (!this.allTypes || !this.typesByNs) {
      this.allTypes = [];
      this.typesByNs = new Map();
      for (const s of this.graph.symbols) {
        if (!isTypeKind(s.kind) || s.external) continue;
        this.allTypes.push(s);
        const key = this.namespaceOf(s.id);
        const list = this.typesByNs.get(key);
        if (list) list.push(s);
        else this.typesByNs.set(key, [s]);
      }
    }
    return ns === null ? this.allTypes : (this.typesByNs.get(ns) ?? []);
  }

  /** A unique outer type, or the unique outer type named after the file. */
  primaryTypeInFile(file: string): Symbol | undefined {
    const types = this.typesIn(null).filter((s) =>
      !s.flavors.includes("synthesized") && s.origins.some((o) => o.file === file) &&
      !(s.parent && this.owningType(s.parent)),
    );
    if (types.length <= 1) return types[0];
    const stem = file.split("/").at(-1)?.replace(/(?:\.d)?\.[^.]+$/, "").toLowerCase();
    const named = types.filter((s) => s.name.toLowerCase() === stem);
    return named.length === 1 ? named[0] : undefined;
  }

  /** Open a file's sole initializer callback directly. A module can show several callbacks. */
  fileBlueprint(file: string, inBlueprintView: boolean): Symbol | undefined {
    const module = this.modulesByFile.get(file);
    if (module?.language === "typescript") {
      const callbacks = (this.children.get(module.id) ?? [])
        .filter((symbol) => symbol.flavors.includes("top-level"))
        .flatMap((initializer) => this.children.get(initializer.id) ?? [])
        .filter((symbol) => symbol.has_flow && symbol.flavors.includes("initializer-callback") &&
          symbol.origins.some((origin) => origin.file === file));
      if (callbacks.length) return callbacks.length === 1 ? callbacks[0] : module;
    }
    return (inBlueprintView ? this.primaryTypeInFile(file) : undefined) ?? module;
  }
}

export interface NamespaceData {
  symbol: Symbol;
  types: number;
  /** Free functions (module-level code). */
  functions: number;
  external: boolean;
  /** For a group of namespaces or modules sharing a prefix: how many it holds. */
  group?: number;
}

/**
 * Above this many namespaces, modules and packages, the Architecture view groups them by path
 * prefix: a TypeScript or Rust project has one module per file, tens of thousands of nodes no
 * layout handles.
 */
export const ARCHITECTURE_CAP = 300;

/** The first `depth` segments of a qualified name (`a.b.c`, `src/a/b`, `app::a`), with their separators. */
function prefix(name: string, depth: number): string {
  return name.split(/(::|\.|\/)/).slice(0, depth * 2 - 1).join("");
}

/** Architecture view: namespaces, external namespaces and packages, with import edges. */
export function architectureGraph(index: StructureIndex, scope?: ReadonlySet<string>): GGraph & { grouped: number } {
  const scopes: { s: Symbol; types: number; functions: number }[] = [];
  const imports = index.graph.edges.filter((e) => e.kind === "Imports");
  const covered = new Set<string>();
  if (scope) {
    for (const id of scope) {
      for (let s = index.byId.get(id); s; s = s.parent ? index.byId.get(s.parent) : undefined) {
        if (covered.has(s.id)) break;
        covered.add(s.id);
      }
    }
  }
  const imported = new Set(imports.filter((e) => covered.has(e.from) && index.byId.get(e.to)?.external).map((e) => e.to));
  for (const s of index.graph.symbols) {
    if (!isScopeKind(s.kind) && s.kind !== "Package") continue;
    if (scope && !covered.has(s.id) && !imported.has(s.id)) continue;
    // Namespaces nested in a module (TypeScript `namespace X {}`) stay inside their module.
    if (s.kind === "Namespace" && s.parent && index.byId.get(s.parent)?.kind === "Module") continue;
    const types = isScopeKind(s.kind) ? index.typesIn(s.qualified_name).length : 0;
    const functions = s.kind === "Module" ? index.freeMembers(s.id).filter((m) => m.kind === "Function").length : 0;
    scopes.push({ s, types, functions });
  }
  const node = (symbol: Symbol, types: number, functions: number, group?: number): GNode => ({
    id: symbol.id,
    type: "namespace",
    data: { symbol, types, functions, external: symbol.external, group } satisfies NamespaceData,
    width: 220,
    height: symbol.external && !group ? 48 : 64,
    ports: [],
  });

  if (scopes.length <= ARCHITECTURE_CAP) {
    const included = new Set(scopes.map((x) => x.s.id));
    const edges: GEdge[] = imports
      .filter((e) => included.has(e.from) && included.has(e.to))
      .map((e) => ({ id: `${e.from}->${e.to}`, source: e.from, target: e.to, kind: "imports", layout: true }));
    return { nodes: scopes.map((x) => node(x.s, x.types, x.functions)), edges, direction: "RIGHT", grouped: 0 };
  }

  // The deepest prefix that still fits under the cap (one segment at least).
  const keyAt = (s: Symbol, depth: number) => `${s.external ? "ext" : "own"}|${s.kind === "Package" ? "pkg" : "ns"}|${prefix(s.qualified_name, depth)}`;
  let depth = 1;
  for (let d = 2; d <= 12; d++) {
    const keys = new Set(scopes.map((x) => keyAt(x.s, d)));
    if (keys.size > ARCHITECTURE_CAP) break;
    depth = d;
    if (keys.size === scopes.length) break;
  }
  const groups = new Map<string, typeof scopes>();
  for (const x of scopes) {
    const key = keyAt(x.s, depth);
    const list = groups.get(key);
    if (list) list.push(x);
    else groups.set(key, [x]);
  }
  const groupOf = new Map<string, string>();
  const nodes: GNode[] = [];
  for (const [key, members] of groups) {
    const first = members[0].s;
    if (members.length === 1) {
      groupOf.set(first.id, first.id);
      nodes.push(node(first, members[0].types, members[0].functions));
      continue;
    }
    const name = prefix(first.qualified_name, depth);
    const symbol: Symbol = {
      ...first,
      id: `group:${key}`,
      name,
      qualified_name: `${name} …`,
      parent: null,
      origins: [],
      flavors: [],
      signature: null,
      doc: null,
    };
    for (const m of members) groupOf.set(m.s.id, symbol.id);
    const sum = (f: (x: (typeof scopes)[number]) => number) => members.reduce((a, x) => a + f(x), 0);
    nodes.push(node(symbol, sum((x) => x.types), sum((x) => x.functions), members.length));
  }
  const weights = new Map<string, { source: string; target: string; weight: number }>();
  for (const e of imports) {
    const source = groupOf.get(e.from);
    const target = groupOf.get(e.to);
    if (!source || !target || source === target) continue;
    const id = `${source}->${target}`;
    const w = weights.get(id);
    if (w) w.weight++;
    else weights.set(id, { source, target, weight: 1 });
  }
  const edges: GEdge[] = [...weights].map(([id, w]) => ({ id, source: w.source, target: w.target, kind: "imports", layout: true, weight: w.weight }));
  return { nodes, edges, direction: "RIGHT", grouped: scopes.length };
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
export function typesGraph(index: StructureIndex, namespace: string | null, showUses: boolean, scope?: ReadonlySet<string>): GGraph {
  const nodes: GNode[] = [];
  const edges: GEdge[] = [];
  const included = new Set<string>();
  const addType = (s: Symbol) => {
    if (included.has(s.id)) return;
    included.add(s.id);
    const members = index.members(s.id).filter((member) => !scope || scope.has(member.id));
    nodes.push({
      id: s.id,
      type: "typeCard",
      data: { symbol: s, members, namespace: index.namespaceOf(s.id) } satisfies TypeCardData,
      width: 260,
      height: typeCardHeight(members.length),
      ports: [],
    });
  };
  for (const t of index.typesIn(namespace)) if (!scope || scope.has(t.id)) addType(t);

  const seen = new Set<string>();
  for (const e of index.graph.edges) {
    let kind: string | null = null;
    if (e.kind === "Inherits") kind = "inherits";
    else if (e.kind === "Implements") kind = "implements";
    else if (e.kind === "Compatible") kind = "compatible";
    else if (showUses && (e.kind === "References" || e.kind === "Instantiates")) kind = e.kind === "Instantiates" ? "creates" : "uses";
    if (!kind || (scope && !scope.has(e.from))) continue;
    const from = index.owningType(e.from);
    const to = index.byId.get(e.to);
    if (!from || !to || from.id === to.id || !included.has(from.id) || (scope && !scope.has(from.id))) continue;
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
