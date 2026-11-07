import type { Catalog } from "./gen/Catalog";
import type { FlowGraph } from "./gen/FlowGraph";
import type { ProjectSummary } from "./gen/ProjectSummary";
import type { StructureGraph } from "./gen/StructureGraph";

/** Session token: taken from `?token=` once, then kept for the tab's lifetime. */
function readToken(): string {
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

export type Positions = Record<string, { x: number; y: number }>;

export const api = {
  project: () => json<ProjectSummary>("/api/project"),
  structure: () => json<StructureGraph>("/api/structure"),
  catalog: () => json<Catalog>("/api/catalog"),
  flow: (symbol: string) => json<FlowGraph>(`/api/flow?symbol=${encodeURIComponent(symbol)}`),
  source: async (file: string) => (await request(`/api/source?file=${encodeURIComponent(file)}`)).text(),
  layout: (key: string) => json<Positions>(`/api/layout?key=${encodeURIComponent(key)}`),
  saveLayout: (key: string, positions: Positions) =>
    request(`/api/layout?key=${encodeURIComponent(key)}`, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(positions),
    }),
};

export type ServerEvent = { type: "graph-changed"; files: string[] };

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
