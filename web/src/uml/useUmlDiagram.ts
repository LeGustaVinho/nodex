import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../api";
import { useUi } from "../store";
import { scopeKey } from "./uml";

/** The availability of the current scope and the current diagram (shared by the view and its controls). */
export function useUmlDiagram() {
  const kind = useUi((s) => s.umlKind);
  const scope = useUi((s) => s.umlScope);
  const umlOpts = useUi((s) => s.umlOpts);
  const confidences = useUi((s) => s.confidences);
  // The confidence toggles of the top bar filter relations (all three: no filter).
  const opts = useMemo(
    () => ({ ...umlOpts, confidences: confidences.length === 3 ? [] : (confidences as ("Exact" | "Inferred" | "Unresolved")[]) }),
    [umlOpts, confidences],
  );
  const key = scopeKey(scope);
  const availability = useQuery({ queryKey: ["uml-availability", key], queryFn: () => api.umlAvailability(scope), retry: false });
  const available = availability.data?.kinds.find((k) => k.kind === kind);
  const diagram = useQuery({
    queryKey: ["uml", kind, key, JSON.stringify(opts)],
    queryFn: () => api.uml(kind, scope, opts),
    enabled: available?.available ?? false,
    retry: false,
    placeholderData: (prev) => prev,
  });
  return { kind, scope, availability, available, diagram };
}
