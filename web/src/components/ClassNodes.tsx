import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import { CLASS_IN, CLASS_OUT, FIELD_OUT, type ClassBoxData, type FieldCardData, type MethodCardData } from "../graph/classBlueprint";
import { visibilityOf } from "../graph/explore";
import { codePreview, uniqueCodeRange } from "../graph/blueprintSource";
import { codeLanguage } from "../graph/codeTokens";
import type { Symbol } from "../gen/Symbol";
import { useUi } from "../store";
import { BlueprintCode } from "./HighlightedCode";
import { syntaxBadges } from "../graph/syntaxSites";
import { ValueBadge } from "./ValueBadge";
import { LanguageBadges } from "./LanguageBadges";
import { TypeLabel } from "./TypeLabel";
import { declarationDetailFlavors } from "../graph/blueprintExplanation";
import { flavorInfo, useCatalog } from "../catalog";

function DeclarationDetails({ symbol }: { symbol: Symbol }) {
  const explain = useUi((s) => s.blueprintExplanations);
  const select = useUi((s) => s.select);
  const catalog = useCatalog();
  const details = explain ? syntaxBadges(symbol.syntax_sites ?? [], (key) => flavorInfo(catalog, key, symbol.language),
    declarationDetailFlavors(symbol).flatMap((flavor) => flavorInfo(catalog, flavor, symbol.language) ?? [])) : [];
  return details.length > 0 && (
    <div className="class-declaration">
      <LanguageBadges details={details} onInspect={(syntax) => select({ kind: "symbol", id: symbol.id, syntax })} />
    </div>
  );
}

function Declaration({ text, symbol }: { text: string; symbol: Symbol }) {
  const preview = codePreview(text, uniqueCodeRange(text, symbol.name));
  return preview && <BlueprintCode preview={preview} language={codeLanguage(symbol.language, symbol.origins[0]?.file ?? "")}
    className="blueprint-declaration-code" />;
}

function ClassBoxImpl({ data, selected }: NodeProps) {
  const { symbol, bases, members, declaration, note } = data as unknown as ClassBoxData;
  const setOpen = useUi((s) => s.setClassOpen);
  return (
    <div className={`class-box ${selected ? "selected" : ""}`}>
      <div className="class-box-head">
        <div className="class-box-title">
          {declaration
            ? <Declaration text={declaration} symbol={symbol} />
            : <strong title={symbol.qualified_name}>{symbol.name}</strong>}
          {bases.length > 0 && <span className="muted" title={bases.join(", ")}>{bases.join(", ")}</span>}
        </div>
        {members.length > 0 && (
          <div className="class-actions">
            <button className="nodrag" onClick={(e) => { e.stopPropagation(); setOpen(members, true); }}>Open all</button>
            <button className="nodrag" onClick={(e) => { e.stopPropagation(); setOpen(members, false); }}>Close all</button>
          </div>
        )}
      </div>
      {note && <div className="class-declaration muted" title={note}>{note}</div>}
      <DeclarationDetails symbol={symbol} />
    </div>
  );
}

function MethodImpl({ data, selected }: NodeProps) {
  const { symbol, complexity, calls, open, note } = data as unknown as MethodCardData;
  const toggle = useUi((s) => s.toggleClassMember);
  const title = symbol.signature ?? symbol.name;
  const showKind = useUi((s) => s.blueprintExplanations);
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
        {open ? <span title={title}>{symbol.name}</span> : <Declaration text={title} symbol={symbol} />}
        {symbol.type_info && <TypeLabel info={symbol.type_info} />}
        {showKind && symbol.value_info && <ValueBadge info={symbol.value_info}
          onInspect={() => useUi.getState().select({ kind: "symbol", id: symbol.id })} />}
      </div>
      {note && <div className="class-declaration muted" title={note}>{note}</div>}
      <DeclarationDetails symbol={symbol} />
      {!open && (
        <>
          <div className="class-method-meta">
            {complexity !== undefined && <span title="Cyclomatic complexity">Complexity {complexity}</span>}
            <span>{calls} {calls === 1 ? "call" : "calls"}</span>
          </div>
          <Handle type="source" position={Position.Bottom} id={CLASS_OUT} isConnectable={false} />
          <Handle className="class-wire-handle" type="source" position={Position.Right} id={FIELD_OUT} isConnectable={false} />
        </>
      )}
    </div>
  );
}

function FieldCardImpl({ data, selected }: NodeProps) {
  const { symbol, declaration, note, role } = data as unknown as FieldCardData;
  const showKind = useUi((s) => s.blueprintExplanations);
  return (
    <div className={`class-field ${role ? `class-${role}` : ""} ${selected ? "selected" : ""}`} title={symbol.signature ?? symbol.qualified_name}>
      {!role && <Handle type="target" position={Position.Left} id={CLASS_IN} isConnectable={false} />}
      <div className="class-field-title">
        {declaration && symbol.signature
          ? <Declaration text={declaration} symbol={symbol} />
          : <strong>{symbol.name}</strong>}
        {(symbol.kind === "Field" || symbol.kind === "Property") && <TypeLabel info={symbol.type_info} fallback={symbol.type_text}
          onInspect={() => useUi.getState().select({ kind: "symbol", id: symbol.id })} />}
        {showKind && (symbol.kind === "Field" || symbol.kind === "Property") && <ValueBadge info={symbol.value_info}
          onInspect={() => useUi.getState().select({ kind: "symbol", id: symbol.id })} />}
        <span className="badge small">{role === "variant" ? "Variant" : role === "contract" ? "Contract" : visibilityOf(symbol) ?? "default"}</span>
      </div>
      {!(declaration && symbol.signature) && (
        <span className="class-field-type muted">{symbol.kind}</span>
      )}
      {note && <span className="class-field-type muted" title={note}>{note}</span>}
      <DeclarationDetails symbol={symbol} />
    </div>
  );
}

export const ClassBox = memo(ClassBoxImpl);
export const MethodNode = memo(MethodImpl);
export const FieldCard = memo(FieldCardImpl);
