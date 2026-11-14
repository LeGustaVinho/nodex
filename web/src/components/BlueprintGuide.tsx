import { Fragment, useState } from "react";
import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import { executionPinLabel, pinSyntaxDetails, statementExplanations } from "../graph/blueprintExplanation";
import { syntaxId } from "../graph/syntaxSites";
import { useUi } from "../store";
import { BlueprintReading } from "./BlueprintReading";
import { executionReading, routeName } from "../graph/blueprintReading";
import { rustOwnershipRows } from "../graph/rustOwnership";
import type { BlueprintSources } from "../graph/blueprintSource";

function guideChoice(label: string) {
  if (label === "Then") return "View after block";
  if (label === "True") return "If the condition is true";
  if (label === "False") return "If the condition is false";
  return label || "Next";
}

function nextIterationAction(node: FlowNode) {
  switch (node.concept) {
    case "Loop.ForEach":
    case "Loop.Range": return "Check for the next item";
    case "Loop.While": return "Check the loop condition again";
    case "Loop.Infinite": return "Start the next iteration";
    default: return undefined;
  }
}

function continueTarget(flow: FlowGraph, node: FlowNode) {
  const label = node.flavors.find((flavor) => flavor.startsWith("label:"));
  const predecessors = new Map<string, FlowGraph["edges"][number] | null>();
  for (const edge of flow.edges) {
    if (edge.kind !== "Exec") continue;
    predecessors.set(edge.to, predecessors.has(edge.to) ? null : edge);
  }
  const nodes = new Map(flow.nodes.map((entry) => [entry.id, entry]));
  const seen = new Set([node.id]);
  let edge = predecessors.get(node.id);
  while (edge && !seen.has(edge.from)) {
    const { from, from_pin: port } = edge;
    const previous = nodes.get(from);
    if (!previous?.outputs.some((pin) => pin.kind === "Exec" && pin.id === port)) return;
    if (port === "body" && (!label || previous.flavors.includes(label))) {
      const action = nextIterationAction(previous);
      if (action) return { node: previous, action };
    }
    seen.add(previous.id);
    edge = predecessors.get(previous.id);
  }
}

function rustBreakResult(node: FlowNode) {
  if (node.concept !== "Break") return;
  const target = node.flavors.includes("break-value-target:block") ? "block"
    : node.flavors.includes("break-value-target:loop") ? "loop" : undefined;
  if (!target) return;
  const label = node.flavors.find((flavor) => flavor.startsWith("label:"))?.slice("label:".length);
  const binding = node.flavors.find((flavor) => flavor.startsWith("break-value-binding:"))?.slice("break-value-binding:".length);
  return { target, label, binding, returns: node.flavors.includes("break-value-return") };
}

function sourceAfterBreak(flow: FlowGraph, node: FlowNode) {
  if (node.concept !== "Break") return;
  const id = node.flavors.find((flavor) => flavor.startsWith("break-loop:"))?.slice("break-loop:".length);
  const loop = id && flow.nodes.find((candidate) => candidate.id === id && candidate.concept.startsWith("Loop."));
  if (!loop || !loop.outputs.some((pin) => pin.id === "out" && pin.kind === "Exec")) return;
  const [successor, ...others] = flow.edges.filter((edge) => edge.kind === "Exec" && edge.from === loop.id && edge.from_pin === "out");
  if (!successor || others.length) return;
  const next = flow.nodes.find((candidate) => candidate.id === successor.to);
  if (!next || next.flavors.includes("implicit-drop") || next.span.file !== flow.span.file) return;
  return next.span.start_byte >= node.span.end_byte && next.span.end_byte > next.span.start_byte ? next.span : undefined;
}

interface Visit {
  flow: string;
  node: string;
  via?: string;
  resumesLoop?: true;
}

function enclosingBlock(flow: FlowGraph, visits: readonly Visit[]) {
  for (let i = visits.length - 2; i >= 0; i--) {
    const { via, resumesLoop } = visits[i + 1];
    if (resumesLoop) return;
    const visit = visits[i];
    if (visit.flow !== flow.symbol || !via || via === "out") continue;
    const node = flow.nodes.find((entry) => entry.id === visit.node);
    if (node && (node.concept.startsWith("Loop.") || node.outputs.some((pin) => pin.id === "out" && pin.label === "Then"))) return node;
  }
}

interface Props {
  flows: readonly FlowGraph[];
  activeFlow: string | null;
  activeNode?: string;
  replaying?: boolean;
  onStep: (flow: string, node: FlowNode) => void;
  sources?: BlueprintSources;
}

export function BlueprintGuide({ flows, activeFlow, activeNode, replaying, onStep, sources }: Props) {
  const [history, setHistory] = useState<Visit[]>([]);
  const [localChoice, setLocalChoice] = useState<{ flow: string; node: string; revision?: string; text: string }>();
  const guideOnOpen = useUi((s) => s.blueprintGuideOnOpen);
  const explanations = useUi((s) => s.blueprintExplanations);
  const select = useUi((s) => s.select);
  const focusCode = useUi((s) => s.focusCode);
  const setGuideOnOpen = useUi((s) => s.setBlueprintGuideOnOpen);
  const flow = flows.find((f) => f.symbol === activeFlow);
  const entry = (f: FlowGraph) => f.nodes.find((n) => n.concept === "Entry" || n.concept === "EventEntry") ?? f.nodes[0];
  const node = flow && (flow.nodes.find((n) => n.id === activeNode) ?? entry(flow));
  const revision = flow?.source_revision ?? undefined;
  const source = flow && sources?.[flow.span.file];
  const currentSource = !source || source.revision === revision;
  const showReading = explanations && currentSource;
  const reading = showReading && flow && node ? flow.reading?.filter((site) => site.node === node.id) ?? [] : [];
  const routed = node && executionReading(node, reading);
  const ownership = flow && node ? rustOwnershipRows(flow, showReading).get(node.id) : undefined;
  const choice = !replaying && showReading && localChoice?.flow === flow?.symbol && localChoice?.node === node?.id
    && localChoice?.revision === revision ? localChoice : undefined;
  if (localChoice && !choice) setLocalChoice(undefined);
  const current = history.at(-1);
  const visits = current?.flow === flow?.symbol && current?.node === node?.id ? history : [];
  const choices = flow?.edges.filter((edge) => edge.kind === "Exec" && edge.from === node?.id) ?? [];
  const guards = showReading && flow?.language === "rust" && node?.concept === "Match"
    ? node.outputs.flatMap((pin) => pinSyntaxDetails(node, pin.id)
      .filter((detail) => detail.label === "Match guard").map((detail) => ({ pin: pin.id, detail })))
    : [];
  const unmapped = node?.outputs.filter((pin) =>
    pin.kind === "Exec" && (pin.id !== "out" || (node.flavors.includes("while-let") && routed?.labels.out))
      && pin.id !== "error" && pin.label && !choices.some((edge) => edge.from_pin === pin.id),
  ) ?? [];
  const terminal = node && ["Return", "Throw", "Break", "Continue", "Jump"].includes(node.concept);
  const continues = flow?.language === "rust" && node?.concept === "Continue";
  const breakResult = currentSource && flow?.language === "rust" && node ? rustBreakResult(node) : undefined;
  const afterBreak = !replaying && currentSource && revision && flow?.language === "rust" && node
    ? sourceAfterBreak(flow, node) : undefined;
  const nextIteration = !replaying && currentSource && continues && flow && node ? continueTarget(flow, node) : undefined;
  const enclosing = !choices.length && !terminal && flow ? enclosingBlock(flow, visits) : undefined;

  const visit = (nextFlow: FlowGraph, target: FlowNode, transition?: Pick<Visit, "via" | "resumesLoop">) => {
    const trail = visits.length ? visits : flow && node ? [{ flow: flow.symbol, node: node.id }] : [];
    setLocalChoice(undefined);
    setHistory([...trail, { flow: nextFlow.symbol, node: target.id, ...transition }]);
    onStep(nextFlow.symbol, target);
  };
  const back = () => {
    const previous = visits.at(-2);
    const target = previous && flows.find((f) => f.symbol === previous.flow)?.nodes.find((n) => n.id === previous.node);
    if (!previous || !target) return;
    setHistory(visits.slice(0, -1));
    onStep(previous.flow, target);
  };

  return (
    <section className="blueprint-guide" aria-label="Blueprint guide">
      <div className="blueprint-guide-row">
        <strong>{replaying ? "Recorded execution" : "Possible paths"}</strong>
        <label className="toggle">
          <input type="checkbox" checked={guideOnOpen} onChange={(event) => setGuideOnOpen(event.target.checked)} /> Open guide by default
        </label>
        {!replaying && (flows.length > 1 || (!flow && flows.length > 0)) && (
          <select aria-label="Choose a function to follow" value={flow?.symbol ?? ""} onChange={(event) => {
            const targetFlow = flows.find((f) => f.symbol === event.target.value);
            const target = targetFlow && entry(targetFlow);
            if (targetFlow && target) visit(targetFlow, target);
          }}>
            <option value="" disabled>Choose a function</option>
            {flows.map((f) => <option key={f.symbol} value={f.symbol}>{f.name}</option>)}
          </select>
        )}
      </div>
      {!replaying && flow && node && (
        <>
          <div aria-live="polite">{node.label || node.concept}</div>
          {breakResult && (
            <div>
              If the expression after <code>break</code> produces a value, execution leaves the {breakResult.target}
              {breakResult.label && <> <code>{breakResult.label}</code></>}.
              {breakResult.binding ? <> That value initializes <code>{breakResult.binding}</code>.</>
                : breakResult.returns ? " That value becomes the result of this body."
                  : ` That value becomes the result of the ${breakResult.target}.`}
            </div>
          )}
          {statementExplanations(node, flow.language, reading.length > 0).map((label) => <div key={label}>{label}</div>)}
          {guards[0] && <div>{guards[0].detail.explain}</div>}
          {ownership?.map((row) => <div key={row.pin}><code>{row.name}</code> · {row.role}{row.origin && ` · ${row.origin}`}</div>)}
          <BlueprintReading full sites={reading} route={(pin) => routeName(node, pin)} choose={(site, label, output, pin) => {
            const edge = routed?.site === site && pin ? choices.find((edge) => edge.from_pin === pin) : undefined;
            const next = edge && flow.nodes.find((node) => node.id === edge.to);
            if (next && edge) {
              visit(flow, next, { via: edge.from_pin });
            } else {
              setLocalChoice({ flow: flow.symbol, node: node.id, revision, text: `${label} → ${output}` });
            }
          }} inspect={(site, span) => {
            const expected = span.file === flow.span.file ? revision : undefined;
            if (!site.path) {
              select({ kind: "flow-node", flow: flow.symbol, id: node.id }, expected ? span : undefined, expected);
              return;
            }
            useUi.getState().toggleExpanded(flow.symbol, node.id, false);
            select({ kind: "expr", flow: flow.symbol, nodeId: node.id, path: site.path }, expected ? span : undefined, expected);
          }} />
          {choice && <div role="status">{choice.text}</div>}
          <div className="blueprint-guide-row">
            <button disabled={visits.length < 2 && !choice} onClick={() => choice ? setLocalChoice(undefined) : back()}>Back</button>
            {choices.map((edge) => {
              const target = flow.nodes.find((n) => n.id === edge.to);
              const label = routed?.labels[edge.from_pin] ?? executionPinLabel(node, edge.from_pin, node.outputs.find((p) => p.id === edge.from_pin)?.label ?? "");
              const guard = guards.find(({ pin }) => pin === edge.from_pin);
              const site = guard && node.syntax_sites?.find(({ key, pin }) => key === "match-guard" && pin === guard.pin);
              return target && (
                <Fragment key={`${edge.from_pin}:${edge.to}`}>
                  <button onClick={() => visit(flow, target, { via: edge.from_pin })}>
                    {guard ? `${label} · pattern matches and guard passes` : guideChoice(label)}
                  </button>
                  {site && revision && site.span.file === flow.span.file && (
                    <button type="button" aria-label={`Show guard for ${label}`} onClick={() => select({
                      kind: "flow-node", flow: flow.symbol, id: node.id, pin: edge.from_pin, syntax: syntaxId(site),
                    }, site.span, revision)}>Show guard</button>
                  )}
                </Fragment>
              );
            })}
            {enclosing && (
              <button onClick={() => visit(flow, enclosing)}>
                {enclosing.concept.startsWith("Loop.")
                  ? flow.language === "rust" && enclosing.flavors.includes("rust-loop") ? "View loop" : "View loop condition"
                  : "View enclosing block"}
              </button>
            )}
            {node.outputs.some((p) => p.id === "error" && !choices.some((e) => e.from_pin === p.id)) && (
              <span>{node.outputs.find((p) => p.id === "error")?.label}</span>
            )}
            {unmapped.map((pin) => (
              <span key={pin.id}>{routed?.labels[pin.id] ?? executionPinLabel(node, pin.id, pin.label)} · no mapped step</span>
            ))}
            {continues ? <span>Skip the rest of this iteration</span> : terminal && !breakResult && !afterBreak && <span>End of this path</span>}
            {afterBreak && <button type="button" onClick={() => focusCode(afterBreak, revision)}>Show code after the loop</button>}
            {nextIteration && <button type="button" onClick={() => visit(flow, nextIteration.node, { resumesLoop: true })}>{nextIteration.action}</button>}
            {(!terminal || breakResult || afterBreak) && !choices.length && !enclosing && <span>End of the mapped path</span>}
          </div>
        </>
      )}
      {!replaying && !node && <div>{flows.length ? "Choose an executable member on the graph." : "Choose a declaration card to inspect its contract."}</div>}
    </section>
  );
}
