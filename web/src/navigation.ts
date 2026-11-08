import { create } from "zustand";
import { openMenu, type MenuItem } from "./components/ContextMenu";
import type { Span } from "./gen/Span";
import type { Symbol } from "./gen/Symbol";
import type { BlueprintData, ExprData } from "./graph/flowGraph";
import type { StructureIndex } from "./graph/structureGraph";

export type NavigationKind = "usages" | "declaration" | "implementations";

export interface NavigationLocation {
  span: Span;
  symbol: string | null;
  approximate: boolean;
  snippet: string;
  source_revision: string;
}

export interface NavigationResult {
  coverage: "indexed" | "loading" | "partial";
  locations: NavigationLocation[];
}

interface NavigationRequest {
  symbol: Symbol;
  kind: NavigationKind;
}

export const useNavigation = create<{ request: NavigationRequest | null; close: () => void }>((set) => ({
  request: null,
  close: () => set({ request: null }),
}));

function canHaveImplementations(symbol: Symbol, owner?: Symbol): boolean {
  if (symbol.kind === "TypeAlias") {
    return symbol.flavors.includes("object-contract") ||
      (symbol.language === "rust" && owner?.kind === "Trait" && symbol.flavors.includes("associated-type"));
  }
  if (["Class", "Interface", "Trait", "Record"].includes(symbol.kind)) return !symbol.flavors.includes("sealed");
  if (!["Method", "Property", "Field", "Event"].includes(symbol.kind)) return false;
  if (owner?.kind === "Interface" || owner?.kind === "Trait") return true;
  if (owner?.kind === "TypeAlias" && owner.flavors.includes("object-contract")) return true;
  if (symbol.language === "csharp") {
    return !symbol.flavors.includes("sealed") && symbol.flavors.some((f) => ["abstract", "virtual", "override"].includes(f));
  }
  return symbol.language === "typescript" && owner?.kind === "Class" &&
    !symbol.flavors.some((f) => ["private", "private-field"].includes(f));
}

export function navigationMenuItems(symbol: Symbol | null | undefined, owner?: Symbol | StructureIndex): MenuItem[] {
  if (!symbol || symbol.external || !symbol.origins.length || ["Namespace", "Module", "Package"].includes(symbol.kind)) return [];
  const ownerSymbol = owner && "byId" in owner ? owner.byId.get(symbol.parent ?? "") : owner;
  const item = (label: string, kind: NavigationKind): MenuItem => ({
    label, run: () => useNavigation.setState({ request: { symbol, kind } }),
  });
  return [
    item("Find usages", "usages"),
    item("Go to declaration", "declaration"),
    ...(canHaveImplementations(symbol, ownerSymbol) ? [item("Go to implementation", "implementations")] : []),
  ];
}

export function openNavigationMenu(e: Parameters<typeof openMenu>[0], symbol: Symbol | undefined, owner?: Symbol | StructureIndex) {
  const items = navigationMenuItems(symbol, owner);
  if (items.length) openMenu(e, items);
}

/** Only a unique resolved target or a flow's entry identifies a symbol. */
export function blueprintNavigationTarget(n: { type?: string; data: unknown }): string | undefined {
  if (n.type === "expr") return (n.data as ExprData).expr.symbol ?? undefined;
  if (n.type !== "blueprint") return undefined;
  const { flow, node } = n.data as BlueprintData;
  if (node.concept === "Entry" || node.concept === "EventEntry") return flow;
  if (node.expr?.symbol) return node.expr.symbol;
  // A resolved root call is handled above. Without one, a non-local or unresolved
  // call prevents any other call in the statement from becoming its target.
  if (node.calls.some((call) => !call.symbol)) return undefined;
  const targets = new Set(node.calls.map((call) => call.symbol));
  return targets.size === 1 ? targets.values().next().value ?? undefined : undefined;
}
