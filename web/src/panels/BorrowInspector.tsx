import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api";
import type { BorrowLocation } from "../gen/BorrowLocation";
import type { BorrowOriginKind } from "../gen/BorrowOriginKind";
import { useUi } from "../store";

const DESCRIPTION: Record<BorrowOriginKind, string> = {
  Local: "Owner",
  Shared: "Shared ownership",
  Temporary: "Temporary value",
  Static: "Static storage",
  Caller: "From caller",
  Unknown: "Origin unknown",
};

export function BorrowInspector({ flow, node, pin, path }: { flow: string; node: string; pin?: string; path?: string }) {
  const [pages, setPages] = useState([0]);
  const offset = pages.at(-1)!;
  const [sourceChanged, setSourceChanged] = useState(false);
  const queryClient = useQueryClient();
  const project = useUi((s) => s.projectSession);
  const query = useQuery({
    queryKey: ["borrow-origin", project, flow, node, pin, path, offset],
    queryFn: ({ signal }) => api.borrowOrigin(flow, node, pin, path, offset, signal),
  });
  const show = async (location: BorrowLocation) => {
    const revision = location.revision;
    if (!revision) return;
    const state = useUi.getState();
    const selection = state.selection;
    setSourceChanged(false);
    try {
      const source = await queryClient.fetchQuery({
        queryKey: ["source", location.span.file],
        queryFn: ({ signal }) => api.sourceFile(location.span.file, signal),
        staleTime: 0,
      });
      const ui = useUi.getState();
      if (ui.projectSession !== project || ui.selection !== selection) return;
      if (source.revision !== revision) {
        setSourceChanged(true);
        return;
      }
      if (location.flow !== flow) ui.openFlow(location.flow);
      if (location.node) {
        ui.select({ kind: "flow-node", flow: location.flow, id: location.node, pin: location.pin ?? undefined }, location.span, revision);
      } else {
        ui.focusCode(location.span, revision);
      }
    } catch {
      if (useUi.getState().projectSession === project && useUi.getState().selection === selection) setSourceChanged(true);
    }
  };
  const link = (location: BorrowLocation, label: string) => location.revision ?
    <button type="button" className="link" onClick={() => void show(location)}>{label}</button> : <span>{label}</span>;
  return (
    <section className="insp-section borrow-inspector" aria-label="Borrow origin">
      <div className="section-title">Borrow origin</div>
      {query.isPending ? <p className="muted">Finding the origin…</p> :
        query.isError ? <p className="muted">Origin unavailable</p> : <>
          {query.data.origins.map(({ kind, owner, borrowed, through, call }, i) => (
            <div className="borrow-origin" key={i}>
              {call && <div className="value-row"><strong>Call</strong> {link(call, call.label + " · " + call.span.file + ":" + call.span.start_line)}</div>}
              <div className="value-row"><strong>{DESCRIPTION[kind]}</strong> {owner && <code>{owner.label}</code>}</div>
              {borrowed && kind !== "Temporary" && kind !== "Static" && <div className="value-row"><strong>Borrowed</strong> <code>{borrowed}</code></div>}
              {through.length > 0 && <ol className="borrow-chain" aria-label="Reference chain">
                {through.map((location) => <li key={location.span.file + ":" + location.span.start_byte}>{link(location, location.label)}</li>)}
              </ol>}
              {owner && link(owner, kind === "Local" || kind === "Shared" ? "Show owner" : "Show in source")}
            </div>
          ))}
          {query.data.partial && <p className="muted">Some paths could only be followed part of the way.</p>}
          {(pages.length > 1 || query.data.next_offset != null) && <div className="button-row">
            {pages.length > 1 && <button onClick={() => setPages((history) => history.slice(0, -1))}>Previous calls</button>}
            {query.data.next_offset != null && <button onClick={() => setPages((history) => [...history, query.data.next_offset!])}>More calls</button>}
          </div>}
        </>}
      {sourceChanged && <p role="status" className="muted">The source changed. Select the refreshed occurrence to navigate.</p>}
    </section>
  );
}
