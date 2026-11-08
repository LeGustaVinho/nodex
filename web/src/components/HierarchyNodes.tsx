import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import type { HierarchyNodeData } from "../graph/hierarchy";
import { useUi } from "../store";

const ICON: Record<string, string> = {
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

function kindLabel(kind: string): string {
  return kind === "folder" || kind === "namespace" ? kind : kind.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();
}

function HierarchyNodeImpl({ data, selected }: NodeProps) {
  const { item, childCount } = data as unknown as HierarchyNodeData;
  const toggle = useUi((s) => s.toggleTree);
  return (
    <div className={`hnode hkind-${item.kind.toLowerCase()} ${selected ? "selected" : ""}`} title={item.title}>
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
  const { item } = data as unknown as HierarchyNodeData;
  const toggle = useUi((s) => s.toggleTree);
  return (
    <div className={`hgroup hkind-${item.kind.toLowerCase()} ${selected ? "selected" : ""}`} title={item.title}>
      <Handle type="target" position={Position.Top} isConnectable={false} />
      <div className="hgroup-head">
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
