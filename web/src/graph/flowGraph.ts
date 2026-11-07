import type { Expr } from "../gen/Expr";
import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import type { Language } from "../gen/Language";
import { type GEdge, type GGraph, type GNode, type GPort, lines } from "./model";

export const BLUEPRINT_WIDTH = 300;
export const EXPR_WIDTH = 190;
export const PIN_ROW = 22;
const MAX_EXPR_NODES = 80;

export interface BlueprintData {
  node: FlowNode;
  flow: string;
  language: Language;
  expanded: boolean;
}

export interface ExprData {
  expr: Expr;
  ownerId: string;
  path: string;
  flow: string;
  language: Language;
}

export function blueprintHeight(node: FlowNode, expanded: boolean): number {
  const rows = Math.max(node.inputs.length + (expanded ? 1 : 0), node.outputs.length, 1);
  const label = lines(node.label, 36) * 17;
  const badges = node.flavors.length > 0 ? 24 : 0;
  return 30 + 10 + label + rows * PIN_ROW + badges + 26;
}

/** Converts a flow graph to the neutral model. Expanded statements show their expression trees. */
export function flowToGraph(flow: FlowGraph, expanded: Record<string, boolean>, showDataWires: boolean): GGraph {
  const nodes: GNode[] = [];
  const edges: GEdge[] = [];

  for (const node of flow.nodes) {
    const isExpanded = !!expanded[node.id] && node.expr !== null;
    const ports: GPort[] = [];
    for (const p of node.inputs) ports.push({ id: p.id, side: "WEST" });
    if (isExpanded) ports.push({ id: "expr", side: "WEST" });
    for (const p of node.outputs) ports.push({ id: p.id, side: "EAST" });
    const height = blueprintHeight(node, isExpanded);
    const gnode: GNode = {
      id: node.id,
      type: "blueprint",
      data: { node, flow: flow.symbol, language: flow.language, expanded: isExpanded } satisfies BlueprintData,
      width: BLUEPRINT_WIDTH,
      height,
      ports,
    };
    nodes.push(gnode);
    if (isExpanded && node.expr) {
      const tree = addExprTree(node.expr, node.id, flow, nodes, edges);
      // Reserve room below-left of the statement for its expression tree, so the tree feeds the
      // statement's "details" pin from the left without crossing the exec wire on top.
      gnode.box = { width: tree.width + BLUEPRINT_WIDTH, height: height + TREE_GAP + tree.height, offsetX: tree.width };
      for (const n of tree.placed) {
        n.attach = { to: node.id, dx: n.attach!.dx - tree.width, dy: height + TREE_GAP + n.attach!.dy };
      }
    }
  }

  for (const e of flow.edges) {
    const isExec = e.kind === "Exec";
    if (!isExec && !showDataWires) continue;
    const ty = isExec ? null : (flow.nodes.find((n) => n.id === e.from)?.outputs.find((p) => p.id === e.from_pin)?.ty ?? null);
    edges.push({
      id: `${e.from}:${e.from_pin}->${e.to}:${e.to_pin}`,
      source: e.from,
      sourceHandle: e.from_pin,
      target: e.to,
      targetHandle: e.to_pin,
      kind: isExec ? "exec" : "data",
      layout: isExec,
      data: { ty },
    });
  }
  return { nodes, edges, direction: "RIGHT" };
}

const TREE_GAP = 28;
const TREE_COL = EXPR_WIDTH + 44;
const TREE_ROW = 78;

/**
 * Adds the expression tree as pure nodes. Positions are relative: the root sits in the
 * right-most column (next to its statement) and children fan out to the left, like data
 * inputs in a Blueprint. Returns the tree's size and the placed nodes.
 */
function addExprTree(root: Expr, ownerId: string, flow: FlowGraph, nodes: GNode[], edges: GEdge[]) {
  let count = 0;
  let leaves = 0;
  let maxDepth = 0;
  const placed: GNode[] = [];
  const visit = (expr: Expr, path: string, depth: number, parent: { id: string; handle: string }): number => {
    count++;
    maxDepth = Math.max(maxDepth, depth);
    const id = `${ownerId}/e/${path}`;
    const ports: GPort[] = expr.children.map((_, i) => ({ id: `c${i}`, side: "WEST" as const }));
    ports.push({ id: "out", side: "EAST" });
    const gnode: GNode = {
      id,
      type: "expr",
      data: { expr, ownerId, path, flow: flow.symbol, language: flow.language } satisfies ExprData,
      width: EXPR_WIDTH,
      height: 34 + lines(expr.label, 24) * 15 + Math.max(expr.children.length, 1) * 16,
      ports,
      attach: { to: ownerId, dx: depth, dy: 0 },
    };
    nodes.push(gnode);
    placed.push(gnode);
    edges.push({
      id: `${id}->${parent.id}:${parent.handle}`,
      source: id,
      sourceHandle: "out",
      target: parent.id,
      targetHandle: parent.handle,
      kind: "expr",
      layout: false,
    });
    const kids = count < MAX_EXPR_NODES ? expr.children : [];
    let y: number;
    if (kids.length === 0) {
      y = leaves * TREE_ROW;
      leaves++;
    } else {
      const ys = kids.map((child, i) => visit(child, `${path}.${i}`, depth + 1, { id, handle: `c${i}` }));
      y = (ys[0] + ys[ys.length - 1]) / 2;
    }
    gnode.attach!.dy = y;
    return y;
  };
  visit(root, "0", 0, { id: ownerId, handle: "expr" });
  const width = (maxDepth + 1) * TREE_COL;
  for (const n of placed) {
    // depth → x: depth 0 is the right-most column.
    n.attach!.dx = width - (n.attach!.dx + 1) * TREE_COL;
  }
  return { width, height: Math.max(leaves, 1) * TREE_ROW, placed };
}
