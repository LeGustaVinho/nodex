import { BaseEdge, type EdgeProps, getBezierPath } from "@xyflow/react";
import { memo } from "react";
import { blueprintTypeColor } from "../graph/blueprintTypes";
import type { Access } from "../gen/Access";
import type { TypeShape } from "../gen/TypeShape";

export interface OwnershipData {
  ty: string | null;
  shape?: TypeShape;
  access: Access;
}

/** What each wire style means (tooltips and the legend). */
export const ACCESS_TEXT: Record<Access, string> = {
  Move: "moves: the value goes here and the source can no longer use it",
  Copy: "copies: the source keeps its own value",
  Borrow: "borrows (&): reads the value, the owner keeps it",
  BorrowMut: "borrows to change (&mut): exclusive access while it is used",
  ByValue: "uses this value: Rust can copy, move or reborrow it depending on its type and how it is used",
};

export const ACCESS_LABEL: Record<Access, string> = {
  Move: "Transfer ownership",
  Copy: "Copy value",
  Borrow: "Shared borrow",
  BorrowMut: "Borrow to change",
  ByValue: "Value input",
};

/**
 * A data wire that shows Rust ownership: a move is a thick solid wire, a borrow a hollow wire
 * (outline only), a mutable borrow a double wire, a copy a thin wire.
 */
function OwnershipEdgeImpl({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data }: EdgeProps) {
  const { ty, shape, access } = data as unknown as OwnershipData;
  const [path] = getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition });
  const color = blueprintTypeColor(shape, ty);
  const bg = "var(--bg)";
  const title = <title>{ACCESS_TEXT[access]}</title>;
  switch (access) {
    case "Move":
      return (
        <g className="edge-own edge-move">
          {title}
          <BaseEdge path={path} style={{ stroke: color, strokeWidth: 2.8, opacity: 0.9 }} />
        </g>
      );
    case "Borrow":
      return (
        <g className="edge-own edge-borrow">
          {title}
          <BaseEdge path={path} style={{ stroke: color, strokeWidth: 3.6, opacity: 0.85 }} />
          <path d={path} fill="none" style={{ stroke: bg, strokeWidth: 1.6 }} />
        </g>
      );
    case "BorrowMut":
      return (
        <g className="edge-own edge-borrow-mut">
          {title}
          <BaseEdge path={path} style={{ stroke: color, strokeWidth: 5.2, opacity: 0.9 }} />
          <path d={path} fill="none" style={{ stroke: bg, strokeWidth: 1.8 }} />
        </g>
      );
    case "ByValue":
      return (
        <g className="edge-own">
          {title}
          <BaseEdge path={path} style={{ stroke: color, strokeWidth: 1.4, strokeDasharray: "3 3", opacity: 0.85 }} />
        </g>
      );
    case "Copy":
      return (
        <g className="edge-own edge-copy">
          {title}
          <BaseEdge path={path} style={{ stroke: color, strokeWidth: 1.4, opacity: 0.7 }} />
        </g>
      );
  }
}

export const OwnershipEdge = memo(OwnershipEdgeImpl);
