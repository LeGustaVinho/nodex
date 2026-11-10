import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo, useMemo } from "react";
import { SHAPE_ICON, conceptColor, conceptInfo, flavorInfo, flavorLabel, operationInfo, useCatalog } from "../catalog";
import { type CodeLanguage, codeLanguage } from "../graph/codeTokens";
import type { CodePreview } from "../graph/blueprintSource";
import { pointerBadge } from "../graph/pointers";
import type { Pin } from "../gen/Pin";
import type { BlueprintData } from "../graph/flowGraph";
import { executionPinLabel, isExplanationFlavor, pinSyntaxDetails, resolvePinDetails, statementDetailFlavors } from "../graph/blueprintExplanation";
import { ACCESS_LABEL, ACCESS_TEXT } from "./OwnershipEdge";
import { CLASS_IN, CLASS_OUT } from "../graph/classBlueprint";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { hasBreakpoint, toggleBreakpoint, useDebug } from "../debug";
import { BlueprintCode } from "./HighlightedCode";
import { bindingDetails, type LanguageDetail } from "../graph/bindingDetails";
import { statementSites, syntaxBadges } from "../graph/syntaxSites";
import { ValueBadge } from "./ValueBadge";
import { LanguageBadges } from "./LanguageBadges";
import { TypeLabel } from "./TypeLabel";
import { blueprintTypeColor } from "../graph/blueprintTypes";

function PinView({ pin, side, value, code, syntax, label = pin.label, details, inspect, showKind }: {
  pin: Pin; side: "in" | "out"; value?: string; code?: CodePreview; syntax?: CodeLanguage; label?: string;
  showKind: boolean; details: readonly LanguageDetail[]; inspect: (site?: string) => void;
}) {
  const exec = pin.kind === "Exec";
  const color = exec ? "#e8e8e8" : blueprintTypeColor(pin.type_info?.shape, pin.ty);
  const pointer = exec ? undefined : pointerBadge(pin.ty);
  return (
    <div className={`pin pin-${side} ${exec ? "pin-exec" : "pin-data"}`} title={pin.ty ? `${label}: ${pin.ty}` : label}>
      <Handle
        type={side === "in" ? "target" : "source"}
        position={side === "in" ? Position.Left : Position.Right}
        id={pin.id}
        className={exec ? "handle-exec" : "handle-data"}
        style={{ "--pin": color } as React.CSSProperties}
        isConnectable={false}
      />
      <span className="pin-label">{label}</span>
      {!exec && (pin.id !== "expr" || pin.type_info) && <TypeLabel info={pin.type_info} fallback={pin.ty} onInspect={() => inspect()} />}
      {showKind && !exec && pin.id !== "expr" && <ValueBadge info={pin.value_info} onInspect={() => inspect()} />}
      <LanguageBadges details={details} onInspect={inspect} />
      {pointer && (
        <span className="pin-ptr" title={pointer.explain}>
          {pointer.label}
        </span>
      )}
      {code && syntax ? (
        <BlueprintCode preview={code} language={syntax} className="pin-value" />
      ) : !exec && value !== undefined ? (
        <span className="pin-value" title={value}>
          {value}
        </span>
      ) : null}
    </div>
  );
}

const EXPR_PIN: Pin = { id: "expr", kind: "Data", label: "details", ty: null };

function BlueprintNodeImpl({ id, data, selected }: NodeProps) {
  const { node, code, sourceRevision, expanded, language, flow, argPins, hiddenInputs, canExpand, explanations, sequence, inputAccess, detailsLabel, resultInfo } = data as unknown as BlueprintData;
  const explain = useUi((s) => s.blueprintExplanations);
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
  const select = useUi((s) => s.select);
  const inspect = (pin: Pin, syntax?: string) => select({ kind: "flow-node", flow, id: node.id, pin: pin.id, syntax });
  const detailsFor = (pin: Pin, incoming: boolean): LanguageDetail[] => {
    if (!explain) return [];
    const access = incoming ? inputAccess?.[pin.id] : undefined;
    if (access) return [{ label: ACCESS_LABEL[access], explain: ACCESS_TEXT[access] }];
    return syntaxBadges(statementSites(node, false, false, pin.id), (key) => flavorInfo(catalog, key, language),
      [...pinSyntaxDetails(node, pin.id), ...(pin.binding ? bindingDetails(pin.binding, language) : [])]);
  };
  // Debugger: breakpoints by the statement's first line; the node the program is paused at.
  const file = node.span.file;
  const syntax = codeLanguage(language, file);
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
  const args: Pin[] = (argPins ?? []).map((p, i) => ({ id: p.id, kind: "Data", label: p.name ?? `argument ${i + 1}`, ty: p.ty, value_info: p.valueInfo, type_info: p.typeInfo }));
  const argDetails = new Map((argPins ?? []).flatMap((p) => p.languageDetails ? [[p.id, {
    path: p.languageDetails.path,
    details: syntaxBadges(p.languageDetails.sites ?? [], (key) => flavorInfo(catalog, key, language),
      resolvePinDetails(p.languageDetails, (flavor) => flavorInfo(catalog, flavor, language))),
  }] as const] : []));
  const argPaths = new Map((argPins ?? []).map((p) => [p.id, p.path]));
  const argValues = new Map((argPins ?? []).flatMap((p) => p.value === null ? [] : [[p.id, { text: p.value, emphasis: p.emphasis }] as const]));
  const own = hiddenInputs?.length ? node.inputs.filter((p) => !hiddenInputs.includes(p.id)) : node.inputs;
  const inputs = argPins ? [...own, ...args] : expanded ? [...own, { ...EXPR_PIN, label: detailsLabel ?? EXPR_PIN.label, type_info: node.expr?.type_info }] : own;
  // A statement that performs a shared operation with an effect (print, write file) is titled by it.
  const op = node.expr && node.concept === "Call" ? operationInfo(catalog, node.expr.op) : undefined;
  const rows = Math.max(inputs.length, node.outputs.length);
  const unreachable = node.flavors.includes("unreachable");
  const details = explain ? syntaxBadges(statementSites(node, expanded, !!argPins), (key) => flavorInfo(catalog, key, language),
    statementDetailFlavors(node, language, expanded, !!argPins).flatMap((flavor) => flavorInfo(catalog, flavor, language) ?? [])) : [];
  const languageFlavors = explain ? statementDetailFlavors(node, language, true) : [];
  const visibleFlavors = node.flavors.filter((f) => f !== "unreachable" && !isExplanationFlavor(f)
    && !languageFlavors.includes(f)
    && !(explain && (f === "const" || f === "var" || f === "mut") && node.outputs.some((pin) => pin.binding)));
  // An assignment's focus is its destination. Other statements only use the root's
  // resolved target when the same source fragment is on the focus line.
  const focusedExpr =
    node.expr?.concept === "Assign" || node.expr?.concept === "UnaryOp" ? node.expr.children[0]
      : node.expr?.concept === "Pipeline" ? node.expr.children.at(-1)
      : node.expr;
  const focusTarget = [focusedExpr, node.concept === "Emit" ? node.expr?.children.find((child) => child.flavors.includes("target")) : undefined]
    .find((expr) => expr?.target && node.focus && expr.focus?.primary === node.focus.primary &&
      expr.focus.prefix === node.focus.prefix && expr.focus.prefix_bytes?.[0] === node.focus.prefix_bytes?.[0] &&
      expr.focus.prefix_bytes?.[1] === node.focus.prefix_bytes?.[1] &&
      expr.focus.primary_bytes?.[0] === node.focus.primary_bytes?.[0] &&
      expr.focus.primary_bytes?.[1] === node.focus.primary_bytes?.[1])?.target;

  return (
    <div
      className={`bp shape-${info.shape} ${selected ? "selected" : ""} ${pausedHere ? "paused-here" : ""} ${breakpoint ? "has-breakpoint" : ""} ${unreachable ? "unreachable" : ""} ${active && !replaying ? "active" : ""} ${isReplayNode ? "replay-current" : ""} ${count && !replaying ? "visited" : ""} ${replaying && !isReplayNode ? "dimmed" : ""} ${throws && !replaying ? "threw" : ""}`}
      style={{ "--accent": color } as React.CSSProperties}
    >
      {id !== node.id && (
        <>
          <Handle className="class-wire-handle" type="target" position={Position.Left} id={CLASS_IN} isConnectable={false} />
          <Handle className="class-wire-handle" type="source" position={Position.Right} id={CLASS_OUT} isConnectable={false} />
        </>
      )}
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
        {explain && node.concept === "Return" && node.expr && !expanded && <ValueBadge info={resultInfo ?? node.expr.value_info}
          onInspect={() => select({ kind: "expr", flow, nodeId: node.id, path: "0" })} />}
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
        <button className="bp-source nodrag" title="Show the complete statement in the source panel" onClick={(event) => {
          event.stopPropagation();
          select({ kind: "flow-node", flow, id: node.id }, node.span, sourceRevision ?? undefined);
        }}>Source</button>
        {node.expr && canExpand !== false && (
          <button
            className="bp-expand nodrag"
            title={expanded ? "Hide expression details" : "Show expression details"}
            onClick={(e) => {
              e.stopPropagation();
              toggle(flow, node.id, expanded);
            }}
          >
            {expanded ? "−" : "+"}
          </button>
        )}
      </div>
      {code && <BlueprintCode preview={code} language={syntax} target={focusTarget} className="blueprint-statement-code" />}
      {explanations?.map((label) => <div key={label} className="blueprint-explanation" title={label}>{label}</div>)}
      {sequence && (
        <div className="blueprint-sequence" aria-label="Collection sequence">
          <strong>Collection steps</strong>
          <ol>
            {sequence.stages.map((stage, i) => (
              <li key={i}><span title={stage.action}>{stage.action}</span><small title={stage.timing}>{stage.timing}</small></li>
            ))}
          </ol>
          {sequence.example && (
            <div className="blueprint-sequence-example" aria-label="Illustrative example">
              <strong>Example</strong>
              {sequence.example.map((step) => <small key={step} title={step}>{step}</small>)}
            </div>
          )}
        </div>
      )}
      {shownValue && (
        <div className={`bp-live ${shownValue.startsWith("⚠") ? "bp-live-error" : ""}`} title={shownValue}>
          {shownValue}
        </div>
      )}
      <div className="bp-pins">
        {Array.from({ length: rows }, (_, i) => (
          <div className="pin-row" key={i}>
            <div className="pin-cell">{inputs[i] && <PinView pin={inputs[i]} side="in" showKind={explain} code={argValues.get(inputs[i].id)} syntax={syntax}
              details={argDetails.get(inputs[i].id)?.details ?? detailsFor(inputs[i], true)} inspect={(syntax) => {
                const path = argPaths.get(inputs[i].id);
                if (path || inputs[i].id === "expr") select({ kind: "expr", flow, nodeId: node.id, path: path ?? "0", syntax });
                else inspect(inputs[i], syntax);
              }} />}</div>
            <div className="pin-cell right">
              {node.outputs[i] && (
                <PinView pin={node.outputs[i]} side="out" showKind={explain}
                  label={explain ? executionPinLabel(node, node.outputs[i].id, node.outputs[i].label) : undefined}
                  details={detailsFor(node.outputs[i], false)} inspect={(syntax) => inspect(node.outputs[i], syntax)}
                  value={replaying ? undefined : (pausedVars?.[node.outputs[i].label] ?? vars?.[node.outputs[i].label])} />
              )}
            </div>
          </div>
        ))}
      </div>
      {(visibleFlavors.length > 0 || details.length > 0) && (
        <div className="badges">
          <LanguageBadges details={details} onInspect={(syntax) => select({ kind: "flow-node", flow, id: node.id, syntax })} />
          {visibleFlavors.map((f) => (
            <span key={f} className={`badge ${f.startsWith("unity:") ? "badge-unity" : ""}`}>
              {flavorLabel(catalog, f, language)}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

export const BlueprintNode = memo(BlueprintNodeImpl);
