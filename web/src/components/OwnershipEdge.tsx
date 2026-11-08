import { BaseEdge, type EdgeProps, getBezierPath } from "@xyflow/react";
import { memo } from "react";
import { typeColor } from "../catalog";
import type { Access } from "../gen/Access";

export interface OwnershipData {
  ty: string | null;
  access: Access;
}

/** What each wire style means (tooltips and the legend). */
export const ACCESS_TEXT: Record<Access, string> = {
  Move: "moves: the value goes here and the source can no longer use it",
  Copy: "copies: the source keeps its own value",
  Borrow: "borrows (&): reads the value, the owner keeps it",
  BorrowMut: "borrows to change (&mut): exclusive access while it is used",
};

/**
 * A data wire that shows Rust ownership: a move is a thick solid wire, a borrow a hollow wire
 * (outline only), a mutable borrow a double wire, a copy a thin wire.
 */
function OwnershipEdgeImpl({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data }: EdgeProps) {
  const { ty, access } = data as unknown as OwnershipData;
  const [path] = getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition });
  const color = typeColor(ty);
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
    default:
      return (
        <g className="edge-own edge-copy">
          {title}
          <BaseEdge path={path} style={{ stroke: color, strokeWidth: 1.4, opacity: 0.7 }} />
        </g>
      );
  }
}

export const OwnershipEdge = memo(OwnershipEdgeImpl);
