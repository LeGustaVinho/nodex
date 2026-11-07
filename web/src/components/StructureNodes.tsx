import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import { flavorLabel, useCatalog } from "../catalog";
import type { SymbolKind } from "../gen/SymbolKind";
import { MAX_MEMBERS, type NamespaceData, type TypeCardData } from "../graph/structureGraph";
import { useUi } from "../store";

const KIND_ICON: Partial<Record<SymbolKind, string>> = {
  Method: "ƒ",
  Function: "ƒ",
  Constructor: "✦",
  Property: "◈",
  Field: "●",
  Event: "⚡",
  EnumMember: "•",
};

const TYPE_LABEL: Partial<Record<SymbolKind, string>> = {
  Class: "class",
  Struct: "struct",
  Interface: "interface",
  Trait: "trait",
  Enum: "enum",
  Union: "union",
  Record: "record",
  Delegate: "delegate",
  TypeAlias: "type",
};

/** Flavors worth showing on a type card header. */
const CARD_FLAVORS = new Set([
  "abstract", "sealed", "static", "partial", "generic", "value-type", "unity:MonoBehaviour", "unity:ScriptableObject",
  "type-only", "declare", "export", "default-export",
]);

function NamespaceNodeImpl({ data, selected }: NodeProps) {
  const { symbol, types, functions, external } = data as unknown as NamespaceData;
  const isPackage = symbol.kind === "Package";
  const isModule = symbol.kind === "Module";
  return (
    <div className={`ns ${external ? "external" : ""} ${isPackage ? "package" : ""} ${selected ? "selected" : ""}`}>
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <div className="ns-kind">{isPackage ? "package" : external ? (isModule ? "external module" : "external namespace") : isModule ? "module" : "namespace"}</div>
      <div className="ns-name">{symbol.qualified_name}</div>
      {!external && (
        <div className="ns-meta">
          {types} types{functions > 0 ? ` · ${functions} functions` : ""}
        </div>
      )}
      {isPackage && symbol.signature && <div className="ns-meta">{symbol.signature}</div>}
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
}

function TypeCardImpl({ data, selected }: NodeProps) {
  const { symbol, members, namespace } = data as unknown as TypeCardData;
  const catalog = useCatalog();
  const openFlow = useUi((s) => s.openFlow);
  const select = useUi((s) => s.select);
  const shown = members.slice(0, MAX_MEMBERS);
  const flavors = symbol.flavors.filter((f) => CARD_FLAVORS.has(f));
  return (
    <div className={`card kind-${symbol.kind.toLowerCase()} ${symbol.external ? "external" : ""} ${selected ? "selected" : ""}`}>
      <Handle type="target" position={Position.Top} isConnectable={false} />
      <div className="card-head">
        <span className="card-kind">{TYPE_LABEL[symbol.kind] ?? symbol.kind}</span>
        <span className="card-name">{symbol.name}</span>
      </div>
      {(flavors.length > 0 || namespace) && (
        <div className="card-sub">
          {namespace && <span className="card-ns">{namespace}</span>}
          {flavors.map((f) => (
            <span key={f} className={`badge small ${f.startsWith("unity:") ? "badge-unity" : ""}`}>
              {flavorLabel(catalog, f, symbol.language)}
            </span>
          ))}
        </div>
      )}
      {shown.length > 0 && (
        <ul className="members">
          {shown.map((m) => (
            <li key={m.id}>
              <button
                className={`member nodrag ${m.has_flow ? "has-flow" : ""}`}
                title={[m.signature ?? m.name, m.doc].filter(Boolean).join("\n\n")}
                onClick={(e) => {
                  e.stopPropagation();
                  select({ kind: "symbol", id: m.id }, m.origins[0] ?? null);
                }}
                onDoubleClick={(e) => {
                  e.stopPropagation();
                  if (m.has_flow) openFlow(m.id);
                }}
              >
                <span className="member-icon">{KIND_ICON[m.kind] ?? "·"}</span>
                <span className="member-name">{m.name}</span>
                {m.type_text && <span className="member-type">{m.type_text}</span>}
                {m.has_flow && (
                  <span
                    className="member-open"
                    role="button"
                    title="Open Blueprint"
                    onClick={(e) => {
                      e.stopPropagation();
                      openFlow(m.id);
                    }}
                  >
                    ▶
                  </span>
                )}
              </button>
            </li>
          ))}
          {members.length > MAX_MEMBERS && <li className="more">+{members.length - MAX_MEMBERS} more</li>}
        </ul>
      )}
      <Handle type="source" position={Position.Bottom} isConnectable={false} />
    </div>
  );
}

export const NamespaceNode = memo(NamespaceNodeImpl);
export const TypeCard = memo(TypeCardImpl);
