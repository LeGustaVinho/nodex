import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import { CLASS_IN, CLASS_OUT, type ClassBoxData, type FieldsCardData, type MethodCardData } from "../graph/classBlueprint";
import { visibilityOf } from "../graph/explore";
import { useUi } from "../store";

function ClassBoxImpl({ data, selected }: NodeProps) {
  const { symbol, bases, members } = data as unknown as ClassBoxData;
  const setOpen = useUi((s) => s.setClassOpen);
  return (
    <div className={`class-box ${selected ? "selected" : ""}`}>
      <div className="class-box-head">
        <div className="class-box-title">
          <strong title={symbol.qualified_name}>{symbol.name}</strong>
          {bases.length > 0 && <span className="muted" title={bases.join(", ")}>{bases.join(", ")}</span>}
        </div>
        {members.length > 0 && (
          <div className="class-actions">
            <button className="nodrag" onClick={(e) => { e.stopPropagation(); setOpen(members, true); }}>Open all</button>
            <button className="nodrag" onClick={(e) => { e.stopPropagation(); setOpen(members, false); }}>Close all</button>
          </div>
        )}
      </div>
    </div>
  );
}

function MethodImpl({ data, selected }: NodeProps) {
  const { symbol, complexity, calls, open } = data as unknown as MethodCardData;
  const toggle = useUi((s) => s.toggleClassMember);
  const title = symbol.signature ?? symbol.name;
  return (
    <div className={`class-method ${open ? "method-box" : "method-card"} ${selected ? "selected" : ""}`}>
      {!open && <Handle type="target" position={Position.Top} id={CLASS_IN} isConnectable={false} />}
      <div className="class-method-head">
        <button
          className="nodrag class-toggle"
          aria-label={`${open ? "Close" : "Open"} ${symbol.name}`}
          aria-expanded={open}
          onClick={(e) => { e.stopPropagation(); toggle(symbol.id, open); }}
        >
          {open ? "▾" : "▸"}
        </button>
        <span title={title}>{title}</span>
      </div>
      {!open && (
        <>
          <div className="class-method-meta">
            {complexity !== undefined && <span title="Cyclomatic complexity">Complexity {complexity}</span>}
            <span>{calls} {calls === 1 ? "call" : "calls"}</span>
          </div>
          <Handle type="source" position={Position.Bottom} id={CLASS_OUT} isConnectable={false} />
        </>
      )}
    </div>
  );
}

function FieldsCardImpl({ data }: NodeProps) {
  const { fields } = data as unknown as FieldsCardData;
  return (
    <div className="class-fields">
      <Handle type="target" position={Position.Top} id={CLASS_IN} isConnectable={false} />
      <div className="class-fields-head">Fields &amp; properties</div>
      {fields.map((f) => (
        <div key={f.id} className="class-field" title={f.signature ?? f.qualified_name}>
          <span>{f.name}</span>
          <span className="muted">{f.type_text ?? f.kind.toLowerCase()}</span>
          <span className="badge small">{visibilityOf(f) ?? "default"}</span>
        </div>
      ))}
    </div>
  );
}

export const ClassBox = memo(ClassBoxImpl);
export const MethodNode = memo(MethodImpl);
export const FieldsCard = memo(FieldsCardImpl);
