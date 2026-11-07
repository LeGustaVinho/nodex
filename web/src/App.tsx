import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { ApiError, api, connectEvents, token } from "./api";
import { CodePanel } from "./panels/CodePanel";
import { Inspector } from "./panels/Inspector";
import { Sidebar } from "./panels/Sidebar";
import { type View, useUi } from "./store";
import { ArchitectureView, BlueprintView, TypesView, useStructure } from "./views/Views";

const VIEWS: { id: View; label: string; hint: string }[] = [
  { id: "architecture", label: "Architecture", hint: "Namespaces, packages and their dependencies" },
  { id: "types", label: "Types", hint: "Classes, interfaces and how they relate" },
  { id: "blueprint", label: "Blueprint", hint: "The steps inside one function" },
];

function useLiveUpdates() {
  const queryClient = useQueryClient();
  const setConnected = useUi((s) => s.setConnected);
  useEffect(
    () =>
      connectEvents(
        (event) => {
          if (event.type === "graph-changed") {
            for (const key of ["structure", "project", "flow", "source"]) {
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
        {ui.view === "blueprint" && flowSymbol && (
          <>
            <span className="crumb" title={flowSymbol.signature ?? ""}>
              {flowSymbol.qualified_name}
            </span>
            <label className="toggle" title="Wires from where a variable is created to where it is used">
              <input type="checkbox" checked={ui.showDataWires} onChange={(e) => ui.setShowDataWires(e.target.checked)} /> data wires
            </label>
            <button onClick={ui.collapseAll} title="Hide all expression details">
              Collapse
            </button>
          </>
        )}
      </div>
      <span className={`status ${ui.connected ? "on" : "off"}`} title={ui.connected ? "Live: updates on save" : "Disconnected"}>
        ●
      </span>
    </header>
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
        {view === "blueprint" && <BlueprintView />}
      </main>
      <section className="right">
        <Inspector />
        <CodePanel />
      </section>
    </div>
  );
}
