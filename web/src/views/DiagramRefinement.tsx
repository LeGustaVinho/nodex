import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { useUi } from "../store";
import { errorText } from "../toast";
import type { Refined, RefinementPreview, RefinementTarget, Reviewed } from "../uml/refinement";

export function useDiagramRefinement(target: RefinementTarget, enabled = true) {
  const session = useUi((s) => s.projectSession);
  const key = ["diagram-refinements", session, target];
  const identity = JSON.stringify(key);
  const client = useQueryClient();
  const history = useQuery({ queryKey: key, queryFn: () => api.refinementHistory(target), enabled, retry: false });
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
  const [choice, setChoice] = useState<{ identity: string; version: string | null }>();
  const active = history.data?.versions.find((v) => v.id === history.data?.active && v.basis === history.data.current);
  const chosen = choice?.identity === identity ? choice.version : active?.id;
  const version = enabled ? history.data?.versions.find((v) => v.id === chosen) : undefined;
  return {
    target, identity, session, history, version, enabled, canGenerate: settings.data?.diagram_ai ?? false,
    historical: !!version && version.basis !== history.data?.current,
    choose: (version: string | null) => setChoice({ identity, version }),
    update: (data: NonNullable<typeof history.data>) => client.setQueryData(key, data),
  };
}

type Controller = ReturnType<typeof useDiagramRefinement>;

export function RefinementControls({ controller }: { controller: Controller }) {
  return <Controls key={controller.identity} controller={controller} />;
}

function Controls({ controller: c }: { controller: Controller }) {
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<RefinementPreview>();
  const [job, setJob] = useState<string>();
  const [focus, setFocus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState<number[]>([]);
  const [failure, setFailure] = useState<string>();
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  const sameProject = () => mounted.current && useUi.getState().projectSession === c.session;
  const progress = useQuery({
    queryKey: ["diagram-refinement-job", c.identity, job],
    queryFn: () => api.refinementStatus(job!),
    enabled: !!job && open,
    retry: false,
    refetchInterval: (query) => !query.state.error && (!query.state.data || query.state.data.phase === "running") ? 700 : false,
  });
  const suggestions = progress.data?.phase === "ready" ? progress.data.suggestions : undefined;
  useEffect(() => { setSelected([]); }, [job]);
  useEffect(() => () => {
    if (job) void api.cancelRefinement(job).catch(() => {});
  }, [job]);
  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setFailure(undefined);
    try { await action(); } catch (error) { if (sameProject()) setFailure(errorText(error)); }
    finally { setBusy(false); }
  };
  const prepare = (exclude: string[], nextFocus: string | null) => run(async () => {
    setJob(undefined);
    setPreview(undefined);
    const next = await api.refinementPreview(c.target, exclude, nextFocus);
    if (sameProject()) { setPreview(next); setFocus(nextFocus); }
  });
  const [excluded, setExcluded] = useState<string[]>([]);
  const start = () => preview && run(async () => {
    const id = await api.startRefinement(preview);
    if (sameProject()) setJob(id);
    else void api.cancelRefinement(id).catch(() => {});
  });
  const close = () => {
    if (job && progress.data?.phase !== "ready") void api.cancelRefinement(job).catch(() => {});
    setOpen(false);
    setJob(undefined);
    setPreview(undefined);
  };
  if (!c.enabled) return null;
  const versions = c.history.data?.versions ?? [];
  return (
    <>
      {versions.length > 0 && <select aria-label="Diagram version" value={c.version?.id ?? ""} onChange={(e) => c.choose(e.target.value || null)}>
        <option value="">Original</option>
        {[...versions].reverse().map((v) => <option key={v.id} value={v.id}>
          {v.basis === c.history.data?.current ? "Refined" : "Historical"} · {new Date(v.created * 1000).toLocaleString()}
        </option>)}
      </select>}
      {c.version && c.history.data && c.version.id !== c.history.data.active && <button disabled={busy} onClick={() => void run(async () => {
        const data = await api.saveRefinement({ action: "activate", target: c.target, version: c.version!.id, revision: c.history.data!.revision });
        if (sameProject()) c.update(data);
      })}>Make active</button>}
      <button onClick={() => {
        setOpen(true);
        setExcluded([]);
        setFailure(undefined);
        if (c.canGenerate) void prepare([], null);
      }}>Refine with AI</button>
      {!open && (failure || c.history.error) && <span role="status" className="muted small">{failure ?? errorText(c.history.error)}</span>}
      {open && <div className="modal-backdrop">
        <section className="modal refinement-modal" role="dialog" aria-modal="true" aria-label="Refine diagram">
          <div className="modal-head"><strong>Refine diagram</strong><button className="modal-close" disabled={busy} onClick={close} aria-label="Close">✕</button></div>
          <div className="settings-body">
            {!c.canGenerate && <p>Enable diagram refinement in <button onClick={() => {
              close();
              useUi.getState().setSettingsOpen(true);
            }}>Settings</button> to prepare a preview for Codex.</p>}
            {busy && <p role="status">Working…</p>}
            {failure && <p role="alert">{failure}</p>}
            {progress.error && <p role="alert">{errorText(progress.error)}</p>}
            {preview && !job && <>
              <p>Codex will receive the exact context below. Review it before sending.</p>
              <p className="muted small">{preview.covered_files.length} files represented · {preview.omitted} entries omitted by the context limit</p>
              <label>Focus <select disabled={busy} value={focus ?? ""} onChange={(e) => void prepare(excluded, e.target.value || null)}>
                <option value="">Whole diagram scope</option>
                {preview.focus_nodes.map(([id, name]) => <option key={id} value={id}>{name}</option>)}
              </select></label>
              {preview.snippets.map((s) => <div key={s.id} className="settings-row">
                <span>{s.path}:{s.line}</span>
                <button disabled={busy} onClick={() => {
                  const next = [...excluded, s.id];
                  setExcluded(next);
                  void prepare(next, focus);
                }}>Remove excerpt</button>
              </div>)}
              {excluded.length > 0 && <button disabled={busy} onClick={() => { setExcluded([]); void prepare([], focus); }}>Restore excerpts</button>}
              <pre className="refinement-context" tabIndex={0}>{preview.input}</pre>
              <button disabled={busy} onClick={() => void start()}>Send this context to Codex</button>
            </>}
            {job && (!progress.data || progress.data.phase === "running") && <div>
              <p role="status">Codex is preparing suggestions…</p>
              <button onClick={() => void run(async () => { await api.cancelRefinement(job); await progress.refetch(); })}>Cancel generation</button>
            </div>}
            {progress.data?.phase === "failed" && <p role="alert">{progress.data.message}</p>}
            {progress.data?.phase === "cancelled" && <p role="status">Generation cancelled.</p>}
            {c.canGenerate && ((job && (progress.data?.phase === "failed" || progress.data?.phase === "cancelled" || progress.error)) || (!preview && !job && failure)) &&
              <button disabled={busy} onClick={() => void prepare(excluded, focus)}>Prepare a new preview</button>}
            {suggestions && <>
              <p>Choose the suggestions to apply. Review the cited evidence for each change.</p>
              <ul className="refinement-suggestions">{suggestions.map((r) => <li key={r.id}>
                <label><input type="checkbox" checked={selected.includes(r.id)} disabled={!!r.error || busy}
                  onChange={(e) => setSelected((ids) => e.target.checked ? [...ids, r.id] : ids.filter((id) => id !== r.id))} /> <SuggestionText item={r} preview={preview} /></label>
              </li>)}</ul>
              <button disabled={!job || selected.length === 0 || busy || !c.history.data} onClick={() => void run(async () => {
                const data = await api.saveRefinement({ action: "approve", job: job!, selected, revision: c.history.data!.revision });
                if (sameProject()) { c.update(data); c.choose(data.active); close(); }
              })}>Apply and save</button>
            </>}
          </div>
        </section>
      </div>}
    </>
  );
}

function SuggestionText({ item, preview }: { item: Reviewed; preview?: RefinementPreview }) {
  const s = item.suggestion;
  const labels = new Map(preview?.focus_nodes);
  const evidence = preview ? (JSON.parse(preview.input) as {
    evidence: { id: string; text: string; file: string | null; line: number | null }[];
  }).evidence : [];
  return <span className="refinement-suggestion-text">
    <strong>{s.kind}{s.target && " · " + (labels.get(s.target) ?? s.target)}</strong>
    <span>{s.text}</span>
    {s.members.length > 0 && <small>{s.members.map((id) => labels.get(id) ?? id).join(", ")}</small>}
    {evidence.filter((e) => s.evidence.includes(e.id)).map((e) =>
      <small key={e.id}>{e.file ? e.file + (e.line ? ":" + e.line : "") + " · " : ""}{e.text}</small>)}
    {item.error && <small role="status">{item.error}</small>}
  </span>;
}

export function RefinementEvidence({ diagram, ids, historical }: { diagram: Refined; ids: string[]; historical: boolean }) {
  const byId = new Map(diagram.evidence.map((e) => [e.id, e]));
  return <ul className="architecture-evidence">
    {[...new Set(ids)].flatMap((id) => {
      const e = byId.get(id);
      if (!e) return [];
      return [<li key={id}><span>{e.text}</span>
        {e.site && !historical
          ? <button onClick={() => useUi.getState().focusCode(e.site!)}>{e.path}:{e.site.start_line}</button>
          : e.path && <small>{e.path}{e.site ? ":" + e.site.start_line : ""}</small>}
      </li>];
    })}
  </ul>;
}

export function RefinementSummary({ diagram, historical }: { diagram: Refined; historical: boolean }) {
  const summaries = diagram.suggestions.filter((r) => r.suggestion.kind === "summary" && !r.suggestion.target);
  return <div className="refinement-summary">
    {historical && <p role="status">Historical version. Evidence locations refer to the saved revision.</p>}
    {summaries.map((r) => <p key={r.id}>{r.suggestion.text}</p>)}
  </div>;
}
