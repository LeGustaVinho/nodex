import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { api } from "../api";
import { useNavigation, type NavigationKind, type NavigationLocation } from "../navigation";
import { useUi } from "../store";
import { toast } from "../toast";

const headings: Record<NavigationKind, string> = {
  usages: "Usages",
  declaration: "Declarations",
  implementations: "Implementations",
};

function reveal({ symbol, span, source_revision }: NavigationLocation, queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: ["source", span.file] });
  const ui = useUi.getState();
  if (symbol) ui.select({ kind: "symbol", id: symbol }, span, source_revision);
  else ui.focusCode(span, source_revision);
}

export function NavigationResults() {
  const request = useNavigation((s) => s.request);
  const close = useNavigation((s) => s.close);
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: ["navigation", request?.symbol.id, request?.kind],
    queryFn: ({ signal }) => api.navigation(request!.symbol.id, request!.kind, signal),
    enabled: !!request,
    refetchInterval: (q) => q.state.data?.coverage === "loading" ? 750 : false,
  });
  useEffect(() => {
    if (!request || request.kind === "usages" || query.isFetching || query.isError || !query.data ||
        query.data.coverage === "loading" || query.data.locations.length !== 1 ||
        useNavigation.getState().request !== request || useUi.getState().codeIntent !== request.codeIntent) return;
    reveal(query.data.locations[0], queryClient);
    if (query.data.coverage === "partial") toast.info("Opened the only result in the available project links.");
    close();
  }, [request, query.data, query.isFetching, query.isError, close, queryClient]);
  if (!request) return null;
  const title = headings[request.kind];
  const files = new Map<string, NavigationLocation[]>();
  for (const location of query.data?.locations ?? []) {
    const list = files.get(location.span.file) ?? [];
    list.push(location);
    files.set(location.span.file, list);
  }
  return (
    <section className="inspector navigation-results" aria-label={title} onKeyDown={(e) => { if (e.key === "Escape") close(); }}>
      <div className="navigation-head">
        <strong>{title} · {request.symbol.name}</strong>
        <button type="button" aria-label="Close navigation results" onClick={close}>×</button>
      </div>
      <div role="status">
        {(query.isPending || (query.isFetching && query.data?.coverage !== "loading")) && "Searching the project…"}
        {query.data?.coverage === "loading" && "Semantic analysis is in progress. Results will update when it finishes."}
        {query.data?.coverage === "partial" && "Partial results from available project links."}
        {query.data && !query.isFetching && !files.size && query.data.coverage !== "loading" && ` No ${title.toLowerCase()} found.`}
      </div>
      {query.isError && <p role="alert">Search failed. <button onClick={() => void query.refetch()}>Retry</button></p>}
      {[...files].map(([file, locations]) => (
        <div className="navigation-file" key={file}>
          <strong title={file}>{file}</strong>
          {locations.map((location) => (
            <button key={location.span.start_byte} type="button" disabled={query.isFetching || query.isError} onClick={() => reveal(location, queryClient)} title={location.snippet}>
              <span className="muted">{location.span.start_line}:{location.span.start_col}</span>
              <code>{location.snippet}</code>
              {location.approximate && <span className="badge small">Symbol only</span>}
            </button>
          ))}
        </div>
      ))}
    </section>
  );
}
