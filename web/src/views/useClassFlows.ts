import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../api";
import { classMembers } from "../graph/classBlueprint";
import type { StructureIndex } from "../graph/structureGraph";

/** The class canvas and the source editor share the same member bodies. */
export function useClassFlows(index: StructureIndex | undefined, typeId: string | null) {
  const ids = useMemo(() => index && typeId ? classMembers(index, typeId).map((m) => m.id) : [], [index, typeId]);
  const query = useQuery({
    queryKey: ["flows", typeId, ids],
    queryFn: () => api.flows(ids),
    enabled: ids.length > 0,
  });
  return { ids, query };
}
