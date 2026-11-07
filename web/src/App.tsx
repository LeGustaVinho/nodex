import { useQuery, useQueryClient } from "@tanstack/react-query";
import { DETAIL_LEVELS } from "./graph/flowGraph";
import { useEffect } from "react";
import { ApiError, api, connectEvents, token } from "./api";
import { CodePanel } from "./panels/CodePanel";
import { ConsolePanel, RunButton } from "./panels/RunPanel";
import { useTrace } from "./trace";
import { Inspector } from "./panels/Inspector";
import { Sidebar } from "./panels/Sidebar";
import { type View, useUi } from "./store";
import { TimelineView } from "./views/Timeline";
import { ArchitectureView, BlueprintView, CallsView, TypesView, useStructure } from "./views/Views";

const VIEWS: { id: View; label: string; hint: string }[] = [
  { id: "architecture", label: "Architecture", hint: "Namespaces, modules, packages and their dependencies" },
  { id: "types", label: "Types", hint: "Classes, interfaces and how they relate" },
  { id: "calls", label: "Calls", hint: "Who calls a function, and what it calls" },
  { id: "timeline", label: "Timeline", hint: "Every function call of the last run, over time" },
  { id: "blueprint", label: "Blueprint", hint: "The steps inside one function" },
];

function useLiveUpdates() {
  const queryClient = useQueryClient();
  const setConnected = useUi((s) => s.setConnected);
  useEffect(() => {
    void api.run().then((snap) => useTrace.getState().loadSnapshot(snap)).catch(() => undefined);
  }, []);
  useEffect(
    () =>
      connectEvents(
        (event) => {
          const trace = useTrace.getState();
          if (event.type === "run-status") trace.setStatus(event.data);
          else if (event.type === "run-output") trace.appendOutput(event.data.lines);
          else if (event.type === "trace-frame") trace.applyFrame(event.data);
          if (event.type === "graph-changed" || event.type === "semantic-changed") {
            const keys = event.type === "graph-changed" ? ["structure", "project", "flow", "source"] : ["structure", "project", "flow"];
            for (const key of keys) {
              void queryClient.invalidateQueries({ queryKey: [key] });
            }
          }
        },
        setConnected,
      ),
    [queryClient, setConnected],
  );
}

function TopBar() {
  const ui = useUi();
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  const { index } = useStructure();
  const flowSymbol = ui.flowSymbol ? index?.byId.get(ui.flowSymbol) : undefined;
  const root = project.data?.root;
  useEffect(() => {
    if (root) useUi.getState().loadDetail(root);
  }, [root]);
  return (
    <header className="topbar">
      <div className="brand">
        <span className="logo">◆</span> Nodex
        {project.data && <span className="project-name">{project.data.name}</span>}
        {project.data?.flavors.includes("unity") && <span className="badge badge-unity">Unity</span>}
      </div>
      <nav className="tabs">
        {VIEWS.map((v) => (
          <button key={v.id} className={ui.view === v.id ? "active" : ""} title={v.hint} onClick={() => ui.setView(v.id)}>
            {v.label}
          </button>
        ))}
      </nav>
      <div className="context">
        {ui.view === "types" && (
          <>
            <span className="chip">
              {ui.namespace ?? "all namespaces"}
              {ui.namespace && (
                <button className="chip-x" onClick={() => ui.setNamespace(null)} title="Show all namespaces">
                  ×
                </button>
              )}
            </span>
            <label className="toggle">
              <input type="checkbox" checked={ui.showUses} onChange={(e) => ui.setShowUses(e.target.checked)} /> uses
            </label>
          </>
        )}
        {ui.view === "calls" && (
          <label className="toggle" title="How many steps of callers and callees to show">
            depth
            <select value={ui.callDepth} onChange={(e) => ui.setCallDepth(Number(e.target.value))}>
              {[1, 2, 3, 4].map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
          </label>
        )}
        {ui.view === "blueprint" && flowSymbol && (
          <>
            <span className="crumb" title={flowSymbol.signature ?? ""}>
              {flowSymbol.qualified_name}
            </span>
            <div className="segmented" role="group" aria-label="Detail">
              {DETAIL_LEVELS.map((l) => (
                <button
                  key={l.id}
                  className={ui.detail === l.id ? "on" : ""}
                  title={l.explain}
                  aria-pressed={ui.detail === l.id}
                  onClick={() => ui.setDetail(l.id)}
                >
                  {l.label}
                </button>
              ))}
            </div>
            <label className="toggle" title="Wires from where a variable is created to where it is used">
              <input type="checkbox" checked={ui.showDataWires} onChange={(e) => ui.setShowDataWires(e.target.checked)} /> data wires
            </label>
            {ui.detail === "statements" && (
              <button onClick={ui.collapseAll} title="Hide all expression details">
                Collapse
              </button>
            )}
          </>
        )}
      </div>
      <RunButton />
      <SemanticBadge />
      <span className={`status ${ui.connected ? "on" : "off"}`} title={ui.connected ? "Live: updates on save" : "Disconnected"}>
        ●
      </span>
    </header>
  );
}

/** Shows whether exact (compiler-grade) analysis is loading, ready or unavailable. */
function SemanticBadge() {
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  const status = project.data?.semantic ?? [];
  if (status.length === 0) return null;
  const state = status.some((s) => s.state === "loading") ? "loading" : status.every((s) => s.state === "ready") ? "ready" : "unavailable";
  const label = { loading: "analyzing…", ready: "semantics", unavailable: "syntax only" }[state];
  const title = status.map((s) => `${s.language}: ${s.detail}${s.messages.length ? "\n" + s.messages.join("\n") : ""}`).join("\n\n");
  return (
    <span className={`semantic semantic-${state}`} title={title}>
      {label}
    </span>
  );
}

export function App() {
  useLiveUpdates();
  const view = useUi((s) => s.view);
  const project = useQuery({ queryKey: ["project"], queryFn: api.project, retry: false });

  if (!token || (project.error instanceof ApiError && project.error.status === 401)) {
    return (
      <div className="fatal">
        <h1>Nodex needs its session link</h1>
        <p>Open the URL printed by <code>nodex open &lt;folder&gt;</code>. It contains a one-time token that protects your code.</p>
      </div>
    );
  }

  return (
    <div className="app">
      <TopBar />
      <Sidebar />
      <main className="main">
        {view === "architecture" && <ArchitectureView />}
        {view === "types" && <TypesView />}
        {view === "calls" && <CallsView />}
        {view === "timeline" && <TimelineView />}
        {view === "blueprint" && <BlueprintView />}
        <ConsolePanel />
      </main>
      <section className="right">
        <Inspector />
        <CodePanel />
      </section>
    </div>
  );
}
