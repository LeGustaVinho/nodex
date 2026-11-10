import { useMemo } from "react";
import { useDebug } from "../debug";
import { classMembers } from "../graph/classBlueprint";
import { isBlueprintContainer, type StructureIndex } from "../graph/structureGraph";
import { useUi } from "../store";
import { useTrace } from "../trace";

/** The Inspector and the canvas follow the same visible replay. */
export function useBlueprintReplay(index?: StructureIndex) {
  const view = useUi((s) => s.view);
  const symbolId = useUi((s) => s.flowSymbol);
  const selection = useUi((s) => s.selection);
  const debugFrame = useDebug((s) => s.stopped?.frames.find((frame) => frame.id === s.frame));
  const replay = useTrace((s) => s.replay);
  const members = useMemo(() => {
    const symbol = symbolId ? index?.byId.get(symbolId) : undefined;
    return index && symbol && isBlueprintContainer(symbol) ? classMembers(index, symbol.id) : undefined;
  }, [index, symbolId]);

  const selectedNode = selection?.kind === "flow-node" ? selection.id
    : selection?.kind === "expr" ? selection.nodeId : undefined;
  const atDebugFrame = !!debugFrame?.flow && (
    (selection?.kind === "flow-node" || selection?.kind === "expr") && selection.flow === debugFrame.flow &&
      selectedNode === debugFrame.node ||
    !debugFrame.node && selection?.kind === "symbol" && selection.id === debugFrame.flow
  );
  const active = view === "blueprint" && replay && (members
    ? !atDebugFrame && members.some((member) => member.id === replay.flow)
    : replay.flow === symbolId);
  return { replay: active ? replay : null, atDebugFrame };
}
