import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import type { ArchitectureNode } from "../gen/ArchitectureNode";

export interface ArchitectureNodeData {
  node: ArchitectureNode;
  boundary: boolean;
  vertical: boolean;
  action?: { label: string; expanded?: boolean; run: () => void };
}

export const ArchitectureCard = memo(function ArchitectureCard({ data, selected }: NodeProps) {
  const { node, boundary, vertical, action } = data as unknown as ArchitectureNodeData;
  const kind = node.kind === "Store" ? "Data store" : node.kind === "External" ? "External system" : node.kind;
  return (
    <div className={["arch-box", "arch-" + node.kind.toLowerCase(), boundary ? "arch-boundary" : "", selected ? "selected" : ""].join(" ")}>
      <Handle type="target" position={vertical ? Position.Top : Position.Left} isConnectable={false} />
      <div className="arch-box-header">
        <div className="arch-box-text">
          <strong title={node.name}>{node.name}</strong>
          {node.kind !== "Layer" && <span>{kind}{node.technology && " · " + node.technology}</span>}
        </div>
        {action && (
          <button
            className="nodrag"
            title={action.label}
            aria-label={action.label}
            aria-expanded={action.expanded}
            onClick={(event) => { event.stopPropagation(); action.run(); }}
          >
            {action.expanded === true ? "−" : action.expanded === false ? "+" : "↗"}
          </button>
        )}
      </div>
      <Handle type="source" position={vertical ? Position.Bottom : Position.Right} isConnectable={false} />
    </div>
  );
});
