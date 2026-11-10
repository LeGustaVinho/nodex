import { useState } from "react";
import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import { executionPinLabel, statementExplanations } from "../graph/blueprintExplanation";
import { useUi } from "../store";

function guideChoice(label: string) {
  if (label === "Then") return "View after block";
  if (label === "True") return "If the condition is true";
  if (label === "False") return "If the condition is false";
  return label || "Next";
}

interface Visit {
  flow: string;
  node: string;
  via?: string;
}

interface Props {
  flows: readonly FlowGraph[];
  activeFlow: string | null;
  activeNode?: string;
  replaying?: boolean;
  onStep: (flow: string, node: FlowNode) => void;
}

export function BlueprintGuide({ flows, activeFlow, activeNode, replaying, onStep }: Props) {
  const [history, setHistory] = useState<Visit[]>([]);
  const guideOnOpen = useUi((s) => s.blueprintGuideOnOpen);
  const setGuideOnOpen = useUi((s) => s.setBlueprintGuideOnOpen);
  const flow = flows.find((f) => f.symbol === activeFlow);
  const entry = (f: FlowGraph) => f.nodes.find((n) => n.concept === "Entry" || n.concept === "EventEntry") ?? f.nodes[0];
  const node = flow && (flow.nodes.find((n) => n.id === activeNode) ?? entry(flow));
  const current = history.at(-1);
  const visits = current?.flow === flow?.symbol && current?.node === node?.id ? history : [];
  const choices = flow?.edges.filter((edge) => edge.kind === "Exec" && edge.from === node?.id) ?? [];
  const unmapped = node?.outputs.filter((pin) =>
    pin.kind === "Exec" && pin.id !== "out" && pin.id !== "error" && pin.label && !choices.some((edge) => edge.from_pin === pin.id),
  ) ?? [];
  const terminal = node && ["Return", "Throw", "Break", "Continue", "Jump"].includes(node.concept);
  const enclosing = choices.length === 0 && !terminal && flow
    ? visits.slice(0, -1).flatMap((visit, i) => {
      const via = visits[i + 1].via;
      return visit.flow === flow.symbol && via && via !== "out"
        ? [flow.nodes.find((n) => n.id === visit.node)] : [];
    }).reverse()
      .find((n) => n && (n.concept.startsWith("Loop.") || n.outputs.some((p) => p.id === "out" && p.label === "Then")))
    : undefined;

  const visit = (nextFlow: FlowGraph, target: FlowNode, via?: string) => {
    const trail = visits.length ? visits : flow && node ? [{ flow: flow.symbol, node: node.id }] : [];
    setHistory([...trail, { flow: nextFlow.symbol, node: target.id, via }]);
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
          {statementExplanations(node, flow.language).map((label) => <div key={label}>{label}</div>)}
          <div className="blueprint-guide-row">
            <button disabled={visits.length < 2} onClick={back}>Back</button>
            {choices.map((edge) => {
              const target = flow.nodes.find((n) => n.id === edge.to);
              const label = executionPinLabel(node, edge.from_pin, node.outputs.find((p) => p.id === edge.from_pin)?.label ?? "");
              return target && (
                <button key={`${edge.from_pin}:${edge.to}`} onClick={() => visit(flow, target, edge.from_pin)}>
                  {guideChoice(label)}
                </button>
              );
            })}
            {enclosing && (
              <button onClick={() => visit(flow, enclosing)}>
                {enclosing.concept.startsWith("Loop.") ? "View loop condition" : "View enclosing block"}
              </button>
            )}
            {node.outputs.some((p) => p.id === "error" && !choices.some((e) => e.from_pin === p.id)) && (
              <span>{node.outputs.find((p) => p.id === "error")?.label}</span>
            )}
            {unmapped.map((pin) => (
              <span key={pin.id}>{executionPinLabel(node, pin.id, pin.label)} · no mapped step</span>
            ))}
            {terminal && <span>End of this path</span>}
            {!terminal && !choices.length && !enclosing && <span>End of the mapped path</span>}
          </div>
        </>
      )}
      {!replaying && !node && <div>{flows.length ? "Choose an executable member on the graph." : "Choose a declaration card to inspect its contract."}</div>}
    </section>
  );
}
