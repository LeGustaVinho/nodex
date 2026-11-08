import { type QueryClient, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../api";
import { debugActive, hiddenFrames, showDebugFrame, toggleBreakpoint, topFrame, useDebug, visibleFrames } from "../debug";
import type { FlowGraph } from "../gen/FlowGraph";
import type { AdapterStatus } from "../gen/AdapterStatus";
import type { DebugVariable } from "../gen/DebugVariable";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { useStructure } from "../views/Views";

type Command = "continue" | "next" | "stepIn" | "stepOut" | "pause" | "stop";

function send(command: Command) {
  void api.debugCommand(command).catch((e: Error) => console.warn(e.message));
}

/** Debug / Continue / Step controls for the top bar; F5, F10, F11, Shift+F11, Shift+F5. */
export function DebugControls() {
  const status = useDebug((s) => s.status);
  const runBusy = useTrace((s) => s.status.state === "preparing" || s.status.state === "running");
  const setConsoleOpen = useTrace((s) => s.setConsoleOpen);
  const active = debugActive(status.state);
  const paused = status.state === "paused";
  queryClientRef.current = useQueryClient();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
      const state = useDebug.getState().status.state;
      const isActive = debugActive(state);
      // F5 always belongs to the debugger here: it never reloads Nodex.
      if (e.key === "F5" && e.shiftKey) {
        if (isActive) send("stop");
      } else if (e.key === "F5") {
        if (!isActive && !runBusy) startDebugging(setConsoleOpen);
        else if (state === "paused") send("continue");
      }
      else if (e.key === "F10" && state === "paused") send("next");
      else if (e.key === "F11" && e.shiftKey && state === "paused") send("stepOut");
      else if (e.key === "F11" && state === "paused") send("stepIn");
      else if (e.key === "F9") toggleSelectedNode();
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [runBusy, setConsoleOpen]);

  if (!active) {
    return (
      <button
        className="debug-btn"
        disabled={runBusy}
        onClick={() => startDebugging(setConsoleOpen)}
        title="Build for debugging and run under the debugger: stops at breakpoints (F5). Set breakpoints with the ● on a node or in the code margin (F9)."
      >
        🐞 Debug
      </button>
    );
  }
  return (
    <div className="debug-controls" role="toolbar" aria-label="Debugger">
      {status.state === "starting" && <span className="muted">building…</span>}
      {paused ? (
        <button onClick={() => send("continue")} title="Continue (F5)">
          ▶
        </button>
      ) : (
        <button onClick={() => send("pause")} disabled={status.state !== "running"} title="Pause">
          ❚❚
        </button>
      )}
      <button onClick={() => send("next")} disabled={!paused} title="Step over: run this statement (F10)">
        ↷
      </button>
      <button onClick={() => send("stepIn")} disabled={!paused} title="Step into the call (F11)">
        ↓
      </button>
      <button onClick={() => send("stepOut")} disabled={!paused} title="Step out of this function (Shift+F11)">
        ↑
      </button>
      <button className="stop" onClick={() => send("stop")} title="Stop debugging (Shift+F5)">
        ■
      </button>
    </div>
  );
}

/** F9: breakpoint on the selected Blueprint node. */
function toggleSelectedNode() {
  const sel = useUi.getState().selection;
  if (sel?.kind !== "flow-node") return;
  const flow = queryClientRef.current?.getQueryData<FlowGraph>(["flow", sel.flow]);
  const node = flow?.nodes.find((n) => n.id === sel.id);
  if (node && node.concept !== "Entry" && node.concept !== "EventEntry") void toggleBreakpoint(node.span.file, node.span.start_line);
}

/** Set by DebugControls so F9 can read the open flow. */
const queryClientRef: { current: QueryClient | null } = { current: null };

function startDebugging(setConsoleOpen: (open: boolean) => void) {
  setConsoleOpen(true);
  void api.debugStart().catch(() => {
    /* a missing adapter is reported through the debug status (install card) */
  });
}

function Variable({ v, depth }: { v: DebugVariable; depth: number }) {
  const [open, setOpen] = useState(false);
  const children = useQuery({ queryKey: ["debug-vars", v.reference], queryFn: () => api.debugVariables(v.reference), enabled: open && v.reference > 0 });
  return (
    <div className="dbg-var" style={{ paddingLeft: depth * 12 }}>
      <div className="dbg-var-row" onClick={() => v.reference > 0 && setOpen(!open)}>
        <span className="dbg-caret">{v.reference > 0 ? (open ? "▾" : "▸") : ""}</span>
        <span className="dbg-name">{v.name}</span>
        <span className="dbg-value" title={v.value}>
          {v.value}
        </span>
        {v.ty && <span className="muted dbg-type">{v.ty}</span>}
      </div>
      {open && children.data?.map((c) => <Variable key={c.name} v={c} depth={depth + 1} />)}
    </div>
  );
}

/** Starts a download; the button shows progress at once (no second click while it starts). */
function installAdapter(language: AdapterStatus["language"], size: number) {
  const { setProgress } = useDebug.getState();
  setProgress({ language, received: 0, total: size, done: false, error: null });
  void api.installAdapter(language).catch((e: Error) => setProgress({ language, received: 0, total: size, done: true, error: e.message.replace(/^\d+ /, "") }));
}

/** Missing adapter: what it is and an Install button (the user's explicit choice). */
function InstallCard({ language }: { language: string }) {
  const queryClient = useQueryClient();
  const adapters = useQuery({ queryKey: ["adapters"], queryFn: api.adapters });
  const progress = useDebug((s) => s.progress[language]);
  const adapter = adapters.data?.find((a) => a.language === language);
  useEffect(() => {
    if (progress?.done) void queryClient.invalidateQueries({ queryKey: ["adapters"] });
  }, [progress?.done, queryClient]);
  if (!adapter) return null;
  if (adapter.state === "installed" || adapter.state === "override") {
    return (
      <div className="dbg-card">
        <strong>{adapter.name} is installed.</strong> Press 🐞 Debug again.
      </div>
    );
  }
  const busy = progress && !progress.done;
  return (
    <div className="dbg-card">
      <div>
        Debugging {language} needs the <strong>{adapter.name}</strong> debug adapter ({adapter.version}, {(adapter.size / 1e6).toFixed(1)} MB) from{" "}
        <a href={adapter.homepage} target="_blank" rel="noreferrer">
          {adapter.homepage.replace("https://", "")}
        </a>
        . Nodex checks it against its published checksum and keeps it in your user folder.
      </div>
      {adapter.state === "unsupported" ? (
        <div className="muted">There is no build of it for this machine.</div>
      ) : (
        <button className="primary" disabled={busy} onClick={() => installAdapter(adapter.language, adapter.size)}>
          {busy ? `Downloading… ${progress.total ? Math.round((100 * progress.received) / progress.total) : 0}%` : "Install"}
        </button>
      )}
      {progress?.error && <div className="open-error">{progress.error}</div>}
    </div>
  );
}

/** Call stack, variables and watch expressions while debugging. */
export function DebugPanel() {
  const { status, stopped, frame, setFrame, setFrameScopes, scopes: frameScopes } = useDebug();
  const { index } = useStructure();
  const [watches, setWatches] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [allFrames, setAllFrames] = useState(false);
  const isTop = frame !== null && frame === topFrame(stopped);
  const scopes = useQuery({
    queryKey: ["debug-scopes", frame, stopped],
    queryFn: () => api.debugScopes(frame!),
    enabled: frame !== null && !!stopped && !isTop,
  });
  // Other frames' variables also go to the store, for the pins of their Blueprint.
  useEffect(() => {
    if (frame !== null && !isTop && scopes.data) setFrameScopes(frame, scopes.data);
  }, [frame, isTop, scopes.data, setFrameScopes]);
  const watchValues = useQuery({
    queryKey: ["debug-watch", watches, frame, stopped],
    queryFn: () => Promise.all(watches.map((w) => api.debugEvaluate(w, frame).catch((e: Error) => ({ name: w, value: e.message.replace(/^\d+ /, ""), ty: null, reference: 0 })))),
    enabled: !!stopped && watches.length > 0,
  });

  if (status.missing_adapter) return <InstallCard language={status.missing_adapter} />;
  if (!debugActive(status.state)) return null;
  const shownScopes = frameScopes ?? [];
  return (
    <div className="debug-panel">
      <div className="section-title">
        Debugger · {status.state === "paused" ? `paused (${stopped?.description ?? stopped?.reason ?? ""})` : status.state}
      </div>
      {stopped && (
        <>
          <div className="dbg-section">
            <div className="dbg-head">Call stack</div>
            {visibleFrames(stopped.frames, allFrames).map((f) => (
              <button
                key={f.id}
                className={`dbg-frame ${f.id === frame ? "on" : ""} ${f.file ? "" : "external"}`}
                onClick={() => {
                  setFrame(f.id);
                  showDebugFrame(f, index);
                }}
                title={f.file ? `${f.file}:${f.line}` : "outside the project"}
              >
                <span>{f.name}</span>
                <span className="muted">{f.file ? `${f.file.split("/").pop()}:${f.line}` : ""}</span>
              </button>
            ))}
            {hiddenFrames(stopped.frames) > 0 && (
              <button className="dbg-more" onClick={() => setAllFrames(!allFrames)}>
                {allFrames ? "Show project frames" : `${hiddenFrames(stopped.frames)} more frames outside the project`}
              </button>
            )}
          </div>
          <div className="dbg-section">
            <div className="dbg-head">Variables</div>
            {shownScopes.map((s) => (
              <div key={s.reference}>
                <div className="dbg-scope">{s.name}</div>
                {s.variables.length > 0 ? (
                  s.variables.map((v) => <Variable key={v.name} v={v} depth={0} />)
                ) : (
                  <Variable v={{ name: "…", value: "open", ty: null, reference: s.reference }} depth={0} />
                )}
              </div>
            ))}
          </div>
          <div className="dbg-section">
            <div className="dbg-head">Watch</div>
            {watches.map((w, i) => {
              const v = watchValues.data?.[i];
              return (
                <div key={w} className="dbg-watch">
                  {v ? <Variable v={{ ...v, name: w }} depth={0} /> : <span className="muted">{w}</span>}
                  <button className="dbg-x" onClick={() => setWatches(watches.filter((x) => x !== w))} aria-label="Remove watch">
                    ✕
                  </button>
                </div>
              );
            })}
            <form
              onSubmit={(e) => {
                e.preventDefault();
                if (draft.trim() && !watches.includes(draft.trim())) setWatches([...watches, draft.trim()]);
                setDraft("");
              }}
            >
              <input className="dbg-input" value={draft} onChange={(e) => setDraft(e.target.value)} placeholder="Add an expression to watch" />
            </form>
          </div>
        </>
      )}
    </div>
  );
}
