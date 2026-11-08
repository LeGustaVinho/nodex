/**
 * Containment hierarchy for the Hierarchy view: folder or namespace → module → type → member,
 * plus the roll-up of relations onto whatever level is currently visible.
 *
 * The tree and the roll-up algorithm follow CodeGraph's explorer (`crates/ui/src/tree.rs`):
 * every relation climbs to the nearest visible ancestor of each end; relations inside one item
 * or between an item and its own descendants disappear; the rest are counted per pair.
 */
import type { Confidence } from "../gen/Confidence";
import type { StructEdge } from "../gen/StructEdge";
import type { StructEdgeKind } from "../gen/StructEdgeKind";
import type { Symbol } from "../gen/Symbol";
import type { GEdge, GGraph, GNode } from "./model";
import { type StructureIndex, isTypeKind } from "./structureGraph";

/** Kind of a synthetic container (no symbol behind it). */
export type ContainerKind = "folder" | "namespace";

export interface HItem {
  id: string;
  parent: string | null;
  /** `folder`, `namespace` (synthetic) or the symbol kind. */
  kind: ContainerKind | Symbol["kind"];
  label: string;
  /** Full name for tooltips. */
  title: string;
  symbol: Symbol | null;
  children: string[];
}

const rank = (item: HItem): number => {
  if (item.kind === "folder" || item.kind === "namespace" || item.kind === "Namespace" || item.kind === "Package") return 0;
  if (item.kind === "Module") return 1;
  if (item.symbol && isTypeKind(item.symbol.kind)) return 2;
  return 3;
};

export class Hierarchy {
  items = new Map<string, HItem>();
  roots: string[] = [];

  constructor(index: StructureIndex) {
    const add = (item: HItem) => this.items.set(item.id, item);
    for (const s of index.graph.symbols) {
      if (s.external) continue;
      add({ id: s.id, parent: s.parent, kind: s.kind, label: labelOf(s), title: s.qualified_name, symbol: s, children: [] });
    }
    // Synthetic containers for namespaces without a declaration of their own and for folders.
    const ensure = (id: string, kind: ContainerKind, label: string, title: string, parent: string | null): string => {
      if (!this.items.has(id)) add({ id, parent, kind, label, title, symbol: null, children: [] });
      return id;
    };
    const namespaceContainer = (qualified: string): string | null => {
      if (!qualified) return null;
      const real = `ns:${qualified}`;
      if (this.items.has(real)) return real;
      const dot = qualified.lastIndexOf(".");
      const parent = namespaceContainer(dot > 0 ? qualified.slice(0, dot) : "");
      return ensure(`grp:ns:${qualified}`, "namespace", qualified.slice(dot + 1), qualified, parent);
    };
    const folderContainer = (dir: string): string | null => {
      if (!dir) return null;
      const slash = dir.lastIndexOf("/");
      const parent = folderContainer(slash > 0 ? dir.slice(0, slash) : "");
      return ensure(`dir:${dir}`, "folder", dir.slice(slash + 1), dir, parent);
    };
    for (const item of [...this.items.values()]) {
      const s = item.symbol;
      if (!s || item.parent) continue;
      if (s.kind === "Namespace") {
        const dot = s.qualified_name.lastIndexOf(".");
        item.parent = namespaceContainer(dot > 0 ? s.qualified_name.slice(0, dot) : "");
      } else if (s.kind === "Module" && !s.id.includes("::")) {
        // File modules (TypeScript): grouped by folder.
        const slash = s.qualified_name.lastIndexOf("/");
        item.parent = folderContainer(slash > 0 ? s.qualified_name.slice(0, slash) : "");
      } else if (s.kind !== "Module") {
        const file = s.origins[0]?.file ?? "";
        const slash = file.lastIndexOf("/");
        item.parent = folderContainer(slash > 0 ? file.slice(0, slash) : "");
      }
    }
    // A parent that is not part of the tree (external) makes the item a root.
    for (const item of this.items.values()) if (item.parent && !this.items.has(item.parent)) item.parent = null;
    this.link();
    // Namespaces and modules with nothing declared in them (`(global)` for usings only) are noise.
    for (const item of [...this.items.values()]) {
      const scope = !item.symbol || item.kind === "Namespace" || item.kind === "Module";
      if (scope && item.children.length === 0) this.items.delete(item.id);
    }
    this.link();
    this.compact();
  }

  private link() {
    this.roots = [];
    for (const item of this.items.values()) item.children = [];
    for (const item of this.items.values()) {
      if (item.parent) this.items.get(item.parent)!.children.push(item.id);
      else this.roots.push(item.id);
    }
    const order = (ids: string[]) =>
      ids.sort((a, b) => {
        const x = this.items.get(a)!;
        const y = this.items.get(b)!;
        return rank(x) - rank(y) || x.label.localeCompare(y.label);
      });
    order(this.roots);
    for (const item of this.items.values()) order(item.children);
  }

  /** Merges chains of synthetic containers with a single child (`src` → `src/services`). */
  private compact() {
    let changed = true;
    while (changed) {
      changed = false;
      for (const item of this.items.values()) {
        if (item.symbol || item.children.length !== 1) continue;
        const only = this.items.get(item.children[0])!;
        if (only.symbol || only.kind !== item.kind) continue;
        const sep = item.kind === "folder" ? "/" : ".";
        only.label = `${item.label}${sep}${only.label}`;
        only.parent = item.parent;
        this.items.delete(item.id);
        changed = true;
        break;
      }
      if (changed) this.link();
    }
  }

  get(id: string): HItem | undefined {
    return this.items.get(id);
  }

  parentOf(id: string): string | null {
    return this.items.get(id)?.parent ?? null;
  }

  /** Ancestors of `id`, nearest first. */
  ancestors(id: string): string[] {
    const out: string[] = [];
    let cur = this.parentOf(id);
    while (cur) {
      out.push(cur);
      cur = this.parentOf(cur);
    }
    return out;
  }

  isAncestor(ancestor: string, id: string): boolean {
    let cur = this.parentOf(id);
    while (cur) {
      if (cur === ancestor) return true;
      cur = this.parentOf(cur);
    }
    return false;
  }

  /** Items drawn when `expanded` containers are open, parents before children (at most `cap`). */
  visible(expanded: ReadonlySet<string>, cap = Infinity): { ids: string[]; truncated: boolean } {
    const ids: string[] = [];
    let truncated = false;
    const walk = (list: string[]) => {
      for (const id of list) {
        if (ids.length >= cap) {
          truncated = true;
          return;
        }
        ids.push(id);
        const item = this.items.get(id)!;
        if (expanded.has(id) && item.children.length) walk(item.children);
      }
    };
    walk(this.roots);
    return { ids, truncated };
  }
}

function labelOf(s: Symbol): string {
  if (s.kind === "Module") {
    const path = s.qualified_name;
    const tail = path.includes("::") ? path.split("::").pop() : path.split("/").pop();
    return tail || path;
  }
  if (s.kind === "Namespace") return s.qualified_name.slice(s.qualified_name.lastIndexOf(".") + 1);
  return s.name;
}

/** A relation drawn between two visible items, standing for `count` relations below them. */
export interface RolledEdge {
  source: string;
  target: string;
  count: number;
  /** The most frequent relation. */
  kind: StructEdgeKind;
  /** The strongest confidence present (a relation without confidence is exact). */
  confidence: Confidence;
  kinds: Partial<Record<StructEdgeKind, number>>;
  confidences: Partial<Record<Confidence, number>>;
}

const STRENGTH: Record<Confidence, number> = { Exact: 0, Inferred: 1, Unresolved: 2 };

/** Aggregates `edges` onto the visible items (see the module comment). */
export function rollup(h: Hierarchy, edges: StructEdge[], visible: ReadonlySet<string>, kinds: ReadonlySet<StructEdgeKind>): RolledEdge[] {
  const memo = new Map<string, string | null>();
  const nearest = (id: string): string | null => {
    const known = memo.get(id);
    if (known !== undefined) return known;
    let cur: string | null = h.get(id) ? id : null;
    while (cur && !visible.has(cur)) cur = h.parentOf(cur);
    memo.set(id, cur);
    return cur;
  };
  const out = new Map<string, RolledEdge>();
  for (const e of edges) {
    if (e.kind === "Contains" || !kinds.has(e.kind)) continue;
    const a = nearest(e.from);
    const b = nearest(e.to);
    if (!a || !b || a === b || h.isAncestor(a, b) || h.isAncestor(b, a)) continue;
    const key = `${a}\u0000${b}`;
    const confidence = e.confidence ?? "Exact";
    let r = out.get(key);
    if (!r) {
      r = { source: a, target: b, count: 0, kind: e.kind, confidence, kinds: {}, confidences: {} };
      out.set(key, r);
    }
    r.count++;
    r.kinds[e.kind] = (r.kinds[e.kind] ?? 0) + 1;
    r.confidences[confidence] = (r.confidences[confidence] ?? 0) + 1;
    if (STRENGTH[confidence] < STRENGTH[r.confidence]) r.confidence = confidence;
  }
  for (const r of out.values()) {
    r.kind = (Object.entries(r.kinds) as [StructEdgeKind, number][]).reduce((best, cur) => (cur[1] > best[1] ? cur : best))[0];
  }
  return [...out.values()];
}

/** Visual edge kind of a relation (shared with the other views' edge styles). */
export function edgeKindOf(kind: StructEdgeKind): string {
  switch (kind) {
    case "Instantiates":
      return "creates";
    case "References":
      return "uses";
    default:
      return kind.toLowerCase();
  }
}

export interface HierarchyNodeData {
  item: HItem;
  /** Direct children count (shown on collapsed items). */
  childCount: number;
  expanded: boolean;
}

export const LEAF_WIDTH = 190;
export const LEAF_HEIGHT = 44;
/** Most items the view draws at once. */
export const HIERARCHY_CAP = 400;

/** The Hierarchy view: expanded containers become boxes around their visible children. */
export function hierarchyGraph(
  h: Hierarchy,
  index: StructureIndex,
  expanded: ReadonlySet<string>,
  kinds: ReadonlySet<StructEdgeKind>,
): GGraph & { truncated: boolean } {
  const { ids, truncated } = h.visible(expanded, HIERARCHY_CAP);
  const visible = new Set(ids);
  const nodes: GNode[] = ids.map((id) => {
    const item = h.get(id)!;
    const open = expanded.has(id) && item.children.some((c) => visible.has(c));
    const parent = item.parent && visible.has(item.parent) ? item.parent : undefined;
    return {
      id,
      type: open ? "hgroup" : "hnode",
      data: { item, childCount: item.children.length, expanded: open } satisfies HierarchyNodeData,
      width: LEAF_WIDTH,
      height: LEAF_HEIGHT,
      ports: [],
      parent,
      container: open,
    };
  });
  const edges: GEdge[] = rollup(h, index.graph.edges, visible, kinds).map((r) => ({
    id: `${r.source}->${r.target}`,
    source: r.source,
    target: r.target,
    kind: edgeKindOf(r.kind),
    layout: true,
    confidence: r.confidence,
    weight: r.count,
    data: r,
  }));
  return { nodes, edges, direction: "DOWN", aspectRatio: 1.8, truncated };
}

/** Relation groups the Hierarchy and Knowledge views can toggle. */
export const RELATION_GROUPS: { id: string; label: string; kinds: StructEdgeKind[] }[] = [
  { id: "calls", label: "calls", kinds: ["Calls", "Raises", "Subscribes"] },
  { id: "types", label: "types", kinds: ["Inherits", "Implements", "Compatible", "Instantiates", "References"] },
  { id: "imports", label: "imports", kinds: ["Imports", "DependsOn"] },
  { id: "knowledge", label: "knowledge", kinds: ["HttpCalls", "ResolvesDi", "Tests", "Throws", "CaughtBy", "Mutates"] },
];

export function kindsOf(groups: readonly string[]): Set<StructEdgeKind> {
  return new Set(RELATION_GROUPS.filter((g) => groups.includes(g.id)).flatMap((g) => g.kinds));
}
