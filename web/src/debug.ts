import { create } from "zustand";
import { api } from "./api";
import type { AdapterProgress } from "./gen/AdapterProgress";
import type { Breakpoint } from "./gen/Breakpoint";
import type { DebugFrame } from "./gen/DebugFrame";
import type { DebugScope } from "./gen/DebugScope";
import type { DebugStatus } from "./gen/DebugStatus";
import type { DebugStopped } from "./gen/DebugStopped";
import { classMembers } from "./graph/classBlueprint";
import { isTypeKind, type StructureIndex } from "./graph/structureGraph";
import { useUi } from "./store";

/** Debugger state shared by the toolbar, the Blueprint, the code panel and the debug panel. */
interface DebugStore {
  status: DebugStatus;
  stopped: DebugStopped | null;
  /** The frame whose variables are shown (the top project frame by default). */
  frame: number | null;
  /** The variables of `frame` (the stop carries the top frame's; others are fetched), or null
   *  while they load. */
  scopes: DebugScope[] | null;
  breakpoints: Breakpoint[];
  progress: Record<string, AdapterProgress>;
  setStatus: (s: DebugStatus) => void;
  setStopped: (s: DebugStopped | null) => void;
  setFrame: (id: number | null) => void;
  /** The fetched scopes of a frame other than the top one. */
  setFrameScopes: (id: number, scopes: DebugScope[]) => void;
  setBreakpoints: (b: Breakpoint[]) => void;
  setProgress: (p: AdapterProgress) => void;
  reset: () => void;
}

const idle: DebugStatus = { state: "idle", detail: "", missing_adapter: null };

export const useDebug = create<DebugStore>((set) => ({
  status: idle,
  stopped: null,
  frame: null,
  scopes: null,
  breakpoints: [],
  progress: {},
  setStatus: (status) =>
    set((s) => ({
      status,
      // Leaving the paused state forgets the stop.
      stopped: status.state === "paused" ? s.stopped : null,
      frame: status.state === "paused" ? s.frame : null,
      scopes: status.state === "paused" ? s.scopes : null,
    })),
  setStopped: (stopped) => set({ stopped, frame: topFrame(stopped), scopes: stopped?.scopes ?? null }),
  setFrame: (frame) => set((s) => ({ frame, scopes: frame !== null && frame === topFrame(s.stopped) ? (s.stopped?.scopes ?? null) : null })),
  setFrameScopes: (id, scopes) => set((s) => (s.frame === id ? { scopes } : {})),
  setBreakpoints: (breakpoints) => set({ breakpoints }),
  setProgress: (p) => set((s) => ({ progress: { ...s.progress, [p.language]: p } })),
  reset: () => set({ status: idle, stopped: null, frame: null, scopes: null, breakpoints: [], progress: {} }),
}));

/** The frame whose scopes a stop carries: the first one in the project (as the server picks it). */
export function topFrame(stopped: DebugStopped | null): number | null {
  return stopped?.frames.find((f) => f.file)?.id ?? stopped?.frames[0]?.id ?? null;
}

/** Reveal a frame in the current class when it belongs to one of its methods or accessors. */
export function showDebugFrame(frame: DebugFrame, index?: StructureIndex): void {
  const { flow, node, file, line } = frame;
  if (!flow) return;
  const ui = useUi.getState();
  const current = ui.view === "blueprint" ? ui.flowSymbol : null;
  const owner = current ? index?.byId.get(current) : undefined;
  if (index && owner && isTypeKind(owner.kind) && classMembers(index, owner.id).some((member) => member.id === flow)) {
    ui.setClassOpen([flow], true);
  } else if (current !== flow) {
    ui.openFlow(flow);
  }
  const span = file && line > 0 ? { file, start_line: line, end_line: line, start_col: 1, end_col: 1, start_byte: 0, end_byte: 0 } : undefined;
  ui.select(node ? { kind: "flow-node", flow, id: node } : { kind: "symbol", id: flow }, span);
}

export function debugActive(state: DebugStatus["state"]): boolean {
  return state === "starting" || state === "running" || state === "paused";
}

export function hasBreakpoint(list: Breakpoint[], file: string, line: number): boolean {
  return list.some((b) => b.file === file && b.line === line);
}

/** Adds or removes a breakpoint and saves the file's breakpoints. */
export async function toggleBreakpoint(file: string, line: number): Promise<void> {
  const { breakpoints, setBreakpoints } = useDebug.getState();
  const lines = breakpoints.filter((b) => b.file === file).map((b) => b.line);
  const next = lines.includes(line) ? lines.filter((l) => l !== line) : [...lines, line];
  setBreakpoints(await api.putBreakpoints(file, next));
}

/** Loads breakpoints and the current session (on start and after a project switch). */
export async function loadDebug(): Promise<void> {
  const store = useDebug.getState();
  try {
    store.setBreakpoints(await api.breakpoints());
    const snap = await api.debug();
    store.setStatus(snap.status);
    if (snap.stopped && snap.status.state === "paused") store.setStopped(snap.stopped);
  } catch {
    /* the server may be restarting */
  }
}

/** Frames up to the outermost project frame (runtime frames below it are folded), at most 50. */
export function visibleFrames(frames: DebugFrame[], all: boolean): DebugFrame[] {
  if (all) return frames.slice(0, 50);
  const last = frames.map((f) => !!f.file).lastIndexOf(true);
  return frames.slice(0, Math.max(last + 1, Math.min(frames.length, 1)));
}

export function hiddenFrames(frames: DebugFrame[]): number {
  return Math.min(frames.length, 50) - visibleFrames(frames, false).length;
}
