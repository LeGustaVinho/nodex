import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import { conceptColor, conceptInfo, flavorLabel, useCatalog } from "../catalog";
import type { ExprData } from "../graph/flowGraph";

/** A pure node from an expanded expression tree. */
function ExprNodeImpl({ data, selected }: NodeProps) {
  const { expr, language } = data as unknown as ExprData;
  const catalog = useCatalog();
  const info = conceptInfo(catalog, expr.concept);
  const color = conceptColor(catalog, expr.concept);
  const rows = Math.max(expr.children.length, 1);
  return (
    <div className={`expr ${selected ? "selected" : ""}`} style={{ "--accent": color } as React.CSSProperties}>
      <div className="expr-head">
        {/* In expressions a Var is a read of an existing value, not a declaration. */}
        <span>ƒ {expr.concept === "Var" ? "Read" : info.title}</span>
        {expr.flavors
          .filter((f) => f !== "stage")
          .map((f) => (
            <span key={f} className="badge small">
              {flavorLabel(catalog, f, language)}
            </span>
          ))}
      </div>
      <div className="expr-label" title={expr.target ?? undefined}>
        {expr.label}
      </div>
      {expr.target && <div className="expr-target">→ {expr.target}</div>}
      <div className="expr-pins" style={{ height: rows * 16 }}>
        {expr.children.map((_, i) => (
          <Handle
            key={i}
            type="target"
            position={Position.Left}
            id={`c${i}`}
            className="handle-data"
            style={{ top: 8 + i * 16, "--pin": "#9aa4b2" } as React.CSSProperties}
            isConnectable={false}
          />
        ))}
      </div>
      <Handle type="source" position={Position.Right} id="out" className="handle-data" style={{ "--pin": "#9aa4b2" } as React.CSSProperties} isConnectable={false} />
    </div>
  );
}

export const ExprNode = memo(ExprNodeImpl);
