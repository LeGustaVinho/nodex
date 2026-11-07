import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import { SHAPE_ICON, conceptColor, conceptInfo, flavorLabel, typeColor, useCatalog } from "../catalog";
import type { Pin } from "../gen/Pin";
import type { BlueprintData } from "../graph/flowGraph";
import { useUi } from "../store";

function PinView({ pin, side }: { pin: Pin; side: "in" | "out" }) {
  const exec = pin.kind === "Exec";
  const color = exec ? "#e8e8e8" : typeColor(pin.ty);
  return (
    <div className={`pin pin-${side} ${exec ? "pin-exec" : "pin-data"}`} title={pin.ty ? `${pin.label}: ${pin.ty}` : pin.label}>
      <Handle
        type={side === "in" ? "target" : "source"}
        position={side === "in" ? Position.Left : Position.Right}
        id={pin.id}
        className={exec ? "handle-exec" : "handle-data"}
        style={{ "--pin": color } as React.CSSProperties}
        isConnectable={false}
      />
      <span className="pin-label">{pin.label}</span>
      {!exec && pin.ty && <span className="pin-type">{pin.ty}</span>}
    </div>
  );
}

const EXPR_PIN: Pin = { id: "expr", kind: "Data", label: "details", ty: null };

function BlueprintNodeImpl({ data, selected }: NodeProps) {
  const { node, expanded, language } = data as unknown as BlueprintData;
  const catalog = useCatalog();
  const toggle = useUi((s) => s.toggleExpanded);
  const info = conceptInfo(catalog, node.concept);
  const color = conceptColor(catalog, node.concept);
  const inputs = expanded ? [...node.inputs, EXPR_PIN] : node.inputs;
  const rows = Math.max(inputs.length, node.outputs.length);
  const unreachable = node.flavors.includes("unreachable");
  const visibleFlavors = node.flavors.filter((f) => f !== "unreachable");

  return (
    <div
      className={`bp shape-${info.shape} ${selected ? "selected" : ""} ${unreachable ? "unreachable" : ""}`}
      style={{ "--accent": color } as React.CSSProperties}
    >
      <div className="bp-head">
        <span className="bp-icon">{SHAPE_ICON[info.shape] ?? "▸"}</span>
        <span className="bp-title">{info.title}</span>
        {node.expr && (
          <button
            className="bp-expand nodrag"
            title={expanded ? "Hide expression details" : "Show expression details"}
            onClick={(e) => {
              e.stopPropagation();
              toggle(node.id);
            }}
          >
            {expanded ? "−" : "+"}
          </button>
        )}
      </div>
      <div className="bp-label">{node.label}</div>
      <div className="bp-pins">
        {Array.from({ length: rows }, (_, i) => (
          <div className="pin-row" key={i}>
            <div className="pin-cell">{inputs[i] && <PinView pin={inputs[i]} side="in" />}</div>
            <div className="pin-cell right">{node.outputs[i] && <PinView pin={node.outputs[i]} side="out" />}</div>
          </div>
        ))}
      </div>
      {visibleFlavors.length > 0 && (
        <div className="badges">
          {visibleFlavors.map((f) => (
            <span key={f} className={`badge ${f.startsWith("unity:") ? "badge-unity" : ""}`}>
              {flavorLabel(catalog, f, language)}
            </span>
          ))}
        </div>
      )}
      <div className="bp-code" title={node.code}>
        {node.code}
      </div>
    </div>
  );
}

export const BlueprintNode = memo(BlueprintNodeImpl);
