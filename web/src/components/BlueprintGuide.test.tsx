// @vitest-environment jsdom
import { act, useState } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { Expr } from "../gen/Expr";
import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { ReplayBar } from "../panels/RunPanel";
import { expansionKey, flowToGraph, recordedEdgeId, type ExprData } from "../graph/flowGraph";
import { rustChoicePresentation } from "../graph/blueprintExplanation";
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

it("keeps while-let mismatch separate from question-mark propagation", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const settings = useUi.getState();
  const check = {
    ...step("Check queue", "Loop.While", [output("body", "Loop body"), output("out", "Completed"), output("error", "If propagation is needed: return early")]),
    flavors: ["while-let", "try"],
  };
  const body = step("Consume next value", "Call");
  const after = step("Return normally", "Return");
  const example: FlowGraph = {
    ...flow, nodes: [check, body, after],
    edges: [
      { from: check.id, from_pin: "body", to: body.id, to_pin: "in", kind: "Exec", access: null },
      { from: check.id, from_pin: "out", to: after.id, to_pin: "in", kind: "Exec", access: null },
    ],
    reading: [
      { node: check.id, span, basis: "Syntax", content: { kind: "Paths", input: "Evaluate on each attempt", deferred: false,
        timing: "Before each loop iteration", branches: [
          { label: "Pattern matches", action: "Run the body", output: "Run this iteration", pin: "body" },
          { label: "Pattern does not match", action: "End the loop", output: "Leave the loop", pin: "out" },
        ] } },
      { node: check.id, path: "0.0", span: { ...span, start_byte: 2, end_byte: 3 }, basis: "Syntax", content: {
        kind: "Paths", input: "Read the Result", deferred: false, branches: [
          { label: "Continue", action: "Take the value", output: "Continue checking the loop condition" },
          { label: "Propagate", action: "Return from catalog", output: "Propagate early", pin: "error" },
        ],
      } },
    ],
  };
  const onStep = vi.fn();
  const render = (graph: FlowGraph) => act(async () => root.render(
    <BlueprintGuide flows={[graph]} activeFlow={graph.symbol} activeNode={check.id} onStep={onStep} />,
  ));
  const click = async (label: string) => {
    const button = [...host.querySelectorAll("button")].find((candidate) => candidate.textContent === label);
    expect(button, label).toBeDefined();
    await act(async () => button!.click());
  };
  try {
    useUi.setState({ blueprintExplanations: true });
    await render(example);
    await click("Continue");
    expect(host.querySelector('[role="status"]')?.textContent).toContain("Continue checking the loop condition");
    expect(onStep).not.toHaveBeenCalled();
    await click("Pattern does not match");
    expect(onStep).toHaveBeenCalledExactlyOnceWith(example.symbol, after);

    onStep.mockClear();
    await render({ ...example, edges: example.edges.slice(0, 1) });
    expect(host.textContent).toContain("Pattern does not match · Leave the loop · no mapped step");
    await click("Pattern does not match");
    expect(host.querySelector('[role="status"]')?.textContent).toContain("Leave the loop");
    expect(onStep).not.toHaveBeenCalled();
    await click("Pattern matches");
    expect(onStep).toHaveBeenCalledExactlyOnceWith(example.symbol, body);
  } finally {
    await act(async () => root.unmount());
    useUi.setState(settings);
  }
});

it("keeps question-mark success local before testing a let-else pattern", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const settings = useUi.getState();
  const check = {
    ...step("Read the key", "Destructure", [output("out", ""), output("else", "Otherwise"), output("error", "Return early on error")]),
    flavors: ["let-else", "try"],
  };
  const matched = step("Use the key", "Call");
  const absent = step("Return missing bits", "Return");
  const example: FlowGraph = {
    ...flow, nodes: [check, matched, absent],
    edges: [
      { from: check.id, from_pin: "out", to: matched.id, to_pin: "in", kind: "Exec", access: null },
      { from: check.id, from_pin: "else", to: absent.id, to_pin: "in", kind: "Exec", access: null },
    ],
    reading: [
      { node: check.id, span, basis: "Syntax", content: { kind: "Paths", input: "Check whether next_key()? matches Some(key)",
        deferred: false, branches: [
          { label: "Pattern matches", action: "Bind the key", output: "Continue below", pin: "out" },
          { label: "Pattern mismatch", action: "Run the else block", output: "Run else", pin: "else" },
        ] } },
      { node: check.id, path: "0.0", span: { ...span, start_byte: 2, end_byte: 3 }, basis: "TypeAnalysis", content: {
        kind: "Paths", input: "Read the Result", deferred: false, branches: [
          { label: "Success", action: "Unwrap Ok", output: "Continue evaluating the initializer before checking the let-else pattern" },
          { label: "Error", action: "Return from visit_map", output: "Propagate early", pin: "error" },
        ],
      } },
    ],
  };
  const onStep = vi.fn();
  const click = async (label: string) => {
    const button = [...host.querySelectorAll("button")].find((candidate) => candidate.textContent === label);
    expect(button, label).toBeDefined();
    await act(async () => button!.click());
  };
  try {
    useUi.setState({ blueprintExplanations: true });
    await act(async () => root.render(<BlueprintGuide flows={[example]} activeFlow={example.symbol} activeNode={check.id} onStep={onStep} />));
    await click("Success");
    expect(host.querySelector('[role="status"]')?.textContent).toContain("before checking the let-else pattern");
    await click("Error");
    expect(host.querySelector('[role="status"]')?.textContent).toContain("Propagate early");
    expect(onStep).not.toHaveBeenCalled();
    await click("Pattern mismatch");
    expect(onStep).toHaveBeenCalledExactlyOnceWith(example.symbol, absent);
    onStep.mockClear();
    await click("Pattern matches");
    expect(onStep).toHaveBeenCalledExactlyOnceWith(example.symbol, matched);
  } finally {
    await act(async () => root.unmount());
    useUi.setState(settings);
  }
});

it("explains a Rust break result and follows only a mapped exit", () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const env = globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean };
  const previousEnvironment = env.IS_REACT_ACT_ENVIRONMENT;
  env.IS_REACT_ACT_ENVIRONMENT = true;
  const leave = {
    ...step("Leave search", "Break", [output("error", "Return early if absent")]),
    flavors: ["label:'search", "break-value-target:block", "break-value-binding:target₂"],
  };
  const after = step("Use target", "Call");
  const example = { ...flow, nodes: [leave, after], edges: [] };
  const onStep = vi.fn();
  const render = (graph: FlowGraph) => act(() => root.render(
    <BlueprintGuide flows={[graph]} activeFlow={graph.symbol} activeNode={leave.id} onStep={onStep} />,
  ));
  try {
    render(example);
    expect(host.textContent).toContain("If the expression after break produces a value, execution leaves the block 'search.");
    expect(host.textContent).toContain("That value initializes target₂.");
    expect(host.textContent).toContain("Return early if absent");
    expect(host.textContent).toContain("End of the mapped path");
    expect(host.querySelectorAll("button")).toHaveLength(1);
    expect(onStep).not.toHaveBeenCalled();

    render({ ...example, language: "csharp" });
    expect(host.textContent).toContain("End of this path");
    expect(host.textContent).not.toContain("initializes target₂");

    render({ ...example, nodes: [{ ...leave, outputs: [...leave.outputs, output("out", "")] }, after], edges: [
      { from: leave.id, from_pin: "out", to: after.id, to_pin: "in", kind: "Exec", access: null },
    ] });
    const next = [...host.querySelectorAll("button")].find((button) => button.textContent === "Next")!;
    act(() => next.click());
    expect(onStep).toHaveBeenCalledExactlyOnceWith(example.symbol, after);
  } finally {
    act(() => root.unmount());
    env.IS_REACT_ACT_ENVIRONMENT = previousEnvironment;
  }
});

it("marks only an observed, unique execution edge and excludes the after-block shortcut", () => {
  const step = { index: 1, micros: 10, thread: 1, node: "Return now", previous_node: "Check input", values: [] };
  expect(recordedEdgeId(flow, step, "example::")).toBe("example::Check input:true->Return now:in");
  expect(recordedEdgeId(flow, { ...step, previous_node: undefined })).toBeUndefined();
  expect(recordedEdgeId(flow, { ...step, node: "After block" })).toBeUndefined();
  const alternate = { ...flow.edges[1], from_pin: "false" };
  expect(recordedEdgeId({ ...flow, edges: [...flow.edges, alternate] }, step)).toBeUndefined();
});

it("shows the code after a Rust break without advancing the execution path", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const settings = useUi.getState();
  const focusCode = vi.fn();
  const onStep = vi.fn();
  const afterSpan = { ...span, start_byte: 12, end_byte: 20 };
  const stop = { ...step("Break", "Break"), flavors: ["break-loop:outer"] };
  const after = { ...step("After outer", "Call"), span: afterSpan };
  const graph: FlowGraph = {
    ...flow, source_revision: "current",
    nodes: [stop, step("outer", "Loop.Infinite", [output("out", "Completed")]), after],
    edges: [exec("outer", "out", after.id)],
  };
  const render = (example: FlowGraph, revision = "current", replaying = false) => act(async () => root.render(
    <BlueprintGuide flows={[example]} activeFlow={example.symbol} activeNode={stop.id} onStep={onStep}
      replaying={replaying} sources={{ [span.file]: { bytes: new Uint8Array(30), revision } }} />,
  ));
  const action = () => [...host.querySelectorAll("button")].find((button) => button.textContent === "Show code after the loop");
  try {
    useUi.setState({ focusCode });
    await render(graph);
    expect(action()).toBeDefined();
    await act(async () => action()!.click());
    expect(focusCode).toHaveBeenCalledExactlyOnceWith(afterSpan, "current");
    expect(onStep).not.toHaveBeenCalled();
    expect(host.querySelector("button")?.disabled).toBe(true);

    await render(graph, "edited");
    expect(action()).toBeUndefined();
    await render(graph, "current", true);
    expect(action()).toBeUndefined();
    await render({ ...graph, nodes: [...graph.nodes.slice(0, 2), { ...after, flavors: ["implicit-drop"] }] });
    expect(action()).toBeUndefined();
    await render({ ...graph, edges: [...graph.edges, exec("outer", "out", "Other")] });
    expect(action()).toBeUndefined();
  } finally {
    await act(async () => root.unmount());
    useUi.setState(settings);
  }
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

it("labels the compared value, selected branch, and match arm on Rust expression pins", () => {
  const expression = (concept: Expr["concept"], label: string, children: Expr[] = [], flavors: string[] = []): Expr => ({
    concept, label, children, flavors, span, symbol: null, target: null, ty: null, op: null, params: [],
  });
  const yes = expression("Literal", "1");
  const no = expression("Literal", "0");
  const conditional = expression("Choose", "choose", [expression("Var", "ready"), yes, no]);
  const some = expression("Match", "Some(Foo { value: n }) ⇒", [conditional], ["arm"]);
  const none = expression("Match", "None ⇒", [no], ["arm"]);
  const match = expression("Match", "match", [expression("Var", "value"), some, none]);
  const binding = { ...step("set", "Var"), expr: match };
  const example = { ...flow, nodes: [binding], edges: [] };
  const open = { [expansionKey(example.symbol, binding.id)]: true };
  const graph = flowToGraph(example, open, true);
  const pins = (path: string) => (graph.nodes.find((node) => node.id === `set/e/${path}`)?.data as ExprData).pins.map((pin) => pin.name);
  expect(pins("0")).toEqual(["Compared value", "Arm · Some(Foo { value: n })", "Arm · None"]);
  expect(pins("0.1")).toEqual(["Arm expression"]);
  expect(pins("0.1.0")).toEqual(["Condition", "If true", "If false"]);
  expect(rustChoicePresentation(some)).toMatchObject({ title: "Match arm", output: "Arm value" });
  expect(rustChoicePresentation(conditional)?.explain).toContain("completes normally");
  expect(graph.edges.every((edge) => edge.kind !== "exec")).toBe(true);

  const hidden = flowToGraph(example, open, true, { explanations: false });
  const oldPins = (hidden.nodes.find((node) => node.id === "set/e/0.1.0")?.data as ExprData).pins;
  expect(oldPins.map((pin) => pin.name)).toEqual(["If", "Then", "Else"]);
});

it("shows the guard for a Rust match arm without choosing an execution edge", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const settings = useUi.getState();
  const onStep = vi.fn();
  const match: FlowNode = {
    ...step("Compare input", "Match", [output("case:0", "Some(value) if allowed(value)"), output("case:1", "_")]),
    flavors: ["match-guard:0"],
    syntax_sites: [{ key: "match-guard", pin: "case:0", span }],
  };
  const example = {
    ...flow, source_revision: "current", nodes: [match, step("Take Some", "Return"), step("Take fallback", "Return")],
    edges: [exec(match.id, "case:0", "Take Some"), exec(match.id, "case:1", "Take fallback")],
  };
  const button = (label: string) => [...host.querySelectorAll("button")].find((entry) => entry.textContent === label);
  try {
    await act(async () => {
      useUi.setState({ blueprintExplanations: true });
      root.render(<BlueprintGuide flows={[example]} activeFlow={example.symbol} activeNode={match.id} onStep={onStep} />);
    });
    expect(host.textContent).toContain("including any remaining alternatives in the same arm");
    expect(button("_")).toBeDefined();
    await act(async () => button("Show guard")!.click());
    expect(useUi.getState().selection).toMatchObject({ kind: "flow-node", id: match.id, pin: "case:0" });
    expect(useUi.getState().codeFocus).toEqual(span);
    expect(useUi.getState().codeFocusRevision).toBe("current");
    expect(onStep).not.toHaveBeenCalled();
    await act(async () => button("Some(value) if allowed(value) · pattern matches and guard passes")!.click());
    expect(onStep).toHaveBeenCalledExactlyOnceWith(example.symbol, expect.objectContaining({ id: "Take Some" }));
  } finally {
    await act(async () => root.unmount());
    useUi.setState(settings);
  }
});

it("previews the selected value of an embedded Rust match before taking the next statement", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const settings = useUi.getState();
  const onStep = vi.fn();
  const example: FlowGraph = {
    ...flow, source_revision: "current",
    nodes: [step("Set fraction", "Var", [output("out", "")]), step("Use fraction", "Return")],
    edges: [exec("Set fraction", "out", "Use fraction")],
    reading: [{ node: "Set fraction", path: "0", span, basis: "Syntax", content: {
      kind: "Paths", input: "Compare profile with the arm patterns in order", deferred: false,
      branches: [
        { label: "Arm 1 · Some(profile)", action: "If this arm is selected and completes normally, use the value of this expression as the match result.", output: "distance / profile.distance_m" },
        { label: "Arm 2 · None", action: "If this arm is selected and completes normally, use the value of this expression as the match result.", output: "js_max(0.0, time)" },
      ],
    } }],
  };
  const click = async (label: string) => {
    const button = [...host.querySelectorAll("button")].find((entry) => entry.textContent === label);
    expect(button, label).toBeDefined();
    await act(async () => button!.click());
  };
  try {
    await act(async () => {
      useUi.setState({ blueprintExplanations: true });
      root.render(<BlueprintGuide flows={[example]} activeFlow={example.symbol} activeNode="Set fraction" onStep={onStep} />);
    });
    expect(host.textContent).toContain("distance / profile.distance_m");
    expect(host.textContent).toContain("js_max(0.0, time)");
    await click("Arm 2 · None");
    expect(host.querySelector('[role="status"]')?.textContent).toBe("Arm 2 · None → js_max(0.0, time)");
    expect(onStep).not.toHaveBeenCalled();
    await click("Back");
    expect(host.querySelector('[role="status"]')).toBeNull();
    await click("Next");
    expect(onStep).toHaveBeenCalledExactlyOnceWith(example.symbol, expect.objectContaining({ id: "Use fraction" }));
  } finally {
    await act(async () => root.unmount());
    useUi.setState(settings);
  }
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
