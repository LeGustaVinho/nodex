import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import type { CallSpan } from "../gen/CallSpan";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { useStructure } from "./Views";

const ROW = 22;
const LANE_GAP = 26;
const AXIS = 24;
const MAX_DRAWN = 6000;

/** Stable color per owning type, so calls into the same class share a hue. */
function hue(text: string): number {
  let h = 0;
  for (let i = 0; i < text.length; i++) h = (h * 31 + text.charCodeAt(i)) >>> 0;
  return h % 360;
}

function fmt(micros: number): string {
  if (micros >= 1_000_000) return `${(micros / 1_000_000).toFixed(2)} s`;
  if (micros >= 1000) return `${(micros / 1000).toFixed(micros >= 100_000 ? 0 : 1)} ms`;
  return `${Math.round(micros)} µs`;
}

/** Flame chart of function executions over time, one lane per thread. */
export function TimelineView() {
  const status = useTrace((s) => s.status.state);
  const events = useTrace((s) => s.events);
  const { index } = useStructure();
  const { openFlow, select } = useUi();
  /** The recorded sequence under one call: the scope is its function, the root the call. */
  const sequenceOf = (id: number) => {
    const span = page?.spans.find((s) => s.id === id);
    const ui = useUi.getState();
    ui.showUml(span ? { kind: "Symbol", id: span.flow } : { kind: "Project" }, "Sequence");
    ui.setUmlOpts({ runtime: true, focus: `span:${id}` });
  };
  const calls = useQuery({
    queryKey: ["trace-calls", status],
    queryFn: () => api.traceCalls(),
    enabled: events > 0,
    refetchInterval: status === "running" ? 1000 : false,
  });
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);
  const [view, setView] = useState<{ t0: number; t1: number } | null>(null);
  const drag = useRef<{ x: number; t0: number; t1: number } | null>(null);
  const [picked, setPicked] = useState<number | null>(null);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(el.clientWidth));
    ro.observe(el);
    setWidth(el.clientWidth);
    return () => ro.disconnect();
  }, [calls.data]);

  const page = calls.data;
  const range = page ? { t0: page.start, t1: Math.max(page.end, page.start + 1) } : { t0: 0, t1: 1 };
  const { t0, t1 } = view ?? range;
  const scale = width / Math.max(t1 - t0, 1);

  const lanes = useMemo(() => {
    const byThread = new Map<number, CallSpan[]>();
    for (const s of page?.spans ?? []) (byThread.get(s.thread) ?? byThread.set(s.thread, []).get(s.thread)!).push(s);
    let y = AXIS;
    return [...byThread.entries()]
      .sort(([a], [b]) => a - b)
      .map(([thread, spans]) => {
        const depth = Math.max(...spans.map((s) => s.depth)) + 1;
        const lane = { thread, spans, y: y + 16, height: depth * ROW };
        y += 16 + depth * ROW + LANE_GAP;
        return lane;
      });
  }, [page]);
  /** The picked call and its callers, up to the outermost one (or the call an async continuation resumes). */
  const chain = useMemo(() => {
    const out = new Set<number>();
    if (picked === null || !page) return out;
    const byId = new Map(page.spans.map((s) => [s.id, s]));
    let at: number | null | undefined = picked;
    while (at !== null && at !== undefined && !out.has(at)) {
      out.add(at);
      const s = byId.get(at);
      at = s ? (s.parent ?? s.resumes) : null;
    }
    return out;
  }, [picked, page]);
  const totalHeight = lanes.length ? lanes[lanes.length - 1].y + lanes[lanes.length - 1].height + 10 : 100;

  const label = (flow: string) => {
    const s = index?.byId.get(flow);
    const owner = s?.parent ? index?.owningType(s.parent)?.name : undefined;
    return { name: s?.name ?? flow.replace(/\(.*$/, "").split(".").pop() ?? flow, owner: owner ?? "" };
  };

  const openAt = async (span: CallSpan) => {
    openFlow(span.flow);
    const { codeIntent } = useUi.getState();
    const steps = await api.traceSteps(span.flow);
    const current = useUi.getState();
    if (current.view !== "blueprint" || current.flowSymbol !== span.flow || current.codeIntent !== codeIntent) return;
    const i = steps.steps.findIndex((st) => st.micros >= span.start && st.thread === span.thread);
    if (steps.total > 0) useTrace.getState().setReplay({ flow: span.flow, page: steps, index: Math.max(0, i) });
  };

  if (events === 0) {
    return <div className="empty">Run the program (▶ Run) to see every function call on a timeline.</div>;
  }
  if (!page) return <div className="empty">Loading the recording…</div>;

  // Draw the spans that are visible and at least a pixel wide; keep the widest if there are too many.
  let drawn = page.spans.filter((s) => s.end >= t0 && s.start <= t1 && (s.end - s.start) * scale >= 1);
  if (drawn.length > MAX_DRAWN) drawn = [...drawn].sort((a, b) => b.end - b.start - (a.end - a.start)).slice(0, MAX_DRAWN);
  const laneY = new Map(lanes.map((l) => [l.thread, l.y]));
  const ticks = Array.from({ length: 6 }, (_, i) => t0 + ((t1 - t0) * i) / 5);

  return (
    <div className="timeline">
      <div className="timeline-bar">
        <span className="muted">
          {page.total.toLocaleString()} calls · {fmt(page.end - page.start)} · wheel to zoom, drag to pan, double-click a call to replay it
        </span>
        {view && <button onClick={() => setView(null)}>Reset zoom</button>}
        {picked !== null && (
          <button onClick={() => sequenceOf(picked)} title="A UML sequence diagram of the picked call and every call made under it, as recorded">
            Sequence of this call
          </button>
        )}
        <button onClick={() => void calls.refetch()} title="Reload the recording">
          ↻
        </button>
      </div>
      <div
        className="timeline-canvas"
        ref={box}
        onWheel={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const at = t0 + (e.clientX - rect.left) / scale;
          // Proportional to the wheel movement: a notch zooms ~20%, a fast spin much more.
          const factor = Math.exp(Math.max(-600, Math.min(600, e.deltaY)) * 0.002);
          const nt0 = at - (at - t0) * factor;
          const nt1 = at + (t1 - at) * factor;
          setView(nt1 - nt0 >= range.t1 - range.t0 ? null : { t0: Math.max(range.t0, nt0), t1: Math.min(range.t1, nt1) });
        }}
        onMouseDown={(e) => (drag.current = { x: e.clientX, t0, t1 })}
        onMouseMove={(e) => {
          const d = drag.current;
          if (!d) return;
          const shift = (d.x - e.clientX) / scale;
          if (Math.abs(d.x - e.clientX) > 2) setView({ t0: d.t0 + shift, t1: d.t1 + shift });
        }}
        onMouseUp={() => (drag.current = null)}
        onMouseLeave={() => (drag.current = null)}
      >
        <svg width={width} height={totalHeight}>
          {ticks.map((t) => {
            const x = (t - t0) * scale;
            return (
              <g key={t}>
                <line x1={x} x2={x} y1={AXIS - 6} y2={totalHeight} className="tl-grid" />
                <text x={Math.min(x + 3, width - 60)} y={AXIS - 10} className="tl-tick">
                  {fmt(t - range.t0)}
                </text>
              </g>
            );
          })}
          {lanes.map((l) => (
            <text key={l.thread} x={4} y={l.y - 4} className="tl-lane">
              thread {l.thread}
            </text>
          ))}
          {drawn.map((s, i) => {
            const x = Math.max(0, (s.start - t0) * scale);
            const w = Math.max(1, Math.min(width, (s.end - t0) * scale) - x);
            const y = (laneY.get(s.thread) ?? 0) + s.depth * ROW;
            const { name, owner } = label(s.flow);
            const h = hue(owner || s.flow);
            return (
              <g
                key={i}
                className={`tl-span ${s.threw ? "threw" : ""} ${s.resumed ? "resumed" : ""} ${s.open ? "open" : ""} ${s.id === picked ? "picked" : chain.has(s.id) ? "chain" : ""}`}
                onClick={() => {
                  setPicked(s.id);
                  select({ kind: "symbol", id: s.flow }, index?.byId.get(s.flow)?.origins[0] ?? null);
                }}
                onDoubleClick={() => void openAt(s)}
              >
                <title>
                  {`${owner ? owner + "." : ""}${name}${s.resumed ? " (resumed)" : ""}${s.threw ? " — ended by an exception" : ""}${s.open ? " — still running" : ""}\n${fmt(s.end - s.start)} · starts at ${fmt(s.start - range.t0)} · thread ${s.thread}${s.args.length ? "\n(" + s.args.join(", ") + ")" : ""}${s.ret !== null ? "\n→ " + s.ret : ""}${s.exception !== null ? "\n⚠ " + s.exception : ""}`}
                </title>
                <rect x={x} y={y} width={w} height={ROW - 3} rx={3} style={{ fill: `hsl(${h} 45% 32%)`, stroke: `hsl(${h} 55% 55%)` }} />
                {w > 44 && (
                  <text x={x + 5} y={y + 14} className="tl-label">
                    {name.length * 6.5 > w - 8 ? name.slice(0, Math.max(1, Math.floor((w - 12) / 6.5))) + "…" : name}
                  </text>
                )}
              </g>
            );
          })}
        </svg>
      </div>
    </div>
  );
}
