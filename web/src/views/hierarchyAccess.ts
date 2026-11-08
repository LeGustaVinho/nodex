// The containment hierarchy and the knowledge query, apart from the Knowledge views, so the
// palette and the inspector can use them without loading those views up front.

import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../api";
import { Hierarchy } from "../graph/hierarchy";
import type { StructureIndex } from "../graph/structureGraph";
import { useUi } from "../store";
import { useStructure } from "./Views";

export function useKnowledge() {
  return useQuery({ queryKey: ["knowledge"], queryFn: api.knowledge });
}

const hierarchies = new WeakMap<StructureIndex, Hierarchy>();

/** The containment hierarchy of the current structure (built once per structure graph). */
export function useHierarchy(): Hierarchy | undefined {
  const { index } = useStructure();
  return useMemo(() => {
    if (!index) return undefined;
    let h = hierarchies.get(index);
    if (!h) {
      h = new Hierarchy(index);
      hierarchies.set(index, h);
    }
    return h;
  }, [index]);
}

/** Opens the ancestors of `id` in the Hierarchy view, shows the view and selects the symbol. */
export function revealInHierarchy(h: Hierarchy, index: StructureIndex, id: string) {
  const ui = useUi.getState();
  ui.expandTree(h.ancestors(id));
  ui.setView("hierarchy");
  ui.select({ kind: "symbol", id }, index.byId.get(id)?.origins[0] ?? null);
}

