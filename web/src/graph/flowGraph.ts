import type { Expr } from "../gen/Expr";
import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import type { Language } from "../gen/Language";
import type { OperationInfo } from "../gen/OperationInfo";
import { type GEdge, type GGraph, type GNode, type GPort, lines } from "./model";

export const BLUEPRINT_WIDTH = 300;
export const EXPR_WIDTH = 190;
export const PIN_ROW = 22;
const EXPR_PIN_ROW = 18;
const MAX_EXPR_NODES = 80;

/**
 * How much of each statement is drawn:
 * - `statements`: one node per statement; expression trees open on demand (ADR-007);
 * - `operations`: every statement's operations as Blueprint nodes, constants inside the pins and
 *   local variables wired from where they are set;
 * - `everything`: operations, plus the bodies of inline functions (lambdas) with their parameters.
 */
export type DetailLevel = "statements" | "operations" | "everything";

export const DETAIL_LEVELS: { id: DetailLevel; label: string; explain: string }[] = [
  { id: "statements", label: "Statements", explain: "One node per statement; open a statement's details with its + button." },
  { id: "operations", label: "Operations", explain: "Every operation (add, compare, upper case, maximum...) as its own node, with values in the pins." },
  { id: "everything", label: "Everything", explain: "Operations, plus the bodies of the small functions written inline (lambdas)." },
];

/** One input pin of an expression node (or of a statement in the compact levels). */
export interface ExprPin {
  id: string;
  /** Pin name (`A`, `Text`, `Separator`); null when the operation has no names for it. */
  name: string | null;
  ty: string | null;
  /** A constant or a name shown inside the pin, instead of a wire. */
  value: string | null;
}

export interface BlueprintData {
  node: FlowNode;
  flow: string;
  language: Language;
  expanded: boolean;
  /** Compact levels: the statement's own call/assignment arguments, drawn as its input pins. */
  argPins?: ExprPin[];
  /** Inputs wired straight into the statement's expression tree instead (compact levels). */
  hiddenInputs?: string[];
  /** Whether the statement has details to open or close (its + / − button). */
  canExpand?: boolean;
}

export interface ExprData {
  expr: Expr;
  ownerId: string;
  path: string;
  flow: string;
  language: Language;
  pins: ExprPin[];
  /** Compact levels: no code label for recognized operations; values inside pins. */
  compact: boolean;
  /** Everything level: the lambda's parameters, drawn as output pins. */
  params: string[];
}

export interface FlowGraphOptions {
  level?: DetailLevel;
  /** Catalog operations, for pin names. */
  operations?: Record<string, OperationInfo>;
}

export function blueprintHeight(node: FlowNode, expanded: boolean, argPins = 0): number {
  const detailsPin = expanded && argPins === 0 ? 1 : 0;
  const rows = Math.max(node.inputs.length + detailsPin + argPins, node.outputs.length, 1);
  const label = lines(node.label, 36) * 17;
  const badges = node.flavors.length > 0 ? 24 : 0;
  return 30 + 10 + label + rows * PIN_ROW + badges + 26;
}

/**
 * Statements that *are* their expression (a call statement, an assignment): in the compact levels
 * the expression's inputs become the statement's own pins. A `for each` over `repo.all()` is not
 * the call, so its expression stays a tree feeding the details pin.
 */
function absorbs(node: FlowNode): boolean {
  const root = node.expr?.concept;
  if (!root) return false;
  if (root === node.concept) return ["Call", "Assign", "Construct", "Await", "Throw", "Emit"].includes(root);
  return (root === "Call" && ["Emit", "Spawn"].includes(node.concept)) || (root === "Assign" && node.concept === "SetMember");
}

/** Pin names for concepts that are not catalog operations. */
const CONCEPT_PINS: Record<string, string[]> = {
  Choose: ["If", "Then", "Else"],
  Index: ["Of", "At"],
  Fallback: ["Value", "Otherwise"],
  Assign: ["Target", "Value"],
  TypeCheck: ["Value"],
  Convert: ["Value"],
  Await: ["Task"],
  Throw: ["Error"],
};

/** Converts a flow graph to the neutral model. Expanded statements show their expression trees. */
export function flowToGraph(flow: FlowGraph, expanded: Record<string, boolean>, showDataWires: boolean, opts: FlowGraphOptions = {}): GGraph {
  const level = opts.level ?? "statements";
  const compact = level !== "statements";
  const nodes: GNode[] = [];
  const edges: GEdge[] = [];
  /** `nodeId use:x` data edges replaced by wires into the node's expression tree. */
  const redirected = new Set<string>();

  for (const node of flow.nodes) {
    // A statement whose whole expression is a constant or a variable has nothing to open.
    const trivial = compact && node.expr !== null && (node.expr.concept === "Literal" || node.expr.concept === "Var");
    const isExpanded = node.expr !== null && !trivial && (compact ? expanded[node.id] !== false : !!expanded[node.id]);
    const ctx: TreeCtx = {
      flow,
      ownerId: node.id,
      level,
      operations: opts.operations ?? {},
      locals: compact ? localsOf(flow, node.id) : new Map(),
      nodes,
      edges,
      redirected,
      count: 0,
      showDataWires,
    };
    const absorb = compact && isExpanded && absorbs(node);
    const argPins = absorb && node.expr ? pinsOf(node.expr, node.id, ctx, "x") : undefined;
    const tree =
      isExpanded && node.expr
        ? absorb
          ? addForest(argPins!.subtrees, node.id, ctx)
          : addExprTree(node.expr, node.id, { id: node.id, handle: "expr" }, "0", ctx)
        : undefined;
    // Inputs now wired straight into the expression tree are not repeated on the statement.
    const hidden = node.inputs.filter((p) => redirected.has(`${node.id} ${p.id}`)).map((p) => p.id);
    const shown = node.inputs.filter((p) => !hidden.includes(p.id));
    const ports: GPort[] = [];
    for (const p of shown) ports.push({ id: p.id, side: "WEST" });
    if (isExpanded && !absorb) ports.push({ id: "expr", side: "WEST" });
    for (const p of argPins?.pins ?? []) ports.push({ id: p.id, side: "WEST" });
    for (const p of node.outputs) ports.push({ id: p.id, side: "EAST" });
    const height = blueprintHeight({ ...node, inputs: shown }, isExpanded, argPins?.pins.length ?? 0);
    const gnode: GNode = {
      id: node.id,
      type: "blueprint",
      data: { node, flow: flow.symbol, language: flow.language, expanded: isExpanded, argPins: argPins?.pins, hiddenInputs: hidden, canExpand: node.expr !== null && !trivial } satisfies BlueprintData,
      width: BLUEPRINT_WIDTH,
      height,
      ports,
    };
    nodes.push(gnode);
    if (!tree || tree.placed.length === 0) continue;
    // Reserve room below-left of the statement for its expression tree, so the tree feeds the
    // statement's pins from the left without crossing the exec wire on top.
    gnode.box = { width: tree.width + BLUEPRINT_WIDTH, height: height + TREE_GAP + tree.height, offsetX: tree.width };
    for (const n of tree.placed) {
      n.attach = { to: node.id, dx: n.attach!.dx - tree.width, dy: height + TREE_GAP + n.attach!.dy };
    }
  }

  for (const e of flow.edges) {
    const isExec = e.kind === "Exec";
    if (!isExec && !showDataWires) continue;
    if (!isExec && redirected.has(`${e.to} ${e.to_pin}`)) continue;
    const ty = isExec ? null : (flow.nodes.find((n) => n.id === e.from)?.outputs.find((p) => p.id === e.from_pin)?.ty ?? null);
    const access = isExec ? null : (e.access ?? null);
    edges.push({
      id: `${e.from}:${e.from_pin}->${e.to}:${e.to_pin}`,
      source: e.from,
      sourceHandle: e.from_pin,
      target: e.to,
      targetHandle: e.to_pin,
      kind: isExec ? "exec" : "data",
      layout: isExec,
      data: { ty, access },
    });
  }
  return { nodes, edges, direction: "RIGHT" };
}

const TREE_GAP = 28;
const TREE_COL = EXPR_WIDTH + 44;
const TREE_ROW = 78;

interface TreeCtx {
  flow: FlowGraph;
  ownerId: string;
  level: DetailLevel;
  operations: Record<string, OperationInfo>;
  /** Local name → the pin that defines it (`var:x` of an earlier statement or the entry). */
  locals: Map<string, { from: string; pin: string; ty: string | null; access: string | null }>;
  nodes: GNode[];
  edges: GEdge[];
  redirected: Set<string>;
  count: number;
  showDataWires: boolean;
}

/** Locals a statement reads, from its `use:x` data edges. */
function localsOf(flow: FlowGraph, nodeId: string): Map<string, { from: string; pin: string; ty: string | null; access: string | null }> {
  const out = new Map<string, { from: string; pin: string; ty: string | null; access: string | null }>();
  for (const e of flow.edges) {
    if (e.kind !== "Data" || e.to !== nodeId || !e.to_pin.startsWith("use:")) continue;
    const ty = flow.nodes.find((n) => n.id === e.from)?.outputs.find((p) => p.id === e.from_pin)?.ty ?? null;
    out.set(e.to_pin.slice(4), { from: e.from, pin: e.from_pin, ty, access: e.access ?? null });
  }
  return out;
}

/** Catalog pin `Name:type` → its parts. */
function splitPin(pin: string | undefined): { name: string | null; ty: string | null } {
  if (!pin) return { name: null, ty: null };
  const i = pin.indexOf(":");
  return i < 0 ? { name: pin, ty: null } : { name: pin.slice(0, i), ty: pin.slice(i + 1) };
}

/** The operation's inputs, without a static receiver (`Math` in `Math.max(a, b)`). */
function operands(expr: Expr, op: OperationInfo | undefined): Expr[] {
  const first = expr.children[0];
  if (op && first?.flavors.includes("target") && op.inputs.length < expr.children.length) return expr.children.slice(1);
  return expr.children;
}

interface Subtree {
  expr: Expr;
  path: string;
  pin: string;
}

/** Pins of `expr` in the compact levels: constants and names inline, locals wired, the rest as subtrees. */
function pinsOf(expr: Expr, id: string, ctx: TreeCtx, prefix = "c", path = "0"): { pins: ExprPin[]; subtrees: Subtree[] } {
  const op = expr.op ? ctx.operations[expr.op] : undefined;
  const kids = operands(expr, op);
  // An assignment keeps Target/Value even when it applies an operation (`total += x`).
  const names = op && expr.concept !== "Assign" ? op.inputs : (CONCEPT_PINS[expr.concept] ?? []).map((n) => `${n}:`);
  const pins: ExprPin[] = [];
  const subtrees: Subtree[] = [];
  kids.forEach((child, i) => {
    const pinId = `${prefix}${i}`;
    const childPath = `${path}.${expr.children.indexOf(child)}`;
    const named = splitPin(names[Math.min(i, names.length - 1)]);
    let name = named.name || null;
    if (!name && child.flavors.includes("target")) name = "Target";
    const pin: ExprPin = { id: pinId, name, ty: named.ty || child.ty, value: null };
    pins.push(pin);
    if (ctx.level === "statements") {
      subtrees.push({ expr: child, path: childPath, pin: pinId });
      return;
    }
    if (child.concept === "Literal") {
      pin.value = child.label;
      pin.ty = pin.ty ?? child.ty;
      return;
    }
    if (child.concept === "Var") {
      const param = lambdaParam(ctx, child.label);
      if (param) {
        ctx.edges.push({
          id: `${param}:p:${child.label}->${id}:${pinId}`,
          source: param,
          sourceHandle: `p:${child.label}`,
          target: id,
          targetHandle: pinId,
          kind: "data",
          layout: false,
          data: { ty: null },
        });
        return;
      }
      const local = ctx.locals.get(child.label);
      if (local) {
        pin.ty = pin.ty ?? local.ty;
        ctx.redirected.add(`${ctx.ownerId} use:${child.label}`);
        if (ctx.showDataWires) {
          ctx.edges.push({
            id: `${local.from}:${local.pin}->${id}:${pinId}`,
            source: local.from,
            sourceHandle: local.pin,
            target: id,
            targetHandle: pinId,
            kind: "data",
            layout: false,
            data: { ty: local.ty, access: local.access },
          });
        }
        return;
      }
      pin.value = child.label;
      return;
    }
    subtrees.push({ expr: child, path: childPath, pin: pinId });
  });
  return { pins, subtrees };
}

/** Lambda parameters in scope while a lambda body is visited (Everything level): name → lambda node id. */
const lambdaScopes = new WeakMap<TreeCtx, Map<string, string>[]>();

function lambdaParam(ctx: TreeCtx, name: string): string | undefined {
  const scopes = lambdaScopes.get(ctx) ?? [];
  for (let i = scopes.length - 1; i >= 0; i--) {
    const hit = scopes[i].get(name);
    if (hit) return hit;
  }
  return undefined;
}

/** Several subtrees feeding one node (the statement's argument pins), stacked vertically. */
function addForest(subtrees: Subtree[], ownerId: string, ctx: TreeCtx) {
  const placed: GNode[] = [];
  let width = 0;
  let height = 0;
  for (const s of subtrees) {
    const tree = addExprTree(s.expr, ownerId, { id: ownerId, handle: s.pin }, s.path, ctx);
    for (const n of tree.placed) n.attach!.dy += height;
    placed.push(...tree.placed);
    width = Math.max(width, tree.width);
    height += tree.height;
  }
  // Right-align the trees: each root sits in the column next to the statement.
  for (const s of placed) {
    const own = (s as GNode & { treeWidth?: number }).treeWidth ?? width;
    s.attach!.dx += width - own;
  }
  return { width, height, placed };
}

/**
 * Adds an expression tree as pure nodes. Positions are relative: the root sits in the right-most
 * column (next to its statement) and children fan out to the left, like data inputs in a
 * Blueprint. Returns the tree's size and the placed nodes.
 */
function addExprTree(root: Expr, ownerId: string, target: { id: string; handle: string }, rootPath: string, ctx: TreeCtx) {
  let leaves = 0;
  let maxDepth = 0;
  const placed: GNode[] = [];
  const visit = (expr: Expr, path: string, depth: number, parent: { id: string; handle: string }): number => {
    ctx.count++;
    maxDepth = Math.max(maxDepth, depth);
    const id = `${ownerId}/e/${path}`;
    const op = expr.op ? ctx.operations[expr.op] : undefined;
    const isLambda = expr.concept === "Lambda";
    const expandLambda = isLambda && ctx.level === "everything" && expr.children.length > 0;
    const { pins, subtrees } =
      isLambda && !expandLambda
        ? { pins: [], subtrees: [] }
        : expandLambda
          ? { pins: [{ id: "c0", name: "Result", ty: null, value: null }], subtrees: expr.children.slice(0, 1).map((c) => ({ expr: c, path: `${path}.0`, pin: "c0" })) }
          : pinsOf(expr, id, ctx, "c", path);
    const params = expandLambda ? expr.params : [];
    const ports: GPort[] = pins.map((p) => ({ id: p.id, side: "WEST" as const }));
    ports.push({ id: "out", side: "EAST" });
    for (const p of params) ports.push({ id: `p:${p}`, side: "EAST" });
    const compact = ctx.level !== "statements";
    const showLabel = !(compact && op && expr.concept === "Operation");
    const rows = Math.max(pins.length, 1) + params.length;
    const gnode: GNode & { treeWidth?: number } = {
      id,
      type: "expr",
      data: { expr, ownerId, path, flow: ctx.flow.symbol, language: ctx.flow.language, pins, compact, params } satisfies ExprData,
      width: EXPR_WIDTH,
      height: 34 + (showLabel ? lines(expr.label, 24) * 15 : 0) + (expr.target && !compact ? 15 : 0) + rows * EXPR_PIN_ROW,
      ports,
      attach: { to: ownerId, dx: depth, dy: 0 },
    };
    ctx.nodes.push(gnode);
    placed.push(gnode);
    ctx.edges.push({
      id: `${id}->${parent.id}:${parent.handle}`,
      source: id,
      sourceHandle: "out",
      target: parent.id,
      targetHandle: parent.handle,
      kind: "expr",
      layout: false,
      data: { ty: expr.ty },
    });
    if (expandLambda) {
      const scopes = lambdaScopes.get(ctx) ?? [];
      scopes.push(new Map(params.map((p) => [p, id])));
      lambdaScopes.set(ctx, scopes);
    }
    const kids = ctx.count < MAX_EXPR_NODES ? subtrees : [];
    let y: number;
    if (kids.length === 0) {
      y = leaves * TREE_ROW;
      leaves++;
    } else {
      const ys = kids.map((s) => visit(s.expr, s.path, depth + 1, { id, handle: s.pin }));
      y = (ys[0] + ys[ys.length - 1]) / 2;
    }
    if (expandLambda) lambdaScopes.get(ctx)?.pop();
    gnode.attach!.dy = y;
    return y;
  };
  visit(root, rootPath, 0, target);
  const width = (maxDepth + 1) * TREE_COL;
  for (const n of placed) {
    // depth → x: depth 0 is the right-most column.
    n.attach!.dx = width - (n.attach!.dx + 1) * TREE_COL;
    (n as GNode & { treeWidth?: number }).treeWidth = width;
  }
  return { width, height: Math.max(leaves, 1) * TREE_ROW, placed };
}
