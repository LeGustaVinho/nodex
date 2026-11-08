/**
 * Helpers of the explorer views (Hierarchy, Knowledge, Overview, Inspector), ported from
 * CodeGraph's explorer: which edges a selection lights up, the layouts that are not ELK's,
 * grouping boxes, third-party code, and the read-only analyses behind the Inspector's tabs
 * (impact radius, tests of a symbol, call trees, full-stack and error flows, file skeleton).
 * Everything here is pure, so it can be unit-tested.
 */
import type { Confidence } from "../gen/Confidence";
import type { ModelField } from "../gen/ModelField";
import type { ProjectKnowledge } from "../gen/ProjectKnowledge";
import type { StructEdge } from "../gen/StructEdge";
import type { StructEdgeKind } from "../gen/StructEdgeKind";
import type { Symbol } from "../gen/Symbol";
import type { EdgeMode, GroupBy, LayoutMode, Spacing } from "../store";
import type { GEdge, GGraph, GNode } from "./model";
import { type StructureIndex, isTypeKind } from "./structureGraph";

// ---------------------------------------------------------------------------- third-party code

const VENDOR_DIRS = ["thirdparty", "third_party", "third-party", "vendor", "external"];

/**
 * Third-party code: a `ThirdParty/`, `third_party/`, `vendor/` or `External/` folder anywhere, plus
 * Unity's `Assets/Plugins/` and `Packages/` (same defaults as CodeGraph).
 */
export function isVendorPath(path: string, unity: boolean): boolean {
  const parts = path.replace(/\\/g, "/").split("/");
  const dirs = parts.slice(0, -1).map((p) => p.toLowerCase());
  if (dirs.some((d) => VENDOR_DIRS.includes(d))) return true;
  if (!unity) return false;
  return (dirs[0] === "assets" && dirs[1] === "plugins") || dirs[0] === "packages";
}

/** True when the symbol is declared only in third-party code. */
export function isVendorSymbol(s: Symbol, unity: boolean): boolean {
  return s.origins.length > 0 && s.origins.every((o) => isVendorPath(o.file, unity));
}

// ---------------------------------------------------------------------------- selection and edges

/** What a selected node lights up, for a given edge mode. */
export interface EdgeFocus {
  /** Edges not drawn (`edgeMode` other than `all`). */
  hidden: Set<string>;
  /** Edges into the selection (coloured when only the selection's edges are drawn). */
  incoming: Set<string>;
  outgoing: Set<string>;
  /** Edges drawn in full, with their label. */
  lit: Set<string>;
  /** Nodes kept bright; `null` = nothing is dimmed. */
  litNodes: Set<string> | null;
}

/**
 * The edges of `anchor` (a box counts with everything inside it), and what stays bright: the
 * anchor, its contents, the other end of each shown edge and the boxes around them.
 */
export function edgeFocus(g: Pick<GGraph, "nodes" | "edges">, anchor: string | null, mode: EdgeMode): EdgeFocus {
  const out: EdgeFocus = { hidden: new Set(), incoming: new Set(), outgoing: new Set(), lit: new Set(), litNodes: null };
  const parent = new Map(g.nodes.map((n) => [n.id, n.parent]));
  if (!anchor || !parent.has(anchor)) {
    if (mode !== "all") for (const e of g.edges) out.hidden.add(e.id);
    return out;
  }
  const inside = (id: string) => {
    for (let cur: string | undefined = id; cur; cur = parent.get(cur)) if (cur === anchor) return true;
    return false;
  };
  const core = new Set(g.nodes.filter((n) => inside(n.id)).map((n) => n.id));
  for (const e of g.edges) {
    const s = core.has(e.source);
    const t = core.has(e.target);
    if (s) out.outgoing.add(e.id);
    else if (t) out.incoming.add(e.id);
  }
  const shown = mode === "in" ? out.incoming : mode === "out" ? out.outgoing : new Set([...out.incoming, ...out.outgoing]);
  if (mode !== "all") for (const e of g.edges) if (!shown.has(e.id)) out.hidden.add(e.id);
  if (mode === "in") out.outgoing.clear();
  if (mode === "out") out.incoming.clear();
  const lit = new Set(core);
  for (const e of g.edges) {
    if (!shown.has(e.id)) continue;
    out.lit.add(e.id);
    lit.add(e.source);
    lit.add(e.target);
  }
  // Boxes around a bright node stay bright (their contents would vanish with them).
  for (const id of [...lit]) for (let cur = parent.get(id); cur; cur = parent.get(cur)) lit.add(cur);
  out.litNodes = lit;
  return out;
}

/** Relations of each node in the graph (both directions). */
export function degrees(edges: readonly GEdge[]): Map<string, number> {
  const d = new Map<string, number>();
  for (const e of edges) {
    d.set(e.source, (d.get(e.source) ?? 0) + 1);
    d.set(e.target, (d.get(e.target) ?? 0) + 1);
  }
  return d;
}

// ---------------------------------------------------------------------------- layouts

export const SPACING_FACTOR: Record<Spacing, number> = { compact: 0.7, normal: 1, wide: 1.6 };

/** Layouts computed here rather than by ELK. */
export type CustomLayout = "layers" | "concentric" | "circle" | "grid";

export const LAYOUTS: { id: LayoutMode; label: string; hint: string }[] = [
  { id: "auto", label: "Auto", hint: "Layers from the focus when there is one, else following the relations" },
  { id: "layered", label: "Layered", hint: "Columns that follow the direction of the relations (callers before callees)" },
  { id: "layers", label: "Layers from focus", hint: "The focus first, then one column per step away from it (callers on the left)" },
  { id: "force", label: "Force-directed", hint: "Related nodes pull together, unrelated ones push apart" },
  { id: "concentric", label: "Concentric", hint: "The focus in the middle, each step away one ring further out" },
  { id: "circle", label: "Circle", hint: "Every node on one circle" },
  { id: "grid", label: "Grid", hint: "Rows and columns, most connected first" },
];

/** ELK algorithm or custom layout behind a layout mode (grouped graphs always use ELK's layered). */
export function resolveLayout(mode: LayoutMode, focus: string | null, nested: boolean): "layered" | "stress" | CustomLayout {
  if (nested) return "layered";
  if (mode === "auto") return focus ? "layers" : "layered";
  if (mode === "force") return "stress";
  return mode;
}

type Box = { x: number; y: number };

/** Positions of a layout computed here (`layers`, `concentric`, `circle`, `grid`). */
export function customLayout(g: GGraph, layout: CustomLayout, focus: string | null, factor: number): Map<string, Box> {
  const nodes = g.nodes.filter((n) => !n.parent && !n.attach);
  const pos = new Map<string, Box>();
  if (nodes.length === 0) return pos;
  const w = Math.max(...nodes.map((n) => n.width));
  const h = Math.max(...nodes.map((n) => n.height));
  const deg = degrees(g.edges);
  const byDegree = [...nodes].sort((a, b) => (deg.get(b.id) ?? 0) - (deg.get(a.id) ?? 0) || a.id.localeCompare(b.id));
  const center = focus && nodes.some((n) => n.id === focus) ? focus : byDegree[0].id;

  if (layout === "grid") {
    const cols = Math.max(1, Math.ceil(Math.sqrt(nodes.length * 1.6)));
    byDegree.forEach((n, i) => pos.set(n.id, { x: (i % cols) * (w + 40 * factor), y: Math.floor(i / cols) * (h + 40 * factor) }));
    return pos;
  }
  if (layout === "circle") {
    // Room along the circle for each node (cards are wide: their width counts most).
    const r = Math.max(w, (nodes.length * (w + 30 * factor)) / (2 * Math.PI));
    byDegree.forEach((n, i) => {
      const a = (2 * Math.PI * i) / nodes.length - Math.PI / 2;
      pos.set(n.id, { x: r + r * Math.cos(a), y: r + r * Math.sin(a) });
    });
    return pos;
  }

  // Steps from the focus: following edges forward (callees, positive), backward (callers,
  // negative), or either way for nodes only reachable through a mix.
  const out = new Map<string, string[]>();
  const into = new Map<string, string[]>();
  for (const e of g.edges) {
    (out.get(e.source) ?? out.set(e.source, []).get(e.source)!).push(e.target);
    (into.get(e.target) ?? into.set(e.target, []).get(e.target)!).push(e.source);
  }
  const ids = new Set(nodes.map((n) => n.id));
  const level = new Map<string, number>([[center, 0]]);
  const bfs = (next: Map<string, string[]>, sign: 1 | -1) => {
    let frontier = [center];
    for (let step = 1; frontier.length; step++) {
      const nxt: string[] = [];
      for (const id of frontier) for (const o of next.get(id) ?? []) if (ids.has(o) && !level.has(o)) {
        level.set(o, sign * step);
        nxt.push(o);
      }
      frontier = nxt;
    }
  };
  bfs(out, 1);
  bfs(into, -1);
  // Undirected steps for the rest; unreachable nodes go one column past the farthest.
  let frontier = [...level.keys()];
  while (frontier.length) {
    const nxt: string[] = [];
    for (const id of frontier) {
      const l = level.get(id)!;
      for (const o of [...(out.get(id) ?? []), ...(into.get(id) ?? [])]) if (ids.has(o) && !level.has(o)) {
        level.set(o, l >= 0 ? l + 1 : l - 1);
        nxt.push(o);
      }
    }
    frontier = nxt;
  }
  const far = Math.max(0, ...[...level.values()].map(Math.abs)) + 1;
  for (const n of nodes) if (!level.has(n.id)) level.set(n.id, far);

  if (layout === "layers") {
    const cols = new Map<number, GNode[]>();
    for (const n of byDegree) {
      const l = level.get(n.id)!;
      (cols.get(l) ?? cols.set(l, []).get(l)!).push(n);
    }
    const tallest = Math.max(...[...cols.values()].map((c) => c.length));
    for (const [l, col] of cols) {
      const top = ((tallest - col.length) * (h + 24 * factor)) / 2;
      col.forEach((n, i) => pos.set(n.id, { x: l * (w + 90 * factor), y: top + i * (h + 24 * factor) }));
    }
    return pos;
  }

  // Concentric: rings by distance from the focus.
  const rings = new Map<number, GNode[]>();
  for (const n of byDegree) {
    const r = Math.abs(level.get(n.id)!);
    (rings.get(r) ?? rings.set(r, []).get(r)!).push(n);
  }
  let radius = 0;
  for (const r of [...rings.keys()].sort((a, b) => a - b)) {
    const ring = rings.get(r)!;
    if (r === 0) {
      ring.forEach((n, i) => pos.set(n.id, { x: i * (w + 30), y: 0 }));
      continue;
    }
    radius = Math.max(radius + (h + 90) * factor, r === 1 ? w + 40 * factor : 0, (ring.length * (w + 30 * factor)) / (2 * Math.PI));
    ring.forEach((n, i) => {
      const a = (2 * Math.PI * i) / ring.length - Math.PI / 2;
      pos.set(n.id, { x: radius * Math.cos(a), y: radius * Math.sin(a) });
    });
  }
  return pos;
}

// ---------------------------------------------------------------------------- grouping

/** Container chain of a symbol for a grouping: nearest box first. */
export function groupChain(index: StructureIndex, id: string, groupBy: GroupBy): Symbol[] {
  if (groupBy === "none") return [];
  const out: Symbol[] = [];
  let cur = index.byId.get(id)?.parent ?? null;
  while (cur) {
    const s = index.byId.get(cur);
    if (!s) break;
    const keep = isTypeKind(s.kind) || (groupBy !== "class" && s.kind === "Module") || (groupBy === "namespace" && (s.kind === "Namespace" || s.kind === "Package"));
    if (keep) out.push(s);
    cur = s.parent;
  }
  return out;
}

// ---------------------------------------------------------------------------- symbols

/** Visibility written in a declaration (`public`, `pub(crate)`, `export`...), or null. */
export function visibilityOf(s: Symbol): string | null {
  const sig = s.signature ?? "";
  const m = /^\s*(?:\[[^\]]*\]\s*)*(pub(?:\([^)]*\))?|public|private|protected internal|private protected|protected|internal|export default|export)(?=\s|$)/.exec(sig);
  if (m) return m[1];
  const f = s.flavors.find((x) => ["public", "private", "protected", "internal"].includes(x));
  return f ?? null;
}

export interface Params {
  total: number;
  required: number;
  variadic: boolean;
}

/** Parameters of a declaration, read from its signature (`null` when it has no parameter list). */
export function paramsOf(signature: string | null): Params | null {
  if (!signature) return null;
  const open = signature.indexOf("(");
  if (open < 0) return null;
  let depth = 0;
  let close = -1;
  for (let i = open; i < signature.length; i++) {
    const c = signature[i];
    if (c === "(" || c === "<" || c === "[" || c === "{") depth++;
    else if (c === ")" || c === ">" || c === "]" || c === "}") {
      // `=>` is an arrow, not a closing bracket.
      if (c === ">" && signature[i - 1] === "=") continue;
      depth--;
      if (depth === 0 && c === ")") {
        close = i;
        break;
      }
    }
  }
  if (close < 0) return null;
  const inner = signature.slice(open + 1, close);
  const parts: string[] = [];
  depth = 0;
  let start = 0;
  for (let i = 0; i < inner.length; i++) {
    const c = inner[i];
    if (c === "(" || c === "<" || c === "[" || c === "{") depth++;
    else if ((c === ")" || c === ">" || c === "]" || c === "}") && !(c === ">" && inner[i - 1] === "=")) depth--;
    else if (c === "," && depth === 0) {
      parts.push(inner.slice(start, i));
      start = i + 1;
    }
  }
  parts.push(inner.slice(start));
  // `self`, `&self`, `&mut self` and `this` (extension methods keep theirs: it is an argument).
  const params = parts.map((p) => p.trim()).filter((p) => p && !/^&?\s*(mut\s+)?self\b/.test(p));
  const variadic = params.some((p) => /^(params\b|\.\.\.)|\*\w/.test(p));
  const optional = (p: string) => /=/.test(p) || /^\w+\?\s*:/.test(p) || /^(params\b|\.\.\.)/.test(p);
  return { total: params.length, required: params.filter((p) => !optional(p)).length, variadic };
}

/** Lines of the symbol's first declaration. */
export function linesOf(s: Symbol): { file: string; start: number; end: number } | null {
  const o = s.origins[0];
  return o ? { file: o.file, start: o.start_line, end: o.end_line } : null;
}

/** `src/Orders.cs:42`. */
export function coordsOf(s: Symbol): string {
  const o = s.origins[0];
  return o ? `${o.file}:${o.start_line}` : s.qualified_name;
}

export type ComplexityLevel = "low" | "medium" | "high";
export const complexityLevel = (cc: number): ComplexityLevel => (cc >= 10 ? "high" : cc >= 5 ? "medium" : "low");

/** `one` or `many` for a field that points at another model (collections are `many`). */
export function cardinality(f: ModelField): "one" | "many" | null {
  if (!f.target) return null;
  return /(\[\]|^(I?List|I?Collection|IEnumerable|ICollection|HashSet|ISet|Set|Vec|Array|IReadOnlyList|IReadOnlyCollection)\s*<)/.test(f.ty ?? "") ? "many" : "one";
}

// ---------------------------------------------------------------------------- analyses

/** Relations a caller depends on its callee through (who breaks when the callee changes). */
const DEPENDS: ReadonlySet<StructEdgeKind> = new Set(["Calls", "References", "Inherits", "Implements", "Instantiates", "HttpCalls", "Subscribes", "Raises", "DependsOn", "Imports"]);

export interface Adjacency {
  out: Map<string, StructEdge[]>;
  into: Map<string, StructEdge[]>;
}

export function adjacency(edges: readonly StructEdge[], keep: (e: StructEdge) => boolean = (e) => e.kind !== "Contains"): Adjacency {
  const out = new Map<string, StructEdge[]>();
  const into = new Map<string, StructEdge[]>();
  for (const e of edges) {
    if (!keep(e)) continue;
    (out.get(e.from) ?? out.set(e.from, []).get(e.from)!).push(e);
    (into.get(e.to) ?? into.set(e.to, []).get(e.to)!).push(e);
  }
  return { out, into };
}

/** A type stands for its members (a call to a method reaches the type). */
function selfAndMembers(index: StructureIndex, id: string): string[] {
  const s = index.byId.get(id);
  return s && isTypeKind(s.kind) ? [id, ...index.members(id).map((m) => m.id)] : [id];
}

/** What depends on `id`, by steps (step 1 = direct dependents). Tests are counted apart. */
export function impactRadius(index: StructureIndex, id: string, depth = 3): { levels: string[][]; tests: string[] } {
  const adj = adjacency(index.graph.edges, (e) => DEPENDS.has(e.kind));
  const seen = new Set(selfAndMembers(index, id));
  const levels: string[][] = [];
  const tests = new Set<string>();
  let frontier = [...seen];
  for (let step = 0; step < depth && frontier.length; step++) {
    const next: string[] = [];
    for (const cur of frontier) {
      for (const e of adj.into.get(cur) ?? []) {
        if (seen.has(e.from)) continue;
        seen.add(e.from);
        next.push(e.from);
      }
    }
    if (next.length) levels.push(next);
    frontier = next;
  }
  const testEdges = adjacency(index.graph.edges, (e) => e.kind === "Tests");
  for (const s of seen) for (const e of testEdges.into.get(s) ?? []) tests.add(e.from);
  return { levels, tests: [...tests] };
}

export interface TestHit {
  test: string;
  /** `direct` = a Tests relation; `via` = the test calls something that reaches the symbol. */
  how: "direct" | "via";
  confidence: Confidence;
  path: string[];
}

/** Tests that cover `id`: by a Tests relation, or by calling something that calls it (3 steps). */
export function testsFor(index: StructureIndex, id: string, isTest: (id: string) => boolean): TestHit[] {
  const hits = new Map<string, TestHit>();
  const targets = selfAndMembers(index, id);
  for (const e of index.graph.edges) {
    if (e.kind === "Tests" && targets.includes(e.to) && !hits.has(e.from)) hits.set(e.from, { test: e.from, how: "direct", confidence: e.confidence ?? "Exact", path: [e.from, e.to] });
  }
  const calls = adjacency(index.graph.edges, (e) => e.kind === "Calls");
  const prev = new Map<string, string>();
  const seen = new Set(targets);
  let frontier = [...targets];
  for (let step = 0; step < 3 && frontier.length; step++) {
    const next: string[] = [];
    for (const cur of frontier) for (const e of calls.into.get(cur) ?? []) {
      if (seen.has(e.from)) continue;
      seen.add(e.from);
      prev.set(e.from, cur);
      next.push(e.from);
      if (isTest(e.from) && !hits.has(e.from)) {
        const path = [e.from, cur];
        for (let c = cur; prev.has(c); ) {
          c = prev.get(c)!;
          path.push(c);
        }
        hits.set(e.from, { test: e.from, how: "via", confidence: "Exact", path });
      }
    }
    frontier = next;
  }
  return [...hits.values()];
}

export interface TreeNode {
  id: string;
  kind: StructEdgeKind | null;
  confidence: Confidence;
  children: TreeNode[];
  /** Already shown higher up (not expanded again). */
  repeat?: boolean;
}

const CALL_KINDS: ReadonlySet<StructEdgeKind> = new Set(["Calls", "Raises", "Subscribes", "HttpCalls"]);

/** Callers (`in`) or callees (`out`) of `id`, `depth` steps, as a tree (each symbol expanded once). */
export function callTree(index: StructureIndex, id: string, dir: "in" | "out", depth: number, kinds: ReadonlySet<StructEdgeKind> = CALL_KINDS): TreeNode {
  const adj = adjacency(index.graph.edges, (e) => kinds.has(e.kind));
  const seen = new Set<string>([id]);
  const grow = (node: TreeNode, left: number) => {
    if (left === 0) return;
    for (const e of (dir === "out" ? adj.out.get(node.id) : adj.into.get(node.id)) ?? []) {
      const other = dir === "out" ? e.to : e.from;
      const child: TreeNode = { id: other, kind: e.kind, confidence: e.confidence ?? "Exact", children: [] };
      if (seen.has(other)) child.repeat = true;
      else {
        seen.add(other);
        grow(child, left - 1);
      }
      node.children.push(child);
    }
  };
  const root: TreeNode = { id, kind: null, confidence: "Exact", children: [] };
  for (const start of selfAndMembers(index, id)) {
    if (start === id) grow(root, depth);
    else {
      const member: TreeNode = { id: start, kind: null, confidence: "Exact", children: [] };
      seen.add(start);
      grow(member, depth);
      if (member.children.length) root.children.push(member);
    }
  }
  return root;
}

export interface FullstackFlow {
  /** Functions that send requests to this symbol (or the handlers this client calls). */
  clients: { id: string; confidence: Confidence; evidence: string | null }[];
  routes: { method: string; path: string; handler: string; framework: string }[];
  /** Handlers reached (the symbol itself when it handles a route). */
  handlers: string[];
  /** What the handlers go on to do, a few steps deep. */
  downstream: TreeNode[];
  /** Data models the flow writes. */
  writes: string[];
}

/**
 * Front end to data: who requests the route a handler serves, the routes, what the handler calls
 * (3 steps, through dependency injection) and the models it changes. For a client function, the
 * flow starts at the handlers it requests.
 */
export function fullstackFlow(index: StructureIndex, knowledge: ProjectKnowledge | undefined, id: string): FullstackFlow | null {
  const http = index.graph.edges.filter((e) => e.kind === "HttpCalls");
  const asHandler = http.filter((e) => e.to === id);
  const asClient = http.filter((e) => e.from === id);
  const routes = (knowledge?.routes ?? []).filter((r) => r.handler === id);
  if (!asHandler.length && !asClient.length && !routes.length) return null;
  const handlers = asClient.length ? [...new Set(asClient.map((e) => e.to))] : [id];
  const clients = asClient.length
    ? [{ id, confidence: "Exact" as Confidence, evidence: null }]
    : asHandler.map((e) => ({ id: e.from, confidence: e.confidence ?? "Exact", evidence: e.evidence ?? null }));
  const handlerRoutes = (knowledge?.routes ?? []).filter((r) => handlers.includes(r.handler));
  const downstream = handlers.map((h) => callTree(index, h, "out", 3, new Set(["Calls", "Mutates", "HttpCalls"])));
  const writes = new Set<string>();
  const collect = (n: TreeNode) => {
    if (n.kind === "Mutates") writes.add(n.id);
    n.children.forEach(collect);
  };
  downstream.forEach(collect);
  return {
    clients,
    routes: handlerRoutes.map((r) => ({ method: r.method, path: r.path, handler: r.handler, framework: r.framework })),
    handlers,
    downstream,
    writes: [...writes],
  };
}

export interface ErrorFlow {
  exception: string;
  /** Other functions that throw the same type. */
  alsoThrownBy: string[];
  /** Where this symbol's throw is caught (exact up the call chain, inferred to a global handler). */
  caughtBy: { id: string; confidence: Confidence; evidence: string | null }[];
}

/** For each exception `id` throws: who else throws it and where it is caught. */
export function errorFlows(index: StructureIndex, id: string): ErrorFlow[] {
  const thrown = index.graph.edges.filter((e) => e.kind === "Throws" && e.from === id).map((e) => e.to);
  const caught = index.graph.edges.filter((e) => e.kind === "CaughtBy" && e.from === id);
  return [...new Set(thrown)].map((ex) => ({
    exception: ex,
    alsoThrownBy: index.graph.edges.filter((e) => e.kind === "Throws" && e.to === ex && e.from !== id).map((e) => e.from),
    caughtBy: caught.map((e) => ({ id: e.to, confidence: e.confidence ?? "Exact", evidence: e.evidence ?? null })),
  }));
}

/** Declarations of a file in source order, with their nesting depth. */
export function fileSkeleton(index: StructureIndex, file: string): { symbol: Symbol; depth: number }[] {
  const inFile = index.graph.symbols.filter((s) => !s.external && s.kind !== "Module" && s.kind !== "Namespace" && s.origins.some((o) => o.file === file));
  const ids = new Set(inFile.map((s) => s.id));
  const depthOf = (s: Symbol): number => {
    let d = 0;
    for (let p = s.parent; p && ids.has(p); p = index.byId.get(p)?.parent ?? null) d++;
    return d;
  };
  const line = (s: Symbol) => s.origins.find((o) => o.file === file)?.start_line ?? 0;
  return inFile
    .sort((a, b) => line(a) - line(b))
    .map((symbol) => ({ symbol, depth: depthOf(symbol) }));
}

/** Text of a file skeleton, ready to copy. */
export function skeletonText(index: StructureIndex, file: string): string {
  return fileSkeleton(index, file)
    .map(({ symbol, depth }) => `${"  ".repeat(depth)}${symbol.signature ?? `${symbol.kind.toLowerCase()} ${symbol.name}`}  // :${symbol.origins[0]?.start_line ?? ""}`)
    .join("\n");
}

/** Bases and members (with signatures) of a type: its contract. */
export function typeContract(index: StructureIndex, id: string): { bases: string[]; members: Symbol[] } {
  const bases = index.graph.edges.filter((e) => (e.kind === "Inherits" || e.kind === "Implements") && e.from === id).map((e) => e.to);
  return { bases, members: index.members(id) };
}

/** Relations whose target the code shows but Nodex could not find. */
export function unresolvedEdges(index: StructureIndex): StructEdge[] {
  return index.graph.edges.filter((e) => e.confidence === "Unresolved");
}

/** Short display name: `OrderService.PlaceOrder` for members, the name otherwise. */
export function shortName(index: StructureIndex, id: string): string {
  const s = index.byId.get(id);
  if (!s) return id.replace(/^ext:/, "");
  const owner = s.parent ? index.byId.get(s.parent) : undefined;
  return owner && isTypeKind(owner.kind) && !isTypeKind(s.kind) ? `${owner.name}.${s.name}` : s.name;
}
