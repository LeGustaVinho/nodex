import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import type { CallNodeData } from "../graph/callGraph";
import { useUi } from "../store";

const ICON: Record<string, string> = { Event: "⚡", Constructor: "✦", Property: "◈", Method: "ƒ", Function: "ƒ" };

/** A function (or event) in the call graph. */
function CallNodeImpl({ data, selected }: NodeProps) {
  const { symbol, owner, focus } = data as unknown as CallNodeData;
  const openFlow = useUi((s) => s.openFlow);
  return (
    <div className={`callnode kind-${symbol.kind.toLowerCase()} ${focus ? "focus" : ""} ${selected ? "selected" : ""}`}>
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <div className="callnode-owner">{owner ?? (symbol.external ? "external" : "")}</div>
      <div className="callnode-name">
        <span className="member-icon">{ICON[symbol.kind] ?? "ƒ"}</span>
        <span className="callnode-title">{symbol.name}</span>
        {symbol.has_flow && (
          <button
            className="callnode-open nodrag"
            title="Open Blueprint"
            onClick={(e) => {
              e.stopPropagation();
              openFlow(symbol.id);
            }}
          >
            ▶
          </button>
        )}
      </div>
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
}

export const CallNode = memo(CallNodeImpl);
