import type { BorrowOrigins } from "./gen/BorrowOrigins";
import type { UnityStatus } from "./gen/UnityStatus";
import type { CallsPage } from "./gen/CallsPage";
import type { Catalog } from "./gen/Catalog";
import type { FlowGraph } from "./gen/FlowGraph";
import type { FolderListing } from "./gen/FolderListing";
import type { AdapterProgress } from "./gen/AdapterProgress";
import type { AdapterStatus } from "./gen/AdapterStatus";
import type { Breakpoint } from "./gen/Breakpoint";
import type { DebugScope } from "./gen/DebugScope";
import type { EnvironmentReport } from "./gen/EnvironmentReport";
import type { SampleInfo } from "./gen/SampleInfo";
import type { Settings } from "./gen/Settings";
import type { DebugStatus } from "./gen/DebugStatus";
import type { DebugStopped } from "./gen/DebugStopped";
import type { DebugVariable } from "./gen/DebugVariable";
import type { Language } from "./gen/Language";
import type { RecentProject } from "./gen/RecentProject";
import type { OutputLine } from "./gen/OutputLine";
import type { UmlAvailability } from "./gen/UmlAvailability";
import type { UmlDiagram } from "./gen/UmlDiagram";
import type { UmlKind } from "./gen/UmlKind";
import type { UmlOptions } from "./gen/UmlOptions";
import type { UmlScope } from "./gen/UmlScope";
import type { ProjectKnowledge } from "./gen/ProjectKnowledge";
import type { ProjectSummary } from "./gen/ProjectSummary";
import type { RunSnapshot } from "./gen/RunSnapshot";
import type { RunStatus } from "./gen/RunStatus";
import type { StepsPage } from "./gen/StepsPage";
import type { StructureGraph } from "./gen/StructureGraph";
import type { TraceFrame } from "./gen/TraceFrame";
import type { NavigationKind, NavigationResult } from "./navigation";
import * as perf from "./perf";

declare global {
  interface Window {
    /** Set by the desktop app on the Nodex origin before the page runs. */
    __NODEX_TOKEN__?: string;
  }
}

/** True inside the Nodex desktop app. */
export const isDesktop = typeof window.__NODEX_TOKEN__ === "string";

/**
 * Session token: injected by the desktop app, or taken from `?token=` once and then kept for the
 * tab's lifetime.
 */
function readToken(): string {
  if (window.__NODEX_TOKEN__) return window.__NODEX_TOKEN__;
  const url = new URL(window.location.href);
  const fromUrl = url.searchParams.get("token");
  if (fromUrl) {
    try {
      sessionStorage.setItem("nodex-token", fromUrl);
    } catch {
      /* storage may be unavailable; the URL still has the token */
    }
    url.searchParams.delete("token");
    window.history.replaceState(null, "", url.toString());
    return fromUrl;
  }
  try {
    return sessionStorage.getItem("nodex-token") ?? "";
  } catch {
    return "";
  }
}

export const token = readToken();

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function request(path: string, init?: RequestInit): Promise<Response> {
  const res = await fetch(path, {
    ...init,
    headers: { "x-nodex-token": token, ...(init?.headers ?? {}) },
  });
  if (!res.ok) throw new ApiError(res.status, `${res.status} ${await res.text()}`);
  return res;
}

const json = async <T,>(path: string): Promise<T> => (await request(path)).json() as Promise<T>;

/** A large payload: logs its size and how long it took to download and parse. */
const bigJson = async <T,>(path: string): Promise<T> => {
  const started = performance.now();
  const text = await (await request(path)).text();
  const downloaded = performance.now();
  const value = JSON.parse(text) as T;
  perf.payload(path, text.length, downloaded - started, performance.now() - downloaded);
  return value;
};

/**
 * The server leaves symbol fields at their default out of `/api/structure` (a large project
 * has hundreds of thousands of symbols); put them back so the rest of the UI sees whole symbols.
 */
export function normalizeStructure(graph: StructureGraph): StructureGraph {
  for (const s of graph.symbols) {
    s.parent ??= null;
    s.origins ??= [];
    s.flavors ??= [];
    s.signature ??= null;
    s.type_text ??= null;
    s.has_flow ??= false;
    s.external ??= false;
    s.doc ??= null;
  }
  return graph;
}

export type Positions = Record<string, { x: number; y: number }>;

export type UnityPackageAction = "link" | "embed" | "update" | "remove";

export const api = {
  project: () => json<ProjectSummary>("/api/project"),
  structure: async () => normalizeStructure(await bigJson<StructureGraph>("/api/structure")),
  navigation: async (symbol: string, kind: NavigationKind, signal: AbortSignal): Promise<NavigationResult> =>
    (await request(`/api/navigation?symbol=${encodeURIComponent(symbol)}&kind=${kind}`, { signal })).json(),
  knowledge: () => bigJson<ProjectKnowledge>("/api/knowledge"),
  catalog: () => json<Catalog>("/api/catalog"),
  borrowOrigin: async (flow: string, node: string, pin: string | undefined, path: string | undefined, offset: number, signal: AbortSignal) => {
    const query = new URLSearchParams({ flow, node, offset: String(offset) });
    if (pin !== undefined) query.set("pin", pin);
    if (path !== undefined) query.set("expr_path", path);
    return (await request("/api/borrow-origin?" + query, { signal })).json() as Promise<BorrowOrigins>;
  },
  flow: (symbol: string) => json<FlowGraph>(`/api/flow?symbol=${encodeURIComponent(symbol)}`),
  flows: (symbols: readonly string[]) => json<FlowGraph[]>(`/api/flows?symbols=${symbols.map(encodeURIComponent).join(",")}`),
  source: async (file: string) => (await request(`/api/source?file=${encodeURIComponent(file)}`)).text(),
  sourceFile: async (file: string, signal: AbortSignal) => {
    const response = await request(`/api/source?file=${encodeURIComponent(file)}`, { signal });
    const bytes = new Uint8Array(await response.arrayBuffer());
    return { bytes, text: new TextDecoder().decode(bytes), revision: response.headers.get("x-nodex-source-revision") };
  },
  layout: (key: string) => json<Positions>(`/api/layout?key=${encodeURIComponent(key)}`),
  listFolders: (path: string) => json<FolderListing>(`/api/fs/list?path=${encodeURIComponent(path)}`),
  recent: () => json<RecentProject[]>("/api/recent"),
  environment: () => json<EnvironmentReport>("/api/environment"),
  samples: () => json<SampleInfo[]>("/api/samples"),
  settings: () => json<Settings>("/api/settings"),
  saveSettings: async (settings: Settings) =>
    (
      await request("/api/settings", { method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify(settings) })
    ).json() as Promise<Settings>,
  openSample: async (id: string) =>
    (
      await request("/api/samples/open", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ id }) })
    ).json() as Promise<ProjectSummary>,
  closeProject: async () => (await request("/api/project/close", { method: "POST" })).json() as Promise<ProjectSummary>,
  openProject: async (path: string) =>
    (
      await request("/api/project/open", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ path }),
      })
    ).json() as Promise<ProjectSummary>,
  debug: () => json<{ status: DebugStatus; stopped: DebugStopped | null }>("/api/debug"),
  debugStart: () => request("/api/debug/start", { method: "POST" }),
  debugCommand: (command: "continue" | "next" | "stepIn" | "stepOut" | "pause" | "stop") =>
    request("/api/debug/command", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ command }) }),
  breakpoints: () => json<Breakpoint[]>("/api/debug/breakpoints"),
  putBreakpoints: async (file: string, lines: number[]) =>
    (
      await request("/api/debug/breakpoints", {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ file, lines }),
      })
    ).json() as Promise<Breakpoint[]>,
  debugVariables: (reference: number) => json<DebugVariable[]>(`/api/debug/variables?reference=${reference}`),
  debugScopes: (frame: number) => json<DebugScope[]>(`/api/debug/scopes?frame=${frame}`),
  debugEvaluate: async (expression: string, frame: number | null) =>
    (
      await request("/api/debug/evaluate", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ expression, frame }),
      })
    ).json() as Promise<DebugVariable>,
  adapters: () => json<AdapterStatus[]>("/api/debug/adapters"),
  installAdapter: (language: Language) =>
    request("/api/debug/adapters/install", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ language }) }),
  unity: () => json<UnityStatus | null>("/api/unity"),
  unityPackage: (action: UnityPackageAction) =>
    request("/api/unity/package", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ action }) }),
  unityTraceOff: () => request("/api/unity/trace-off", { method: "POST" }),
  run: () => json<RunSnapshot>("/api/run"),
  runStart: () => request("/api/run", { method: "POST" }),
  runStop: () => request("/api/run/stop", { method: "POST" }),
  traceCalls: () => json<CallsPage>("/api/trace/calls"),
  traceTree: (root: number, limit = 5000) => json<CallsPage>(`/api/trace/tree?root=${root}&limit=${limit}`),
  uml: (kind: UmlKind, scope: UmlScope, opts: Partial<UmlOptions>) =>
    json<UmlDiagram>(`/api/uml?kind=${kind}&scope=${encodeURIComponent(JSON.stringify(scope))}&opts=${encodeURIComponent(JSON.stringify(opts))}`),
  umlAvailability: (scope: UmlScope) => json<UmlAvailability>(`/api/uml/availability?scope=${encodeURIComponent(JSON.stringify(scope))}`),
  traceSteps: (symbol: string) => json<StepsPage>(`/api/trace/steps?symbol=${encodeURIComponent(symbol)}`),
  saveLayout: (key: string, positions: Positions) =>
    request(`/api/layout?key=${encodeURIComponent(key)}`, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(positions),
    }),
};

/** How far opening a project got: files found (`scanning`, total 0) or read out of `total`. */
export type OpenProgress = { phase: "scanning" | "reading"; done: number; total: number };

export type ServerEvent =
  | { type: "graph-changed"; data: { files: string[] } }
  /** `graph` is false when only the analysis status changed (the graphs are the same). */
  | { type: "semantic-changed"; data: { graph: boolean } }
  | { type: "run-status"; data: RunStatus }
  | { type: "run-output"; data: { lines: OutputLine[] } }
  | { type: "trace-frame"; data: TraceFrame }
  | { type: "project-loading"; data: { path: string } }
  | { type: "project-progress"; data: OpenProgress }
  | { type: "project-changed" }
  | { type: "project-failed"; data: { error: string } }
  | { type: "adapter-progress"; data: AdapterProgress }
  | { type: "debug-status"; data: DebugStatus }
  | { type: "debug-stopped"; data: DebugStopped }
  | { type: "debug-continued" };

/** Connects to the live-update socket, reconnecting with backoff. Returns a disposer. */
export function connectEvents(onEvent: (e: ServerEvent) => void, onStatus: (connected: boolean) => void): () => void {
  let socket: WebSocket | null = null;
  let stopped = false;
  let delay = 500;
  const open = () => {
    const proto = window.location.protocol === "https:" ? "wss" : "ws";
    socket = new WebSocket(`${proto}://${window.location.host}/ws?token=${encodeURIComponent(token)}`);
    socket.onopen = () => {
      delay = 500;
      onStatus(true);
    };
    socket.onmessage = (m) => {
      try {
        onEvent(JSON.parse(m.data as string) as ServerEvent);
      } catch {
        /* ignore malformed events */
      }
    };
    socket.onclose = () => {
      onStatus(false);
      if (!stopped) {
        setTimeout(open, delay);
        delay = Math.min(delay * 2, 10_000);
      }
    };
  };
  open();
  return () => {
    stopped = true;
    socket?.close();
  };
}
