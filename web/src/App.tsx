import { useQuery, useQueryClient } from "@tanstack/react-query";
import { DETAIL_LEVELS } from "./graph/flowGraph";
import { Suspense, lazy, useEffect, useState } from "react";
import { ApiError, api, connectEvents, isDesktop, token } from "./api";
import { ConsolePanel, RunButton } from "./panels/RunPanel";
import { OpenDialog } from "./panels/OpenDialog";
import { DebugControls, DebugPanel } from "./panels/DebugPanel";
import { UnityCard } from "./panels/UnityCard";
import { loadDebug, useDebug } from "./debug";
import { useTrace } from "./trace";
import { Inspector } from "./panels/Inspector";
import { Palette } from "./panels/Palette";
import { Sidebar } from "./panels/Sidebar";
import { type View, useUi } from "./store";
import { RELATION_GROUPS } from "./graph/hierarchy";
import { ArchitectureView, BlueprintView, CallsView, TypesView, useStructure } from "./views/Views";
import { ConfidenceToggles } from "./components/ConfidenceToggles";
import { ContextMenuHost } from "./components/ContextMenu";
import { ConnectionOverlay, Toasts } from "./components/Toasts";
import { EnvironmentDialog } from "./panels/EnvironmentDialog";
import { SettingsDialog, useSettingsMenu } from "./panels/SettingsDialog";
import { Welcome } from "./panels/Welcome";
import * as perf from "./perf";

// Loaded on first use: Monaco, Mermaid and the knowledge views are large.
const CodePanel = lazy(() => import("./panels/CodePanel").then((m) => ({ default: m.CodePanel })));
const UmlView = lazy(() => import("./views/UmlView").then((m) => ({ default: m.UmlView })));
const TimelineView = lazy(() => import("./views/Timeline").then((m) => ({ default: m.TimelineView })));
const OverviewView = lazy(() => import("./views/Knowledge").then((m) => ({ default: m.OverviewView })));
const HierarchyView = lazy(() => import("./views/Knowledge").then((m) => ({ default: m.HierarchyView })));
const KnowledgeView = lazy(() => import("./views/Knowledge").then((m) => ({ default: m.KnowledgeView })));

const VIEWS: { id: View; label: string; hint: string }[] = [
  { id: "overview", label: "Overview", hint: "What the project is: entry points, routes, hotspots, data models" },
  { id: "hierarchy", label: "Hierarchy", hint: "Folders, namespaces, types and members as boxes you can open" },
  { id: "knowledge", label: "Knowledge", hint: "HTTP calls, dependency injection, tests, throws and writes around a symbol" },
  { id: "uml", label: "UML", hint: "Class, package and component diagrams of the project, a folder, a file or the selected code" },
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
    void loadDebug();
  }, []);
  useEffect(
    () =>
      connectEvents(
        (event) => {
          const trace = useTrace.getState();
          if (event.type === "run-status") trace.setStatus(event.data);
          else if (event.type === "run-output") trace.appendOutput(event.data.lines);
          else if (event.type === "trace-frame") {
            trace.applyFrame(event.data);
            refreshRecordedDiagrams(queryClient);
          }
          else if (event.type === "project-loading") {
            perf.openStarted();
            useUi.getState().setProjectLoading(event.data.path);
          }
          else if (event.type === "project-failed") useUi.getState().setProjectLoading(null);
          else if (event.type === "debug-status") useDebug.getState().setStatus(event.data);
          else if (event.type === "debug-continued") useDebug.getState().setStopped(null);
          else if (event.type === "adapter-progress") {
            useDebug.getState().setProgress(event.data);
            if (event.data.done) void queryClient.invalidateQueries({ queryKey: ["adapters"] });
          } else if (event.type === "debug-stopped") {
            useDebug.getState().setStopped(event.data);
            // Show where it stopped: the Blueprint of the top project frame, at its node.
            const top = event.data.frames.find((f) => f.flow);
            if (top?.flow) {
              const ui = useUi.getState();
              if (ui.flowSymbol !== top.flow || ui.view !== "blueprint") ui.openFlow(top.flow);
              if (top.node) ui.select({ kind: "flow-node", flow: top.flow, id: top.node });
            }
            void queryClient.invalidateQueries({ queryKey: ["debug-scopes"] });
            void queryClient.invalidateQueries({ queryKey: ["debug-watch"] });
          } else if (event.type === "project-changed") {
            perf.step("project-changed");
            // Everything shown belonged to the old project.
            trace.reset();
            useDebug.getState().reset();
            void queryClient.invalidateQueries({ queryKey: ["unity"] });
            void loadDebug();
            useUi.getState().resetForProject();
            queryClient.clear();
            return;
          }
          if (event.type === "graph-changed" || event.type === "semantic-changed") {
            const keys =
              event.type === "graph-changed"
                ? ["structure", "knowledge", "project", "flow", "source", "uml", "uml-availability"]
                : ["structure", "knowledge", "project", "flow", "uml", "uml-availability"];
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
  const [opening, setOpening] = useState(false);
  useEffect(() => {
    if (root) useUi.getState().loadDetail(root);
  }, [root]);
  return (
    <header className="topbar">
      <div className="brand">
        <span className="logo">◆</span> Nodex
        {project.data && (
          <span className="project-name" title={project.data.root}>
            {project.data.name}
          </span>
        )}
        <button className="open-btn" onClick={() => setOpening(true)} title="Open another project or folder">
          Open…
        </button>
        {opening && <OpenDialog onClose={() => setOpening(false)} />}
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
        {(ui.view === "knowledge" || ui.view === "uml") && (
          <>
            {ui.view === "knowledge" && <span className="chip" title={ui.knowledgeFocus ?? "Every knowledge relation of the project"}>
              {ui.knowledgeFocus ? (index?.byId.get(ui.knowledgeFocus)?.name ?? ui.knowledgeFocus) : "whole project"}
              {ui.knowledgeFocus && (
                <button className="chip-x" onClick={() => ui.showKnowledge(null)} title="Show every knowledge relation">
                  ×
                </button>
              )}
            </span>}
            {ui.view === "knowledge" && ui.knowledgeFocus && (
              <label className="toggle" title="How many steps of relations to follow">
                depth
                <select value={ui.knowledgeDepth} onChange={(e) => ui.setKnowledgeDepth(Number(e.target.value))}>
                  {[1, 2, 3].map((d) => (
                    <option key={d} value={d}>
                      {d}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {ui.view === "knowledge" && <ConfidenceToggles />}
          </>
        )}
        {(ui.view === "hierarchy" || ui.view === "knowledge") && (
          <>
            <div className="segmented" role="group" aria-label="Relations">
              {RELATION_GROUPS.map((g) => (
                <button
                  key={g.id}
                  className={ui.relations.includes(g.id) ? "on" : ""}
                  aria-pressed={ui.relations.includes(g.id)}
                  title={`Show ${g.label} relations`}
                  onClick={() => ui.toggleRelation(g.id)}
                >
                  {g.label}
                </button>
              ))}
            </div>
            {ui.view === "hierarchy" && (
              <button onClick={ui.collapseTree} title="Close every box">
                Collapse all
              </button>
            )}
          </>
        )}
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
      <button className="palette-button" onClick={() => ui.setPaletteOpen(true)} title="Search symbols and commands (Ctrl+K)">
        Search <kbd>Ctrl K</kbd>
      </button>
      <RunButton />
      <DebugControls />
      <SemanticBadge />
      <span className={`status ${ui.connected ? "on" : "off"}`} title={ui.connected ? "Live: updates on save" : "Disconnected"}>
        ●
      </span>
      <button className="icon-button" onClick={() => ui.setSettingsOpen(true)} title="Settings (Ctrl+,)" aria-label="Settings">
        ⚙
      </button>
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
    <button
      className={`semantic semantic-${state}`}
      title={`${title}\n\nClick to see which tools are installed.`}
      onClick={() => useUi.getState().setEnvironmentOpen(true)}
    >
      {label}
    </button>
  );
}

let lastUmlRefresh = 0;

/** While a run records, activity and sequence diagrams show what ran: refresh them now and then. */
function refreshRecordedDiagrams(client: ReturnType<typeof useQueryClient>) {
  const ui = useUi.getState();
  if (ui.view !== "uml" || (ui.umlKind !== "Activity" && ui.umlKind !== "Sequence")) return;
  const now = Date.now();
  if (now - lastUmlRefresh < 2000) return;
  lastUmlRefresh = now;
  void client.invalidateQueries({ queryKey: ["uml"] });
}

export function App() {
  useLiveUpdates();
  useSettingsMenu();
  const view = useUi((s) => s.view);
  const project = useQuery({ queryKey: ["project"], queryFn: api.project, retry: false });

  if (!token || (project.error instanceof ApiError && project.error.status === 401)) {
    return (
      <div className="fatal">
        <h1>Nodex needs its session</h1>
        {isDesktop ? (
          <p>This window lost its connection to Nodex. Restart Nodex.</p>
        ) : (
          <p>
            Open this page from Nodex: the desktop app opens it for you, and <code>nodex open &lt;folder&gt;</code> prints a link with a
            one-time token that protects your code.
          </p>
        )}
      </div>
    );
  }

  const overlays = (
    <>
      <EnvironmentDialog />
      <SettingsDialog />
      <Toasts />
      <ConnectionOverlay />
    </>
  );

  if (project.data && !project.data.open) {
    return (
      <>
        <Welcome />
        {overlays}
      </>
    );
  }

  const loading = <div className="empty">Loading…</div>;
  return (
    <div className="app">
      <TopBar />
      <Sidebar />
      <main className="main">
        <Suspense fallback={loading}>
          {view === "overview" && <OverviewView />}
          {view === "hierarchy" && <HierarchyView />}
          {view === "knowledge" && <KnowledgeView />}
          {view === "uml" && <UmlView />}
          {view === "architecture" && <ArchitectureView />}
          {view === "types" && <TypesView />}
          {view === "calls" && <CallsView />}
          {view === "timeline" && <TimelineView />}
          {view === "blueprint" && <BlueprintView />}
        </Suspense>
        <ConsolePanel />
      </main>
      <section className="right">
        <UnityCard />
        <DebugPanel />
        <Inspector />
        <Suspense fallback={null}>
          <CodePanel />
        </Suspense>
      </section>
      <Palette />
      <ContextMenuHost />
      {overlays}
    </div>
  );
}
