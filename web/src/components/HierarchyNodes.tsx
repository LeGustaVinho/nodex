import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import type { HierarchyNodeData } from "../graph/hierarchy";
import { useUi } from "../store";

export const ICON: Record<string, string> = {
  folder: "▤",
  namespace: "{ }",
  Namespace: "{ }",
  Module: "▧",
  Package: "▣",
  Class: "C",
  Struct: "S",
  Interface: "I",
  Trait: "T",
  Enum: "E",
  Union: "U",
  Record: "R",
  Delegate: "D",
  TypeAlias: "≡",
  Function: "ƒ",
  Method: "ƒ",
  Constructor: "✦",
  Property: "◈",
  Field: "●",
  Event: "⚡",
  EnumMember: "•",
};

export function kindLabel(kind: string): string {
  return kind === "folder" || kind === "namespace" ? kind : kind.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();
}

/** Classes for what a node is beyond its kind: entry point, test, third-party, outside the project. */
function marks(d: HierarchyNodeData): string {
  const m: string[] = [];
  if (d.entry) m.push("is-entry");
  if (d.test) m.push("is-test");
  if (d.vendor) m.push("is-vendor");
  if (d.outside) m.push("is-outside");
  if (d.item.symbol?.external) m.push("is-external");
  return m.join(" ");
}

function HierarchyNodeImpl({ data, selected }: NodeProps) {
  const d = data as unknown as HierarchyNodeData;
  const { item, childCount } = d;
  const toggle = useUi((s) => s.toggleTree);
  // Busier nodes get a thicker border (1 to 4 px).
  const border = d.degree ? Math.min(4, 1 + Math.log2(1 + d.degree) * 0.6) : undefined;
  return (
    <div className={`hnode hkind-${item.kind.toLowerCase()} ${marks(d)} ${selected ? "selected" : ""}`} style={border ? { borderWidth: border } : undefined}>
      <Handle type="target" position={Position.Top} isConnectable={false} />
      <span className="hnode-icon">{ICON[item.kind] ?? "·"}</span>
      <span className="hnode-text">
        <span className="hnode-label">{item.label}</span>
        <span className="hnode-kind">{kindLabel(item.kind)}</span>
      </span>
      {childCount > 0 && (
        <button
          className="hnode-count nodrag"
          title="Show what is inside (or double-click)"
          onClick={(e) => {
            e.stopPropagation();
            toggle(item.id);
          }}
        >
          {childCount} ▸
        </button>
      )}
      <Handle type="source" position={Position.Bottom} isConnectable={false} />
    </div>
  );
}

function HierarchyGroupImpl({ data, selected }: NodeProps) {
  const d = data as unknown as HierarchyNodeData;
  const { item, childCount } = d;
  const toggle = useUi((s) => s.toggleTree);
  return (
    <div className={`hgroup hkind-${item.kind.toLowerCase()} ${marks(d)} ${selected ? "selected" : ""}`}>
      <Handle type="target" position={Position.Top} isConnectable={false} />
      <div className="hgroup-head">
        {/* Grouping boxes of the Knowledge view do not open or close. */}
        {childCount > 0 && (
          <button
            className="hgroup-caret nodrag"
            title="Hide what is inside"
            onClick={(e) => {
              e.stopPropagation();
              toggle(item.id);
            }}
          >
            ▾
          </button>
        )}
        <span className="hnode-icon">{ICON[item.kind] ?? "·"}</span>
        <span className="hnode-label">{item.label}</span>
        <span className="hnode-kind">{kindLabel(item.kind)}</span>
      </div>
      <Handle type="source" position={Position.Bottom} isConnectable={false} />
    </div>
  );
}

export const HierarchyNode = memo(HierarchyNodeImpl);
export const HierarchyGroup = memo(HierarchyGroupImpl);
