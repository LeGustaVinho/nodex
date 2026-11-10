import { useQueries, type UseQueryResult } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";
import { api } from "../api";
import type { FlowGraph } from "../gen/FlowGraph";
import type { BlueprintSource, BlueprintSources } from "../graph/blueprintSource";

export function useBlueprintSources(flows: readonly FlowGraph[] | undefined, open?: ReadonlySet<string>): BlueprintSources {
  const files = useMemo(() => [...new Set(
    flows?.filter((flow) => flow.source_revision && (!open || open.has(flow.symbol))).map((flow) => flow.span.file),
  )], [flows, open]);
  return useQueries({
    queries: files.map((file) => ({ queryKey: ["source", file], queryFn: ({ signal }: { signal: AbortSignal }) => api.sourceFile(file, signal) })),
    combine: useCallback((results: UseQueryResult<BlueprintSource>[]) => Object.fromEntries(
      results.flatMap((result, i) => result.data ? [[files[i], result.data]] : []),
    ), [files]),
  });
}
