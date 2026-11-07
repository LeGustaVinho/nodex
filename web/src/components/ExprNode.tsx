import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import { conceptColor, conceptInfo, flavorLabel, operationColor, operationInfo, typeColor, useCatalog } from "../catalog";
import type { ExprData } from "../graph/flowGraph";

const PIN = 18;
/** Flavors that say nothing new on an operation node. */
const QUIET = new Set(["stage", "target", "loose-equality", "truthiness", "prefix", "postfix"]);

/** A pure node from an expression tree: an operation (`≥ Greater or equal`) or another expression. */
function ExprNodeImpl({ data, selected }: NodeProps) {
  const { expr, language, pins, compact, params } = data as unknown as ExprData;
  const catalog = useCatalog();
  const op = operationInfo(catalog, expr.op);
  const info = conceptInfo(catalog, expr.concept);
  const color = op ? operationColor(catalog, op) : conceptColor(catalog, expr.concept);
  // In expressions a Var is a read of an existing value, not a declaration.
  const title = op ? op.title : expr.concept === "Var" ? "Read" : expr.concept === "GetMember" ? "Get" : info.title;
  const symbol = op ? op.symbol : "ƒ";
  const showLabel = !(compact && op && expr.concept === "Operation");
  const rows = Math.max(pins.length, 1);
  const outColor = typeColor(expr.ty);
  const flavors = expr.flavors.filter((f) => !QUIET.has(f));
  return (
    <div className={`expr ${op ? "expr-op" : ""} ${selected ? "selected" : ""}`} style={{ "--accent": color } as React.CSSProperties}>
      <div className="expr-head" title={op?.explain ?? info.explain}>
        <span className="expr-symbol">{symbol}</span>
        <span className="expr-title">{title}</span>
        {(expr.op === "and" || expr.op === "or") && (
          <span className="badge small" title="The right side only runs when it can change the result">
            short-circuit
          </span>
        )}
        {flavors.map((f) => (
          <span key={f} className="badge small">
            {flavorLabel(catalog, f, language)}
          </span>
        ))}
      </div>
      {showLabel && (
        <div className="expr-label" title={expr.target ?? undefined}>
          {expr.label}
        </div>
      )}
      {expr.target && !compact && <div className="expr-target">→ {expr.target}</div>}
      <div className="expr-pins" style={{ height: rows * PIN + params.length * PIN }}>
        {pins.map((p, i) => (
          <div key={p.id} className="expr-pin" style={{ top: i * PIN }} title={p.ty ? `${p.name ?? "input"}: ${p.ty}` : undefined}>
            <Handle
              type="target"
              position={Position.Left}
              id={p.id}
              className="handle-data"
              style={{ top: PIN / 2, "--pin": typeColor(p.ty) } as React.CSSProperties}
              isConnectable={false}
            />
            {p.name && <span className="expr-pin-name">{p.name}</span>}
            {p.value !== null && (
              <span className="expr-pin-value" title={p.value}>
                {p.value}
              </span>
            )}
          </div>
        ))}
        {params.map((name, i) => (
          <div key={name} className="expr-pin expr-param" style={{ top: (rows + i) * PIN }} title="A parameter: the value the caller passes in">
            <span className="expr-pin-name">{name}</span>
            <Handle
              type="source"
              position={Position.Right}
              id={`p:${name}`}
              className="handle-data"
              style={{ top: PIN / 2, "--pin": "#9aa4b2" } as React.CSSProperties}
              isConnectable={false}
            />
          </div>
        ))}
      </div>
      <Handle type="source" position={Position.Right} id="out" className="handle-data" style={{ "--pin": outColor } as React.CSSProperties} isConnectable={false} />
      {op?.output && <span className="expr-out-name">{op.output.split(":")[0]}</span>}
    </div>
  );
}

export const ExprNode = memo(ExprNodeImpl);
