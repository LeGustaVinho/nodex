import type { MenuItem } from "../components/ContextMenu";
import type { Symbol } from "../gen/Symbol";
import type { ContextItem } from "../graph/context";
import { isBlueprintContainer } from "../graph/structureGraph";
import { useUi } from "../store";

export function contextMenuItems(item: ContextItem): MenuItem[] {
  return [
    { label: "Use as context", run: () => useUi.getState().setContext([item]) },
    { label: "Add to context", run: () => useUi.getState().addToContext(item) },
  ];
}

export function blueprintMenuItems(symbol: Symbol | null | undefined): MenuItem[] {
  if (!symbol || symbol.external) return [];
  const container = isBlueprintContainer(symbol);
  return container || symbol.has_flow
    ? [{ label: container ? "Open as Blueprint" : "Open Blueprint", run: () => useUi.getState().openFlow(symbol.id) }]
    : [];
}
