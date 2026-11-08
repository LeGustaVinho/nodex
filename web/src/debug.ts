import { create } from "zustand";
import { api } from "./api";
import type { AdapterProgress } from "./gen/AdapterProgress";
import type { Breakpoint } from "./gen/Breakpoint";
import type { DebugFrame } from "./gen/DebugFrame";
import type { DebugStatus } from "./gen/DebugStatus";
import type { DebugStopped } from "./gen/DebugStopped";

/** Debugger state shared by the toolbar, the Blueprint, the code panel and the debug panel. */
interface DebugStore {
  status: DebugStatus;
  stopped: DebugStopped | null;
  /** The frame whose variables are shown (the top project frame by default). */
  frame: number | null;
  breakpoints: Breakpoint[];
  progress: Record<string, AdapterProgress>;
  setStatus: (s: DebugStatus) => void;
  setStopped: (s: DebugStopped | null) => void;
  setFrame: (id: number | null) => void;
  setBreakpoints: (b: Breakpoint[]) => void;
  setProgress: (p: AdapterProgress) => void;
  reset: () => void;
}

const idle: DebugStatus = { state: "idle", detail: "", missing_adapter: null };

export const useDebug = create<DebugStore>((set) => ({
  status: idle,
  stopped: null,
  frame: null,
  breakpoints: [],
  progress: {},
  setStatus: (status) =>
    set((s) => ({
      status,
      // Leaving the paused state forgets the stop.
      stopped: status.state === "paused" ? s.stopped : null,
      frame: status.state === "paused" ? s.frame : null,
    })),
  setStopped: (stopped) => set({ stopped, frame: stopped?.frames.find((f) => f.file)?.id ?? stopped?.frames[0]?.id ?? null }),
  setFrame: (frame) => set({ frame }),
  setBreakpoints: (breakpoints) => set({ breakpoints }),
  setProgress: (p) => set((s) => ({ progress: { ...s.progress, [p.language]: p } })),
  reset: () => set({ status: idle, stopped: null, frame: null, breakpoints: [], progress: {} }),
}));

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
