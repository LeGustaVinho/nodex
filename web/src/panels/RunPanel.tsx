import { useEffect, useRef } from "react";
import { api } from "../api";
import { Splitter } from "../components/Splitter";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { errorText, toast } from "../toast";

/** Run/Stop control for the top bar. */
export function RunButton() {
  const status = useTrace((s) => s.status);
  const setConsoleOpen = useTrace((s) => s.setConsoleOpen);
  const busy = status.state === "preparing" || status.state === "running";
  const label = { idle: "Run", preparing: "Building…", running: "Running", exited: "Run again", failed: "Run" }[status.state];
  return (
    <div className="runbar">
      {busy ? (
        <button className="run stop" onClick={() => void api.runStop()} title="Stop the program">
          ■ {label}
        </button>
      ) : (
        <button
          className="run"
          title="Build the project with tracing and run it; the graphs light up as it executes"
          onClick={() => {
            setConsoleOpen(true);
            void api.runStart().catch((e: unknown) => toast.error(`Could not run: ${errorText(e)}`));
          }}
        >
          ▶ {label}
        </button>
      )}
      {status.state === "running" && status.url && (
        <a className="app-link" href={status.url} target="_blank" rel="noreferrer" title="The app runs with tracing: use it and watch the graphs">
          Open app ↗
        </a>
      )}
      {status.state === "exited" && <span className={`run-code ${status.exit_code === 0 ? "ok" : "bad"}`}>exit {status.exit_code ?? "?"}</span>}
      {status.state === "failed" && <span className="run-code bad" title={status.detail}>failed</span>}
    </div>
  );
}

/** Program and build output, docked at the bottom of the canvas. */
export function ConsolePanel() {
  const { output, consoleOpen, setConsoleOpen, status, events, dropped } = useTrace();
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [output.length, consoleOpen]);
  if (!consoleOpen) {
    return output.length > 0 || status.state !== "idle" ? (
      <button className="console-tab" onClick={() => setConsoleOpen(true)}>
        Console ({output.length})
      </button>
    ) : null;
  }
  return (
    <div className="console">
      <Splitter size="console-h" axis="y" target="parent" grow={-1} min={80} max={(main) => main.clientHeight - 60} label="Resize the console" className="edge-top" />
      <div className="console-head">
        <span>Console</span>
        <span className="muted">
          {status.state} {status.detail && `· ${status.detail}`} · {events.toLocaleString()} events
          {dropped > 0 && ` · ${dropped.toLocaleString()} dropped`}
        </span>
        <button onClick={() => setConsoleOpen(false)} title="Hide the console">
          ▾
        </button>
      </div>
      <div className="console-body">
        {output.map((l, i) => (
          <div key={i} className={`console-line stream-${l.stream}`}>
            {l.text}
          </div>
        ))}
        <div ref={end} />
      </div>
    </div>
  );
}

/** Step-by-step replay of the open Blueprint's recorded execution. */
export function ReplayBar({ flow, hasObservedTransition }: { flow: string; hasObservedTransition?: boolean }) {
  const { replay, setReplay, setReplayIndex, status } = useTrace();
  const calls = useTrace((s) => s.calls[flow] ?? 0);
  const focusCode = useUi((s) => s.focusCode);
  const active = replay?.flow === flow ? replay : null;

  if (calls === 0 && !active) return null;
  const load = async () => {
    const { codeIntent, flowSymbol } = useUi.getState();
    const page = await api.traceSteps(flow);
    const current = useUi.getState();
    if (current.view !== "blueprint" || current.flowSymbol !== flowSymbol || current.codeIntent !== codeIntent) return;
    setReplay(page.total > 0 ? { flow, page, index: 0 } : null);
  };
  if (!active) {
    return (
      <div className="replaybar">
        <span className="muted">
          ran {calls}× {status.state === "running" ? "(live)" : ""}
        </span>
        <button onClick={() => void load()} title="Step through what this function did, one statement at a time">
          ⏵ Replay
        </button>
      </div>
    );
  }
  const step = active.page.steps[active.index];
  const max = active.page.steps.length - 1;
  const go = (i: number) => setReplayIndex(Math.max(0, Math.min(max, i)));
  return (
    <div className="replaybar">
      <strong>Recorded execution</strong>
      <button onClick={() => go(active.index - 1)} disabled={active.index === 0} title="Previous step">
        ◀
      </button>
      <input type="range" min={0} max={max} value={active.index} onChange={(e) => go(Number(e.target.value))} />
      <button onClick={() => go(active.index + 1)} disabled={active.index === max} title="Next step">
        ▶
      </button>
      <span className="replay-pos">
        step {active.index + 1} / {active.page.total}
        {active.page.total > active.page.steps.length && ` (first ${active.page.steps.length})`}
      </span>
      {step && (
        <span className="replay-meta">
          t={(step.micros / 1000).toFixed(2)} ms · thread {step.thread}
          <span className="replay-values"> · {step.values.length ? step.values.join(" · ") : "No value recorded"}</span>
          {!step.node && <span> · Source step not mapped</span>}
          {step.node && step.index > 0 && <span> · {hasObservedTransition ? "Recorded transition" : "Transition not confirmed"}</span>}
        </span>
      )}
      <button onClick={() => void load()} title="Reload the recording">
        ↻
      </button>
      <button
        onClick={() => {
          setReplay(null);
          focusCode(null);
        }}
        title="Leave replay"
      >
        ✕
      </button>
    </div>
  );
}
