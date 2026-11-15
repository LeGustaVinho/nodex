import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { ApiError, api, isDesktop } from "../api";
import type { ArchitectureNodeData } from "../components/ArchitectureNode";
import { projectFileAction, saveFile } from "../desktop";
import type { ArchitectureDiagram } from "../gen/ArchitectureDiagram";
import type { ArchitectureEvidence } from "../gen/ArchitectureEvidence";
import type { ArchitectureNode } from "../gen/ArchitectureNode";
import type { Layer } from "../gen/Layer";
import { toUmlScope } from "../graph/context";
import type { GGraph } from "../graph/model";
import { type ArchitectureUi, useUi } from "../store";
import { errorText, toast } from "../toast";
import { ArchitectureEditor, useArchitectureSettings } from "./ArchitectureEditor";
import { exportCanvas, GraphCanvas } from "./GraphCanvas";
import { PackagesView, useStructure } from "./Views";
import { RefinementControls, useDiagramRefinement } from "./DiagramRefinement";
import { RefinedArchitecture } from "./RefinedArchitecture";

const modes = [
  ["packages", "Packages"], ["layers", "Layers"], ["c4", "C4"],
] as const;
const layers: Layer[] = ["Interface", "Application", "Domain", "Infrastructure", "Unclassified"];

function useDiagram() {
  const ui = useUi((s) => s.architecture);
  const items = useUi((s) => s.context);
  const session = useUi((s) => s.projectSession);
  const view = ui.mode === "layers" ? "layers" : ui.level;
  const scope = view === "layers" ? toUmlScope(items) : { kind: "Project" } as const;
  const expanded = view === "layers" ? ui.expanded : [];
  const container = view === "components" ? ui.container : null;
  return useQuery({
    queryKey: ["architecture", session, view, scope, expanded, container],
    queryFn: ({ signal }) => api.architecture(view, scope, expanded, container, signal),
    retry: (count, err) => count < 2 && (!(err instanceof ApiError) || err.status !== 404),
  });
}

function follow(node: ArchitectureNode, ui: ArchitectureUi) {
  const set = useUi.getState().setArchitecture;
  if (node.kind === "Layer" && node.layer) {
    const layer = node.layer;
    set({ expanded: ui.expanded.includes(layer) ? ui.expanded.filter((n) => n !== layer) : layers.filter((n) => n === layer || ui.expanded.includes(n)) });
  }
  if (node.kind === "System" && ui.level === "context") set({ level: "containers", selected: null });
  if (node.kind === "Application" && ui.level === "containers") set({ level: "components", container: node.id, selected: null });
}

export function architectureGraph(diagram: ArchitectureDiagram, ui: ArchitectureUi): GGraph {
  const { nodes, edges } = diagram;
  const parents = new Set(nodes.map((n) => n.parent).filter(Boolean));
  const vertical = ui.mode === "layers";
  const bands = nodes.filter((n) => n.kind === "Layer");
  const pair = (from: string, to: string) => JSON.stringify(from < to ? [from, to] : [to, from]);
  const counts = new Map<string, number>();
  const lanes = new Map<string, number>();
  for (const edge of edges) {
    const key = pair(edge.from, edge.to);
    lanes.set(edge.id, counts.get(key) ?? 0);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return {
    direction: vertical ? "DOWN" : "RIGHT",
    layoutLinks: bands.slice(1).map((n, i) => ({ source: bands[i].id, target: n.id })),
    nodes: nodes.map((node) => {
      const boundary = parents.has(node.id);
      const expandable = node.kind === "Layer" && Boolean(node.layer && diagram.modules.some((n) => n.layer === node.layer));
      const drillable = (node.kind === "System" && ui.level === "context") || (node.kind === "Application" && ui.level === "containers");
      const expanded = expandable && node.layer !== null && ui.expanded.includes(node.layer);
      const data: ArchitectureNodeData = {
        node, boundary, vertical,
        action: expandable || drillable ? {
          label: expandable ? (expanded ? "Collapse " : "Expand ") + node.name : "Open " + node.name,
          expanded: expandable ? expanded : undefined,
          run: () => follow(node, useUi.getState().architecture),
        } : undefined,
      };
      return { id: node.id, parent: node.parent ?? undefined, container: boundary, headerHeight: 51,
        type: "architecture", width: vertical && node.kind === "Layer" ? 260 : 214, height: 60, ports: [], data };
    }),
    edges: edges.map((e) => {
      const count = counts.get(pair(e.from, e.to)) ?? 1;
      const offset = ((lanes.get(e.id) ?? 0) - (count - 1) / 2) * 22 * (e.from < e.to ? 1 : -1);
      return {
        id: e.id, source: e.from, target: e.to, kind: "architecture", layout: !vertical, confidence: e.confidence,
        weight: e.count, data: { label: e.label, http: e.kind === "HTTP", offset },
      };
    }),
  };
}

export function ArchitectureView() {
  const ui = useUi((s) => s.architecture);
  const set = useUi((s) => s.setArchitecture);
  const items = useUi((s) => s.context);
  const target = ui.mode === "packages"
    ? { kind: "packages" as const, scope: toUmlScope(items) }
    : { kind: "architecture" as const, view: ui.mode === "layers" ? "layers" as const : ui.level,
        scope: ui.mode === "layers" ? toUmlScope(items) : { kind: "Project" as const },
        container: ui.mode === "c4" && ui.level === "components" ? ui.container : null };
  const refinement = useDiagramRefinement(target);
  return (
    <div className="architecture-workspace">
      <div className="uml-bar architecture-modes" role="group" aria-label="Architecture views">
        {modes.map(([mode, title]) => (
          <button key={mode} className={ui.mode === mode ? "on" : undefined} aria-pressed={ui.mode === mode} onClick={() => set({ mode, selected: null })}>
            {title}
          </button>
        ))}
        <span className="spacer" />
        <RefinementControls controller={refinement} />
      </div>
      {refinement.version ? <>
        {ui.mode === "c4" && <nav className="uml-bar" aria-label="C4 level">
          <button aria-current={ui.level === "context" ? "step" : undefined} onClick={() => set({ level: "context", selected: null })}>Context</button>
          <button aria-current={ui.level === "containers" ? "step" : undefined} onClick={() => set({ level: "containers", selected: null })}>Containers</button>
          {ui.level === "components" && <strong>Components</strong>}
        </nav>}
        <RefinedArchitecture key={refinement.version.id} diagram={refinement.version.diagram} historical={refinement.historical} follow={(node) => {
          if (ui.mode !== "c4") return false;
          if (ui.level === "context" && node.kind === "System") {
            set({ level: "containers", selected: null });
            return true;
          }
          if (ui.level === "containers" && node.kind === "Application") {
            set({ level: "components", container: node.id, selected: null });
            return true;
          }
          return false;
        }} />
      </> : ui.mode === "packages" ? <div className="architecture-content"><PackagesView /></div> : <ArchitectureCanvas />}
    </div>
  );
}

function ArchitectureCanvas() {
  const query = useDiagram();
  const diagram = query.data;
  const ui = useUi((s) => s.architecture);
  const set = useUi((s) => s.setArchitecture);
  const [editing, setEditing] = useState(false);
  const graph = useMemo(() => diagram && architectureGraph(diagram, ui), [diagram, ui.mode, ui.level, ui.expanded]);

  useEffect(() => {
    if (ui.level === "components" && ui.mode === "c4" && query.error instanceof ApiError && query.error.status === 404) {
      set({ level: "containers", container: null, selected: null });
      toast.info("The selected application is no longer in the system.");
    }
  }, [ui.level, ui.mode, query.error, set]);

  const saveMermaid = async () => {
    if (!diagram) return;
    try {
      await saveFile("architecture-" + (ui.mode === "layers" ? "layers" : ui.level) + ".mmd", "text/plain", diagram.mermaid);
    } catch (error) {
      toast.error(errorText(error));
    }
  };

  return (
    <>
      <div className="uml-bar architecture-toolbar">
        {ui.mode === "c4" ? (
          <nav aria-label="C4 level">
            <button aria-current={ui.level === "context" ? "step" : undefined} onClick={() => set({ level: "context", selected: null })}>Context</button>
            <span aria-hidden="true">›</span>
            <button aria-current={ui.level === "containers" ? "step" : undefined} onClick={() => set({ level: "containers", selected: null })}>Containers</button>
            {ui.level === "components" && <><span aria-hidden="true">›</span><strong>Components</strong></>}
          </nav>
        ) : <strong className="uml-title">{diagram?.title}</strong>}
        <span className="spacer" />
        {ui.mode === "c4" && <button disabled={!diagram} onClick={() => setEditing(true)}>Edit architecture</button>}
        <button disabled={!diagram} onClick={() => void saveMermaid()} title="Save Mermaid source">.mmd</button>
        <button disabled={!graph} onClick={exportCanvas} title="Save the diagram as PNG">.png</button>
      </div>
      {(diagram?.warnings.length || diagram?.omitted_nodes || diagram?.omitted_edges) ? (
        <div className="uml-warnings" role="status">
          {diagram.warnings.map((warning) => <div key={warning}>{warning}</div>)}
          {Boolean(diagram.omitted_nodes || diagram.omitted_edges) && <div>{diagram.omitted_nodes} boxes and {diagram.omitted_edges} relations omitted from this view.</div>}
        </div>
      ) : null}
      <div className="architecture-content">
        <GraphCanvas
          graph={graph}
          layoutKey={"architecture-" + (ui.mode === "layers" ? "layers" : ui.level + "-" + (ui.container ?? ""))}
          exportName={"architecture-" + (ui.mode === "layers" ? "layers" : ui.level)}
          selectedId={ui.selected}
          loadingText="Loading architecture…"
          error={query.error}
          onRetry={() => void query.refetch()}
          onNodeClick={(n) => set({ selected: n.id })}
          onNodeDoubleClick={(n) => {
            const node = diagram?.nodes.find((item) => item.id === n.id);
            if (node) follow(node, ui);
          }}
          onEdgeClick={(selected) => set({ selected })}
          onPaneClick={() => set({ selected: null })}
          savePositions={false}
          renderWholeGraph
          refit
        />
      </div>
      {editing && diagram && ui.mode === "c4" && <ArchitectureEditor diagram={diagram} close={() => setEditing(false)} />}
    </>
  );
}

function Evidence({ items }: { items: ArchitectureEvidence[] }) {
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  return (
    <ul className="architecture-evidence">
      {items.map((item, i) => (
        <li key={i}>
          <span>{item.text}</span>
          {item.span ? (
            <button onClick={() => useUi.getState().focusCode(item.span)} title={item.path ?? undefined}>{item.path}{item.line && ":" + item.line}</button>
          ) : item.path && isDesktop && project.data?.open ? (
            <button onClick={() => void projectFileAction("open", project.data.root, item.path!).catch((error) => toast.error(errorText(error)))}>{item.path}</button>
          ) : item.path ? <small>{item.path}</small> : null}
        </li>
      ))}
    </ul>
  );
}

export function ArchitectureInspector() {
  const query = useDiagram();
  const ui = useUi((s) => s.architecture);
  const set = useUi((s) => s.setArchitecture);
  const { index } = useStructure();
  const diagram = query.data;
  const node = diagram?.nodes.find((n) => n.id === ui.selected) ?? diagram?.modules.find((n) => n.id === ui.selected);
  const edge = diagram?.edges.find((e) => e.id === ui.selected);
  const flow = node?.evidence.find((item) => item.symbol && index?.byId.get(item.symbol)?.has_flow)?.symbol;
  const name = (id: string) => diagram?.nodes.find((n) => n.id === id)?.name ?? id;
  return (
    <div className="inspector architecture-inspector">
      <h3>{node?.name ?? (edge ? name(edge.from) + " → " + name(edge.to) : "Architecture")}</h3>
      {node && (
        <>
          <div className="muted small">{node.provenance}{node.technology && " · " + node.technology}</div>
          {node.kind === "Module" && node.layer && node.path && <LayerEditor path={node.path} layer={node.layer} />}
          {node.code && (
            <div className="architecture-actions">
              <button onClick={() => useUi.getState().showUml(node.code!, "Class")}>Classes</button>
              {flow && <button onClick={() => useUi.getState().openFlow(flow)}>Blueprint</button>}
            </div>
          )}
          <Evidence items={node.evidence} />
          {node.kind === "Layer" && diagram?.modules.filter((m) => m.layer === node.layer).map((m) => (
            <button className="architecture-module-link" key={m.id} onClick={() => set({ selected: m.id })} title={m.path ?? undefined}>{m.name}</button>
          ))}
        </>
      )}
      {edge && (
        <>
          <div className="muted small">{edge.label} · {edge.provenance} · {edge.count} {edge.count === 1 ? "relation" : "relations"}</div>
          <Evidence items={edge.evidence} />
        </>
      )}
      {!node && !edge && diagram && (
        diagram.pending.length > 0 ? (
          <>
            <strong>Needs confirmation</strong>
            <Evidence items={diagram.pending} />
          </>
        ) : <p className="muted">Select a box or relation to inspect its evidence.</p>
      )}
    </div>
  );
}

function LayerEditor({ path, layer }: { path: string; layer: Layer }) {
  const settings = useArchitectureSettings();
  const client = useQueryClient();
  const session = useUi((s) => s.projectSession);
  const mutation = useMutation({
    mutationFn: api.saveArchitecture,
    onSuccess: (saved) => {
      if (useUi.getState().projectSession !== session) return;
      client.setQueryData(["architecture", session, "config"], saved);
      void client.invalidateQueries({ queryKey: ["architecture", session] });
    },
    onError: (error) => {
      if (useUi.getState().projectSession !== session) return;
      toast.error(errorText(error));
      void settings.refetch();
    },
  });
  return (
    <label className="architecture-layer-edit">
      Layer
      <select disabled={!settings.data || settings.isFetching || mutation.isPending}
        aria-label="Module layer"
        value={settings.data?.config.layers[path] ?? ""}
        onChange={(e) => {
          if (!settings.data) return;
          const overrides = { ...settings.data.config.layers };
          if (e.target.value) overrides[path] = e.target.value as Layer;
          else delete overrides[path];
          mutation.mutate({ ...settings.data, config: { ...settings.data.config, layers: overrides } });
        }}>
        <option value="">{settings.data?.config.layers[path] ? "Use inference" : "Automatic (" + layer + ")"}</option>
        {layers.map((choice) => <option key={choice}>{choice}</option>)}
      </select>
    </label>
  );
}
