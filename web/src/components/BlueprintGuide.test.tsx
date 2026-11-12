// @vitest-environment jsdom
import { act, useState } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
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

it("explores a branch inside an expression without advancing to another execution node", () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const env = globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean };
  const previousEnvironment = env.IS_REACT_ACT_ENVIRONMENT;
  env.IS_REACT_ACT_ENVIRONMENT = true;
  const settings = useUi.getState();
  const example: FlowGraph = {
    ...flow, source_revision: "original", reading: [
      { node: "Check input", path: "0", span, basis: "Syntax", content: {
        kind: "Detail", label: "Create a function value",
        text: "The function body waits until this value is called, even though it is written on the same line as its definition.",
        definition: { ...span, file: "other.rs" }, borrow: false,
      } },
      { node: "Check input", path: "0.1", span, basis: "TypeAnalysis", content: {
        kind: "Paths", input: "Look up a value", deferred: true,
        branches: [{ label: "Found", action: "Run the callback", output: "Some(value)" }, { label: "Missing", action: "Skip the callback", output: "None" }],
      } },
      { node: "Check input", span, basis: "Syntax", content: {
        kind: "Pattern", input: "Read the value", route: "true", parts: [],
      } },
    ],
  };
  function Harness() {
    const [active, setActive] = useState("Check input");
    return <BlueprintGuide flows={[example]} activeFlow={example.symbol} activeNode={active} onStep={(_, next) => setActive(next.id)} />;
  }
  const button = (name: string) => [...host.querySelectorAll("button")].find((entry) => entry.textContent === name)!;
  try {
    act(() => {
      useUi.setState({ blueprintExplanations: true });
      root.render(<Harness />);
    });
    const detail = host.querySelector('section[aria-label="Create a function value"]');
    expect(detail?.querySelector("p")?.textContent).toContain("The function body waits until this value is called");
    expect(detail?.textContent).not.toContain("Show macro definition");
    act(() => detail!.querySelector("button")!.click());
    expect(useUi.getState().codeFocus).toEqual(span);
    expect(useUi.getState().codeFocusRevision).toBe("original");
    expect(host.querySelector('section[aria-label="When called"]')?.textContent).toContain("Skip the callback");
    expect(host.querySelector('section[aria-label="Unpack a pattern"]')?.textContent).toContain("When the pattern matches");
    act(() => button("Missing").click());
    expect(host.querySelector('[role="status"]')?.textContent).toBe("Missing → None");
    expect(host.textContent).toContain("Check input");
    act(() => button("Back").click());
    expect(host.querySelector('[role="status"]')).toBeNull();
    act(() => button("Missing").click());
    act(() => button("View after block").click());
    expect(host.textContent).toContain("After block");
    act(() => button("Back").click());
    expect(host.textContent).toContain("Check input");
    expect(host.querySelector('[role="status"]')).toBeNull();
    expect(button("Back").disabled).toBe(true);
  } finally {
    act(() => root.unmount());
    useUi.setState(settings);
    env.IS_REACT_ACT_ENVIRONMENT = previousEnvironment;
  }
});

const exec = (from: string, from_pin: string, to: string): FlowGraph["edges"][number] => ({
  from, from_pin, to, to_pin: "in", kind: "Exec", access: null,
});
const loops: FlowGraph = {
  ...flow,
  nodes: [
    { ...step("For each door", "Loop.ForEach", [output("body", "Loop body")]), flavors: ["label:'doors"] },
    step("While waiting", "Loop.While", [output("body", "Loop body"), output("out", "Completed")]),
    step("Check the door", "Branch", [output("true", "True"), output("false", "False"), output("out", "Then")]),
    step("Skip", "Continue"),
    { ...step("Skip to the next door", "Continue"), flavors: ["label:'doors"] },
    step("After the loop", "Call"),
  ],
  edges: [
    exec("For each door", "body", "While waiting"),
    exec("While waiting", "body", "Check the door"),
    exec("While waiting", "out", "After the loop"),
    exec("Check the door", "true", "Skip"),
    exec("Check the door", "false", "Skip to the next door"),
  ],
};
const exitEdges = loops.edges.filter((edge) => edge.from_pin !== "body");

it.each([
  { scenario: "finds the innermost loop", graph: loops, active: "Skip", action: "Check the loop condition again", target: "While waiting" },
  { scenario: "uses an explicit label to find an outer loop", graph: loops, active: "Skip to the next door", action: "Check for the next item", target: "For each door" },
  { scenario: "restarts a loop with no condition", active: "Skip", action: "Start the next iteration", target: "While waiting",
    graph: { ...loops, nodes: loops.nodes.map((node) => node.id === "While waiting" ? { ...node, concept: "Loop.Infinite" as const } : node) } },
  { scenario: "stops at ambiguous predecessors", active: "Skip",
    graph: { ...loops, edges: [...loops.edges, exec("For each door", "body", "Skip")] } },
  { scenario: "stops at a cycle", active: "Skip",
    graph: { ...loops, edges: [...exitEdges, exec("Check the door", "out", "Check the door")] } },
  { scenario: "stops at an unknown predecessor", active: "Skip",
    graph: { ...loops, edges: [...exitEdges, exec("Missing step", "body", "Check the door")] } },
  { scenario: "does not reenter a loop through its exit", active: "Skip",
    graph: { ...loops, edges: [...exitEdges, exec("While waiting", "out", "Check the door")] } },
  { scenario: "withholds the jump when the source has changed", active: "Skip",
    graph: { ...loops, source_revision: "original" }, sources: { [span.file]: { bytes: new Uint8Array(), revision: "edited" } } },
])("explains a directly selected Rust continue and $scenario", async ({ graph, active, action, target, sources }) => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const onStep = vi.fn();
  try {
    await act(async () => root.render(<BlueprintGuide flows={[graph]} activeFlow={graph.symbol} activeNode={active} onStep={onStep} sources={sources} />));
    expect(host.textContent).toContain("Skip the rest of this iteration");
    expect(host.textContent).not.toContain("End of this path");
    const buttons = [...host.querySelectorAll("button")];
    expect(buttons.map((button) => button.textContent)).toEqual(action ? ["Back", action] : ["Back"]);
    if (action) {
      await act(async () => buttons[1].click());
      expect(onStep).toHaveBeenCalledExactlyOnceWith(graph.symbol, expect.objectContaining({ id: target }));
    }
  } finally {
    await act(async () => root.unmount());
  }
});

it("keeps Back across a continue but leaves the previous iteration's blocks behind when the loop exits", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  function Harness() {
    const [active, setActive] = useState("While waiting");
    return <BlueprintGuide flows={[loops]} activeFlow={loops.symbol} activeNode={active} onStep={(_, next) => setActive(next.id)} />;
  }
  const click = async (label: string) => {
    const button = [...host.querySelectorAll("button")].find((entry) => entry.textContent === label);
    expect(button, label).toBeDefined();
    await act(async () => button!.click());
  };
  try {
    await act(async () => root.render(<Harness />));
    await click("Loop body");
    await click("If the condition is true");
    await click("Check the loop condition again");
    await click("Completed");
    expect(host.textContent).toContain("After the loop");
    expect(host.textContent).toContain("End of the mapped path");
    expect(host.textContent).not.toContain("View enclosing block");
    await click("Back");
    expect(host.textContent).toContain("While waiting");
    await click("Back");
    expect(host.textContent).toContain("Skip the rest of this iteration");
  } finally {
    await act(async () => root.unmount());
  }
});
