import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import type { CallNodeData } from "../graph/callGraph";
import { useUi } from "../store";
import { useTrace } from "../trace";

const ICON: Record<string, string> = { Event: "⚡", Constructor: "✦", Property: "◈", Method: "ƒ", Function: "ƒ" };

/** A function (or event) in the call graph. */
function CallNodeImpl({ data, selected }: NodeProps) {
  const { symbol, owner, focus } = data as unknown as CallNodeData;
  const openFlow = useUi((s) => s.openFlow);
  const calls = useTrace((s) => s.calls[symbol.id]);
  const active = useTrace((s) => s.active.some((a) => a.flow === symbol.id));
  return (
    <div className={`callnode kind-${symbol.kind.toLowerCase()} ${focus ? "focus" : ""} ${selected ? "selected" : ""} ${active ? "active" : ""}`}>
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <div className="callnode-owner">{owner ?? (symbol.external ? "external" : "")}</div>
      <div className="callnode-name">
        <span className="member-icon">{ICON[symbol.kind] ?? "ƒ"}</span>
        <span className="callnode-title">{symbol.name}</span>
        {calls !== undefined && <span className="bp-count" title={`called ${calls} times`}>×{calls}</span>}
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
