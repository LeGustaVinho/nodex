import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo, useMemo } from "react";
import { SHAPE_ICON, conceptColor, conceptInfo, flavorLabel, operationInfo, typeColor, useCatalog } from "../catalog";
import { pointerBadge } from "../graph/pointers";
import type { Pin } from "../gen/Pin";
import type { BlueprintData } from "../graph/flowGraph";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { hasBreakpoint, toggleBreakpoint, useDebug } from "../debug";

function PinView({ pin, side, value }: { pin: Pin; side: "in" | "out"; value?: string }) {
  const exec = pin.kind === "Exec";
  const color = exec ? "#e8e8e8" : typeColor(pin.ty);
  const pointer = exec ? undefined : pointerBadge(pin.ty);
  return (
    <div className={`pin pin-${side} ${exec ? "pin-exec" : "pin-data"}`} title={pin.ty ? `${pin.label}: ${pin.ty}` : pin.label}>
      <Handle
        type={side === "in" ? "target" : "source"}
        position={side === "in" ? Position.Left : Position.Right}
        id={pin.id}
        className={exec ? "handle-exec" : "handle-data"}
        style={{ "--pin": color } as React.CSSProperties}
        isConnectable={false}
      />
      <span className="pin-label">{pin.label}</span>
      {pointer && (
        <span className="pin-ptr" title={pointer.explain}>
          {pointer.label}
        </span>
      )}
      {!exec && value !== undefined ? (
        <span className="pin-value" title={value}>
          {value}
        </span>
      ) : (
        !exec && pin.ty && <span className="pin-type">{pin.ty}</span>
      )}
    </div>
  );
}

const EXPR_PIN: Pin = { id: "expr", kind: "Data", label: "details", ty: null };

function BlueprintNodeImpl({ data, selected }: NodeProps) {
  const { node, expanded, language, flow, argPins, hiddenInputs, canExpand } = data as unknown as BlueprintData;
  const catalog = useCatalog();
  // Live execution overlay.
  const count = useTrace((s) => s.counts[flow]?.[node.id]);
  const throws = useTrace((s) => s.throws[flow]?.[node.id]);
  const liveValue = useTrace((s) => s.values[flow]?.[node.id]);
  const vars = useTrace((s) => s.vars[flow]);
  const active = useTrace((s) => s.active.some((a) => a.flow === flow && a.node === node.id));
  const replayStep = useTrace((s) => (s.replay?.flow === flow ? s.replay.page.steps[s.replay.index] : undefined));
  const replaying = replayStep !== undefined;
  const isReplayNode = replayStep?.node === node.id;
  const shownValue = replaying ? (isReplayNode && replayStep.values.length ? replayStep.values.join(", ") : undefined) : liveValue;
  const toggle = useUi((s) => s.toggleExpanded);
  // Debugger: breakpoints by the statement's first line; the node the program is paused at.
  const file = node.span.file;
  const line = node.span.start_line;
  const breakable = node.concept !== "Entry" && node.concept !== "EventEntry";
  const breakpoint = useDebug((s) => breakable && hasBreakpoint(s.breakpoints, file, line));
  const pausedHere = useDebug((s) => {
    const f = s.stopped?.frames.find((x) => x.id === s.frame) ?? s.stopped?.frames.find((x) => x.flow);
    return !!f && f.flow === flow && f.node === node.id;
  });
  // Values of the selected frame, on that frame's Blueprint only. Selected as stable references;
  // the name → value map is derived (a selector returning a new object each call would make
  // zustand re-render forever).
  const stopped = useDebug((s) => s.stopped);
  const frameId = useDebug((s) => s.frame);
  const frameScopes = useDebug((s) => s.scopes);
  const pausedVars = useMemo(() => {
    const f = stopped?.frames.find((x) => x.id === frameId);
    if (!f || f.flow !== flow || !frameScopes) return undefined;
    const out: Record<string, string> = {};
    for (const scope of frameScopes) for (const v of scope.variables) out[v.name] ??= v.value;
    return out;
  }, [stopped, frameId, frameScopes, flow]);
  const info = conceptInfo(catalog, node.concept);
  const color = conceptColor(catalog, node.concept);
  // Compact levels: the call's arguments are the statement's own pins, values inline.
  const args: Pin[] = (argPins ?? []).map((p, i) => ({ id: p.id, kind: "Data", label: p.name ?? `argument ${i + 1}`, ty: p.ty }));
  const argValues = new Map((argPins ?? []).map((p) => [p.id, p.value ?? undefined]));
  const own = hiddenInputs?.length ? node.inputs.filter((p) => !hiddenInputs.includes(p.id)) : node.inputs;
  const inputs = argPins ? [...own, ...args] : expanded ? [...own, EXPR_PIN] : own;
  // A statement that performs a shared operation with an effect (print, write file) is titled by it.
  const op = node.expr && node.concept === "Call" ? operationInfo(catalog, node.expr.op) : undefined;
  const rows = Math.max(inputs.length, node.outputs.length);
  const unreachable = node.flavors.includes("unreachable");
  const visibleFlavors = node.flavors.filter((f) => f !== "unreachable");

  return (
    <div
      className={`bp shape-${info.shape} ${selected ? "selected" : ""} ${pausedHere ? "paused-here" : ""} ${breakpoint ? "has-breakpoint" : ""} ${unreachable ? "unreachable" : ""} ${active && !replaying ? "active" : ""} ${isReplayNode ? "replay-current" : ""} ${count && !replaying ? "visited" : ""} ${replaying && !isReplayNode ? "dimmed" : ""} ${throws && !replaying ? "threw" : ""}`}
      style={{ "--accent": color } as React.CSSProperties}
    >
      <div className="bp-head">
        {breakable && (
          <button
            className={`bp-break nodrag ${breakpoint ? "on" : ""}`}
            title={breakpoint ? "Remove the breakpoint (F9)" : `Stop here when debugging (line ${line}, F9)`}
            onClick={(e) => {
              e.stopPropagation();
              void toggleBreakpoint(file, line);
            }}
          >
            ●
          </button>
        )}
        <span className="bp-icon">{SHAPE_ICON[info.shape] ?? "▸"}</span>
        <span className="bp-title" title={op?.explain}>
          {op ? op.title : info.title}
        </span>
        {count !== undefined && !replaying && (
          <span className="bp-count" title={`ran ${count} time${count === 1 ? "" : "s"}`}>
            ×{count}
          </span>
        )}
        {throws !== undefined && !replaying && (
          <span className="bp-throws" title={`threw ${throws} exception${throws === 1 ? "" : "s"}`}>
            ⚠ {throws}
          </span>
        )}
        {node.expr && canExpand !== false && (
          <button
            className="bp-expand nodrag"
            title={expanded ? "Hide expression details" : "Show expression details"}
            onClick={(e) => {
              e.stopPropagation();
              toggle(node.id, expanded);
            }}
          >
            {expanded ? "−" : "+"}
          </button>
        )}
      </div>
      <div className="bp-label">{node.label}</div>
      {shownValue && (
        <div className={`bp-live ${shownValue.startsWith("⚠") ? "bp-live-error" : ""}`} title={shownValue}>
          {shownValue}
        </div>
      )}
      <div className="bp-pins">
        {Array.from({ length: rows }, (_, i) => (
          <div className="pin-row" key={i}>
            <div className="pin-cell">{inputs[i] && <PinView pin={inputs[i]} side="in" value={argValues.get(inputs[i].id)} />}</div>
            <div className="pin-cell right">
              {node.outputs[i] && (
                <PinView pin={node.outputs[i]} side="out" value={replaying ? undefined : (pausedVars?.[node.outputs[i].label] ?? vars?.[node.outputs[i].label])} />
              )}
            </div>
          </div>
        ))}
      </div>
      {visibleFlavors.length > 0 && (
        <div className="badges">
          {visibleFlavors.map((f) => (
            <span key={f} className={`badge ${f.startsWith("unity:") ? "badge-unity" : ""}`}>
              {flavorLabel(catalog, f, language)}
            </span>
          ))}
        </div>
      )}
      <div className="bp-code" title={node.code}>
        {node.code}
      </div>
    </div>
  );
}

export const BlueprintNode = memo(BlueprintNodeImpl);
