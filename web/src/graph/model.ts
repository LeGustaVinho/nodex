/**
 * A renderer-neutral graph model. Views build a `GGraph`, ELK lays it out, and the canvas maps
 * it to React Flow. Keeping this layer pure makes the transforms unit-testable.
 */

export type Side = "WEST" | "EAST";

export interface GPort {
  id: string;
  side: Side;
}

export interface GNode<D = unknown> {
  id: string;
  type: string;
  data: D;
  width: number;
  height: number;
  ports: GPort[];
  /** Space reserved in the layout (when larger than the node itself); the node sits at `offsetX`. */
  box?: { width: number; height: number; offsetX: number };
  /** Nodes placed relative to another node instead of by the layout engine. */
  attach?: { to: string; dx: number; dy: number };
  /** Enclosing container node: the node is laid out and drawn inside it. */
  parent?: string;
  /** A container whose size comes from the layout of its children. */
  container?: boolean;
}

export interface GEdge<D = unknown> {
  id: string;
  source: string;
  target: string;
  sourceHandle?: string;
  targetHandle?: string;
  /** Visual kind: exec, data, expr, inherits, implements, uses, imports, ... */
  kind: string;
  /** Edges excluded from layout are drawn but do not influence positions. */
  layout: boolean;
  /** Knowledge relations no compiler proves: drawn dashed (inferred) or dotted red (unresolved). */
  confidence?: "Exact" | "Inferred" | "Unresolved" | null;
  /** How many relations a rolled-up edge stands for (drawn thicker, with a count). */
  weight?: number;
  data?: D;
}

export interface GGraph {
  nodes: GNode[];
  edges: GEdge[];
  direction: "RIGHT" | "DOWN";
  /** Width/height ratio ELK aims for when it packs unconnected parts side by side. */
  aspectRatio?: number;
  /** How to arrange the nodes (default: ELK's layered algorithm). */
  layout?: "layered" | "stress" | "layers" | "concentric" | "circle" | "grid";
  /** Node the `layers` and `concentric` layouts start from. */
  focus?: string | null;
  /** Multiplies every gap (compact 0.7, normal 1, wide 1.6). */
  spacing?: number;
}

/** Rough text width-based line count, used to estimate node heights before rendering. */
export function lines(text: string, charsPerLine: number): number {
  return Math.max(1, Math.ceil(text.length / charsPerLine));
}
