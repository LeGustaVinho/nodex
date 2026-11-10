// @vitest-environment jsdom
import { act, useState } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { ReplayBar } from "../panels/RunPanel";
import { recordedEdgeId } from "../graph/flowGraph";
import { BlueprintGuide } from "./BlueprintGuide";

const span = { file: "sample.rs", start_byte: 0, end_byte: 1, start_line: 1, end_line: 1, start_col: 1, end_col: 2 };
const step = (id: string, concept: FlowNode["concept"], outputs: FlowNode["outputs"] = []): FlowNode => ({
  id, concept, label: id, code: "", span, flavors: [], purity: "Impure", inputs: [], outputs, expr: null, writes: [], calls: [],
});

it("labels missing replay values and advances only to the next recorded step", () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const env = globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean };
  const oldEnvironment = env.IS_REACT_ACT_ENVIRONMENT;
  env.IS_REACT_ACT_ENVIRONMENT = true;
  const previous = useTrace.getState().replay;
  try {
    act(() => {
      useTrace.getState().setReplay({ flow: flow.symbol, index: 0, page: { total: 2, offset: 0, steps: [
        { index: 0, micros: 1000, thread: 1, node: null, values: [] },
        { index: 1, micros: 2000, thread: 2, node: "Return now", values: ["→ 20"] },
      ] } });
      root.render(<><BlueprintGuide flows={[flow]} activeFlow={flow.symbol} replaying onStep={() => {}} /><ReplayBar flow={flow.symbol} /></>);
    });
    expect(host.textContent).toContain("Recorded execution");
    expect(host.textContent).toContain("No value recorded");
    expect(host.textContent).toContain("Source step not mapped");
    expect(host.textContent).not.toContain("View after block");
    act(() => host.querySelector<HTMLButtonElement>('button[title="Next step"]')!.click());
    expect(useTrace.getState().replay?.index).toBe(1);
    expect(host.textContent).toContain("thread 2");
    expect(host.textContent).toContain("→ 20");
    expect(host.textContent).toContain("Transition not confirmed");
  } finally {
    act(() => root.unmount());
    useTrace.getState().setReplay(previous);
    env.IS_REACT_ACT_ENVIRONMENT = oldEnvironment;
  }
});
const output = (id: string, label: string) => ({ id, label, kind: "Exec" as const, ty: null });
const flow: FlowGraph = {
  symbol: "example", name: "example", language: "rust", span,
  nodes: [
    step("Start", "Entry", [output("out", "")]),
    step("Check input", "Branch", [output("true", "True"), output("out", "Then")]),
    step("Return now", "Return"),
    step("After block", "Loop.While", [output("body", "Loop body")]),
    step("Loop action", "Call"),
  ],
  edges: [["Start", "out", "Check input"], ["Check input", "true", "Return now"], ["Check input", "out", "After block"], ["After block", "body", "Loop action"]]
    .map(([from, from_pin, to]) => ({ from, from_pin, to, to_pin: "in", kind: "Exec", access: null })),
};

it("marks only an observed, unique execution edge and excludes the after-block shortcut", () => {
  const step = { index: 1, micros: 10, thread: 1, node: "Return now", previous_node: "Check input", values: [] };
  expect(recordedEdgeId(flow, step, "example::")).toBe("example::Check input:true->Return now:in");
  expect(recordedEdgeId(flow, { ...step, previous_node: undefined })).toBeUndefined();
  expect(recordedEdgeId(flow, { ...step, node: "After block" })).toBeUndefined();
  const alternate = { ...flow.edges[1], from_pin: "false" };
  expect(recordedEdgeId({ ...flow, edges: [...flow.edges, alternate] }, step)).toBeUndefined();
});

it("guides a selected branch and keeps explanations independent of the guide preference", () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const env = globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean };
  const oldEnvironment = env.IS_REACT_ACT_ENVIRONMENT;
  env.IS_REACT_ACT_ENVIRONMENT = true;
  const settings = useUi.getState();
  const project = "guide-ui-test";
  const click = (name: string) => {
    const button = [...host.querySelectorAll("button")].find((el) => el.textContent === name);
    expect(button, name).toBeDefined();
    act(() => button!.click());
  };
  function Harness() {
    const [node, setNode] = useState<string>();
    return <BlueprintGuide flows={[flow]} activeFlow={flow.symbol} activeNode={node} onStep={(_, step) => setNode(step.id)} />;
  }
  try {
    settings.loadDetail(project);
    settings.setBlueprintExplanations(false);
    act(() => root.render(<Harness />));
    expect(host.textContent).toContain("Possible paths");
    click("Next");
    expect(host.textContent).toContain("Check input");
    expect(host.textContent).toContain("View after block");
    click("If the condition is true");
    expect(host.textContent).toContain("Return now");
    expect(host.textContent).toContain("End of this path");
    click("Back");
    expect(host.textContent).toContain("Check input");
    click("View after block");
    click("Loop body");
    expect(host.textContent).toContain("Loop action");
    click("View loop condition");
    expect(host.textContent).toContain("After block");
    act(() => host.querySelector("input")!.click());
    settings.loadDetail("another-guide-ui-test");
    settings.loadDetail(project);
    expect(useUi.getState().blueprintGuide).toBe(true);
    expect(useUi.getState().blueprintExplanations).toBe(false);
  } finally {
    act(() => root.unmount());
    useUi.setState(settings);
    window.localStorage.removeItem(`nodex.detail:${project}:explanations`);
    window.localStorage.removeItem(`nodex.detail:${project}:guide`);
    env.IS_REACT_ACT_ENVIRONMENT = oldEnvironment;
  }
});
