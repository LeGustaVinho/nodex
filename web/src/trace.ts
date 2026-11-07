import { create } from "zustand";
import type { ActiveNode } from "./gen/ActiveNode";
import type { OutputLine } from "./gen/OutputLine";
import type { RunSnapshot } from "./gen/RunSnapshot";
import type { RunStatus } from "./gen/RunStatus";
import type { StepsPage } from "./gen/StepsPage";
import type { TraceFrame } from "./gen/TraceFrame";

type Nested<T> = Record<string, Record<string, T>>;

const MAX_OUTPUT = 2000;

/** Live execution state, fed by `trace-frame`, `run-status` and `run-output` events. */
interface TraceState {
  status: RunStatus;
  output: OutputLine[];
  counts: Nested<number>;
  calls: Record<string, number>;
  values: Nested<string>;
  vars: Nested<string>;
  throws: Nested<number>;
  active: ActiveNode[];
  events: number;
  dropped: number;
  consoleOpen: boolean;
  /** Replay of one flow's recorded steps; `index` is the step shown. */
  replay: { flow: string; page: StepsPage; index: number } | null;
  setStatus: (s: RunStatus) => void;
  appendOutput: (lines: OutputLine[]) => void;
  applyFrame: (f: TraceFrame) => void;
  loadSnapshot: (s: RunSnapshot) => void;
  setConsoleOpen: (open: boolean) => void;
  setReplay: (r: TraceState["replay"]) => void;
  setReplayIndex: (i: number) => void;
}

const merge = <T,>(base: Nested<T>, delta: Record<string, Record<string, T> | undefined>): Nested<T> => {
  const out = { ...base };
  for (const [flow, entries] of Object.entries(delta)) {
    if (entries) out[flow] = { ...(out[flow] ?? {}), ...entries };
  }
  return out;
};

const empty = { counts: {}, calls: {}, values: {}, vars: {}, throws: {}, active: [], events: 0, dropped: 0, replay: null };

export const useTrace = create<TraceState>((set) => ({
  status: { state: "idle", detail: "", exit_code: null },
  output: [],
  ...empty,
  consoleOpen: false,
  setStatus: (status) =>
    set((s) =>
      status.state === "preparing"
        ? { status, ...empty, output: [], consoleOpen: true }
        : { status, active: status.state === "running" ? s.active : [] },
    ),
  appendOutput: (lines) => set((s) => ({ output: [...s.output, ...lines].slice(-MAX_OUTPUT) })),
  applyFrame: (f) =>
    set((s) => ({
      counts: merge(s.counts, f.counts),
      calls: { ...s.calls, ...(f.calls as Record<string, number>) },
      values: merge(s.values, f.values),
      vars: merge(s.vars, f.vars),
      throws: merge(s.throws, f.throws),
      active: f.active,
      events: f.events,
      dropped: f.dropped,
    })),
  loadSnapshot: (snap) =>
    set(() => ({
      status: snap.status,
      output: snap.output,
      ...(snap.trace
        ? {
            counts: snap.trace.counts as Nested<number>,
            calls: snap.trace.calls as Record<string, number>,
            values: snap.trace.values as Nested<string>,
            vars: snap.trace.vars as Nested<string>,
            throws: snap.trace.throws,
            active: snap.status.state === "running" ? snap.trace.active : [],
            events: snap.trace.events,
            dropped: snap.trace.dropped,
          }
        : {}),
    })),
  setConsoleOpen: (consoleOpen) => set({ consoleOpen }),
  setReplay: (replay) => set({ replay }),
  setReplayIndex: (index) => set((s) => (s.replay ? { replay: { ...s.replay, index } } : {})),
}));

/** True while a recording exists (counts) for any flow. */
export const hasRecording = (s: TraceState) => s.events > 0;
