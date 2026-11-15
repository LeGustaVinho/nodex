import { BaseEdge, type EdgeProps } from "@xyflow/react";
import type { GEdge } from "../graph/model";

interface RelationData {
  offset: number;
}

export function ArchitectureRelation(edge: EdgeProps) {
  const graphEdge = edge.data?.g as GEdge<RelationData> | undefined;
  const offset = graphEdge?.data?.offset ?? 0;
  // Fan out relations with the same endpoints so every wire can be inspected.
  const { sourceX, sourceY, targetX, targetY } = edge;
  const dx = targetX - sourceX;
  const dy = targetY - sourceY;
  const length = Math.hypot(dx, dy) || 1;
  const midX = (sourceX + targetX) / 2;
  const midY = (sourceY + targetY) / 2;
  const bendX = -dy * offset / length;
  const bendY = dx * offset / length;
  return (
    <BaseEdge
      id={edge.id}
      path={`M ${sourceX} ${sourceY} Q ${midX + 2 * bendX} ${midY + 2 * bendY} ${targetX} ${targetY}`}
      label={edge.label}
      labelX={midX + bendX}
      labelY={midY + bendY}
      markerEnd={edge.markerEnd}
      style={edge.style}
    />
  );
}
