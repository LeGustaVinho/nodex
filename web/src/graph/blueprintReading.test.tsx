// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import type { BorrowOrigins } from "../gen/BorrowOrigins";
import type { Expr } from "../gen/Expr";
import type { FlowGraph } from "../gen/FlowGraph";
import type { Pin } from "../gen/Pin";
import type { ReadingSite } from "../gen/ReadingSite";
import type { Span } from "../gen/Span";
import type { TypeInfo } from "../gen/TypeInfo";
import { BlueprintReading } from "../components/BlueprintReading";
import { Inspector } from "../panels/Inspector";
import { ReadingInspector, readingOrigin } from "../panels/ReadingInspector";
import { useUi } from "../store";
import { edgeStyle } from "../views/GraphCanvas";
import { executionPinLabel } from "./blueprintExplanation";
import { executionReading, routeName } from "./blueprintReading";
import { prefixGraph } from "./classBlueprint";
import { expansionKey, flowToGraph, type BlueprintData, type ExprData } from "./flowGraph";
import { rustOwnershipRows } from "./rustOwnership";

const span = (start: number, end: number): Span => ({
  file: "step.rs", start_byte: start, end_byte: end, start_line: 1, end_line: 1, start_col: start + 1, end_col: end + 1,
});
const type: TypeInfo = { origin: "Compiler", shape: { kind: "Scalar", name: "UIntSize" }, native: "usize" };
const node = "statement";
const expression = (concept: Expr["concept"], label: string, at: Span, children: Expr[] = [], fields: Partial<Expr> = {}): Expr => ({
  concept, label, span: at, children, flavors: [], params: [], symbol: null, target: null, op: null, ty: null, ...fields,
});
const get = expression("Call", "get", span(24, 54), [
  expression("GetMember", "crew_index", span(24, 43), [], { flavors: ["target"] }),
  expression("Var", "crew", span(49, 53)),
], { flavors: ["target"] });
const pattern = expression("Lambda", "|&(host, _)| …", span(59, 77), [
  expression("Var", "host", span(73, 77), [], { type_info: type }),
], { params: ["&(host, _)"] });
const fn = expression("Lambda", "|crew| …", span(11, 78), [
  expression("Call", "map", span(24, 78), [get, pattern], { flavors: ["option:map"] }),
], { params: ["crew"] });
const flow: FlowGraph = {
  symbol: "prepare_crew", name: "prepare_crew", language: "rust", span: span(0, 78), source_revision: "original",
  nodes: [{
    id: node, concept: "Var", label: "host", code: "let host = |crew: &str| encounter.crew_index.get(crew).map(|&(host, _)| host);",
    span: span(0, 78), flavors: [], purity: "Impure", inputs: [], outputs: [], expr: fn, writes: ["host"], calls: [],
  }],
  edges: [],
  reading: [
    { node, path: "0", span: span(11, 23), basis: "Syntax", content: {
      kind: "Detail", label: "Create function host(crew)", text: "The body runs when called.", borrow: false,
    } },
    { node, path: "0.0", span: span(24, 78), basis: "TypeAnalysis", content: {
      kind: "Paths", input: "Look up crew in encounter.crew_index", deferred: true,
      result: { origin: "Compiler", shape: { kind: "Named", name: "Option", arguments: [type.shape] } },
      branches: [
        { label: "Found", action: "Copy the first tuple item as host", output: "Some(host)" },
        { label: "Missing", action: "Skip the callback", output: "None" },
      ],
    } },
    { node, path: "0.0.0", span: get.span, basis: "TypeAnalysis", content: {
      kind: "Detail", label: "Borrowed entry", text: "Borrow the entry for reading.", borrow: true,
    } },
    { node, path: "0.0.1", span: span(60, 71), basis: "TypeAnalysis", content: {
      kind: "Pattern", input: "Read the referenced tuple", source: get.span,
      parts: [
        { projection: ".0", span: span(62, 66), binding: "host", access: "Copy", type_info: type },
        { projection: ".1", span: span(68, 69) },
      ],
    } },
  ],
};

it("shows both paths in a compact card and opens the callback at the Statements level", () => {
  const closed = flowToGraph(flow, {}, true);
  const compact = closed.nodes[0].data as BlueprintData;
  expect(compact.reading?.map((entry) => entry.content.kind)).toEqual(["Detail", "Paths"]);
  expect(closed.nodes).toHaveLength(1);
  const opened = flowToGraph(flow, { [expansionKey(flow.symbol, node)]: true }, true);
  const callback = opened.nodes.find((entry) => entry.id === `${node}/e/0.0.1`)!.data as ExprData;
  expect(callback.params).toEqual(["host"]);
  expect(callback.reading?.some((entry) => entry.content.kind === "Pattern")).toBe(true);
  expect(opened.edges).toContainEqual(expect.objectContaining({
    source: `${node}/e/0`, sourceHandle: "p:crew", target: `${node}/e/0.0.0`,
  }));
  expect(opened.edges.filter((edge) => edge.kind === "exec")).toEqual(closed.edges);
  const prefixed = prefixGraph(opened, "class/", "class");
  const inner = prefixed.nodes.find((entry) => entry.id === `class/${node}/e/0.0.1`)!.data as ExprData;
  expect(inner.ownerId).toBe(node);
  expect(inner.reading).toEqual(callback.reading);
});

it("omits reading entries from old, stale, or explanation-disabled graphs", () => {
  for (const graph of [
    flowToGraph({ ...flow, reading: undefined }, {}, true),
    flowToGraph(flow, {}, true, { explanations: false }),
    flowToGraph(flow, {}, true, { source: { revision: "edited", bytes: new Uint8Array() } }),
  ]) {
    expect((graph.nodes[0].data as BlueprintData).reading).toEqual([]);
  }
});

it("shows ownership, named origins and caller boundaries with or without statement expansion", () => {
  const rust = structuredClone(flow);
  rust.nodes[0].outputs = [
    { id: "var:record", kind: "Data", label: "record", ty: null },
    { id: "var:view", kind: "Data", label: "view", ty: "&i32", value_info: { kind: "Reference", basis: "Syntax", qualifier: "SharedBorrow" } },
    { id: "var:writer", kind: "Data", label: "writer", ty: "&mut i32", value_info: { kind: "Reference", basis: "Syntax", qualifier: "ExclusiveBorrow" } },
    { id: "var:address", kind: "Data", label: "address", ty: "*const i32", value_info: { kind: "Pointer", basis: "Syntax", qualifier: "Raw" } },
  ];
  rust.borrows = [
    { node, pin: "var:view", sources: [{ kind: "Local", owner: "record", place: "record.count", binding: { node, pin: "var:record" } }] },
    { node, pin: "var:writer", sources: [{ kind: "Caller", owner: null, place: "state.count" }] },
  ];
  for (const expanded of [{}, { [expansionKey(flow.symbol, node)]: true }]) {
    const graph = flowToGraph(rust, expanded, true);
    const data = graph.nodes.find((entry) => entry.id === node)!.data as BlueprintData;
    expect(data.ownership).toEqual([
      { pin: "var:record", name: "record", role: "Owner", origin: "Referenced by: view" },
      { pin: "var:view", name: "view", role: "Shared borrow", origin: "Owner: record · Borrowed: record.count" },
      { pin: "var:writer", name: "writer", role: "Mutable borrow", origin: "From caller · state.count" },
      { pin: "var:address", name: "address", role: "Raw pointer" },
    ]);
    const prefixed = prefixGraph(graph, "class/", "class");
    expect((prefixed.nodes.find((entry) => entry.id === "class/" + node)!.data as BlueprintData).ownership).toEqual(data.ownership);
  }
  const stale = flowToGraph(rust, {}, true, { source: { revision: "edited", bytes: new Uint8Array() } });
  expect((stale.nodes[0].data as BlueprintData).ownership).toEqual([]);
});

function rustAccessFlow(): FlowGraph {
  const reference: Pin = {
    id: "use:encounter", kind: "Data", label: "encounter", ty: "&mut Encounter",
    value_info: { kind: "Reference", basis: "Syntax", qualifier: "ExclusiveBorrow" },
  };
  const source: FlowGraph["nodes"][number] = { ...flow.nodes[0], id: "entry", concept: "Entry", expr: null,
    outputs: [{ ...reference, id: "var:encounter" }] };
  const read = expression("Var", "encounter", span(12, 21));
  const capture = expression("Lambda", "|at| …", span(11, 78), [read]);
  const callback = expression("Call", "map", span(8, 78), [capture]);
  const future = expression("Compose", "async block", span(11, 78), [read], { flavors: ["block", "async"] });
  const nestedFuture = expression("Call", "spawn", span(8, 78), [future]);
  const index: Pin = { id: "use:index", kind: "Data", label: "index", ty: "usize" };
  const nodes = [
    { ...flow.nodes[0], id: "read", expr: read, inputs: [reference] },
    { ...flow.nodes[0], id: "write", expr: read, inputs: [reference] },
    { ...flow.nodes[0], id: "closure", expr: capture, inputs: [reference] },
    { ...flow.nodes[0], id: "callback", expr: callback, inputs: [reference] },
    { ...flow.nodes[0], id: "future", expr: future, inputs: [reference] },
    { ...flow.nodes[0], id: "nested-future", expr: nestedFuture, inputs: [reference, index] },
    { ...flow.nodes[0], id: "ambiguous", expr: read, inputs: [reference] },
  ];
  const edge = (to: string, access: FlowGraph["edges"][number]["access"]): FlowGraph["edges"][number] => ({
    from: source.id, from_pin: "var:encounter", to, to_pin: reference.id, kind: "Data", access,
  });
  source.outputs.push({ ...index, id: "var:index" });
  return {
    ...flow, nodes: [source, ...nodes], reading: [],
    edges: [edge("read", "Borrow"), edge("write", "BorrowMut"), edge("closure", "Borrow"), edge("callback", "Borrow"),
      edge("future", "BorrowMut"), edge("nested-future", "Borrow"),
      { ...edge("nested-future", "Copy"), from_pin: "var:index", to_pin: index.id },
      edge("ambiguous", "Borrow"), edge("ambiguous", "BorrowMut")],
    borrows: nodes.map(({ id }) => ({
      node: id, pin: reference.id, sources: [{ kind: "Caller", owner: null, place: "encounter" }],
    })),
  };
}

it("keeps Rust borrow access at a use separate from the reference type and capture", () => {
  const rust = rustAccessFlow();
  const ownership = rustOwnershipRows(rust, true);
  const expectInput = (id: string, role: string) => expect(ownership.get(id)).toEqual([
    { pin: "use:encounter", name: "encounter", role, origin: "From caller · encounter" },
  ]);
  expectInput("read", "Shared borrow here");
  expectInput("write", "Mutable borrow here");
  expectInput("closure", "Captured binding · mutable reference");
  expectInput("callback", "Type · mutable reference");
  expectInput("future", "Captured binding · mutable reference");
  expectInput("nested-future", "Type · mutable reference");
  expectInput("ambiguous", "Type · mutable reference");
  expect(rustOwnershipRows({ ...rust, edges: [...rust.edges].reverse() }, true)).toEqual(ownership);
  expect(ownership.get("entry")?.[0].role).toBe("Mutable borrow");
  expect(rust.nodes.find(({ id }) => id === "closure")?.inputs[0].ty).toBe("&mut Encounter");
});

it("keeps deferred input wires connected without a definite borrow style, including redirected inputs", () => {
  const rust = rustAccessFlow();
  const closed = flowToGraph(rust, {}, true);
  const expanded = flowToGraph(rust, {}, true, { level: "everything" });
  for (const [graph, target] of [[closed, "nested-future"], [expanded, "nested-future/e/0.0"]] as const) {
    const wire = graph.edges.find((edge) => edge.target === target && edge.sourceHandle === "var:encounter");
    expect(wire).toMatchObject({ source: "entry", kind: "data", data: { access: null, deferredBorrow: true } });
    expect(edgeStyle(wire!)).toMatchObject({ type: "ownership", data: { access: null, deferredBorrow: true } });
  }
  expect(closed.edges.find((edge) => edge.target === "read")?.data).toMatchObject({ access: "Borrow" });
  expect(closed.edges.find((edge) => edge.targetHandle === "use:index")?.data).toMatchObject({ access: "Copy" });
});

it("keeps capture-edge borrow modes out of the pin Inspector while preserving its type and origin", async () => {
  const rust = rustAccessFlow();
  const previous = useUi.getState();
  const host = document.createElement("div");
  const root = createRoot(host);
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, retry: false } } });
  client.setQueryData(["catalog"], { concepts: {}, categories: {}, families: {}, operations: {}, flavors: {} });
  client.setQueryData(["structure"], { symbols: [], edges: [] });
  client.setQueryData(["flow", rust.symbol], rust);
  for (const id of ["closure", "callback", "future", "nested-future", "read"]) {
    client.setQueryData<BorrowOrigins>(["borrow-origin", previous.projectSession, rust.symbol, id, "use:encounter", undefined, 0], {
      origins: [{ kind: "Caller", owner: null, borrowed: "encounter", through: [], call: null }],
      partial: false, next_offset: null,
    });
  }
  const select = (id: string, pin = "use:encounter") => useUi.getState().select({ kind: "flow-node", flow: rust.symbol, id, pin });
  try {
    await act(async () => {
      useUi.setState({ blueprintExplanations: true });
      select("closure");
      root.render(<QueryClientProvider client={client}><Inspector /></QueryClientProvider>);
    });
    for (const id of ["closure", "callback", "future", "nested-future"]) {
      await act(async () => select(id));
      expect(host.textContent).toContain("uses · encounter");
      expect(host.textContent).toContain("mutable reference");
      expect(host.textContent).toContain("&mut Encounter");
      expect(host.querySelector('[aria-label="Borrow origin"]')?.textContent).toContain("From caller");
      expect(host.textContent).not.toContain("reads the value");
      expect(host.textContent).not.toContain("borrows to change");
    }
    await act(async () => select("nested-future", "use:index"));
    expect(host.textContent).toContain("copies · index");
    expect(host.textContent).toContain("Copy value");
    await act(async () => select("read"));
    expect(host.textContent).toContain("borrows · encounter");
    expect(host.textContent).toContain("reads the value");
  } finally {
    await act(async () => root.unmount());
    useUi.setState(previous);
    client.clear();
  }
});

it("keeps the labeled arm and constant comparison on an embedded match", () => {
  const graph = structuredClone(flow);
  graph.nodes[0].expr = expression("Match", "match", span(11, 78));
  graph.reading = [{ node, path: "0", span: span(60, 71), basis: "TypeAnalysis", content: {
    kind: "Pattern", input: "pair", route: "case:0", route_label: "Case · (LIMIT, value)",
    parts: [
      { projection: ".0", span: span(62, 66), condition: "LIMIT" },
      { projection: ".1", span: span(68, 69), binding: "value", access: "Copy" },
    ],
  } }];
  const opened = flowToGraph(graph, { [expansionKey(graph.symbol, node)]: true }, true);
  expect((opened.nodes.find((entry) => entry.id === node)!.data as BlueprintData).reading).toEqual([]);
  expect((opened.nodes.find((entry) => entry.id === `${node}/e/0`)!.data as ExprData).reading).toEqual(graph.reading);
});

it("shows both the condition and the name in an @ pattern", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const binding = span(60, 64);
  const site: ReadingSite = { node, span: span(55, 78), basis: "Syntax", content: {
    kind: "Pattern", input: "slot_value", parts: [
      { projection: "Some.0", span: binding, binding: "slot", condition: "Value::Array(_)", access: "BorrowMut" },
    ],
  } };
  let inspected: Span | undefined;
  try {
    await act(async () => root.render(<BlueprintReading full sites={[site]} inspect={(_, span) => { inspected = span; }} />));
    const row = host.querySelector<HTMLButtonElement>(".reading-outcome button")!;
    expect(row.textContent).toBe("Some.0 → Match Value::Array(_) · bind slot");
    expect(host.querySelector(".reading-action")?.textContent).toBe("Borrow to change");
    await act(async () => row.click());
    expect(inspected).toEqual(binding);
  } finally {
    await act(async () => root.unmount());
  }
});

it("distinguishes a single matching pattern from a chain of conditions", () => {
  const loop = { ...flow.nodes[0], flavors: ["while-let"] };
  expect(routeName(loop, "body")).toBe("Loop body · when the conditions pass");
  const single = expression("TypeCheck", "matches Some(value)", span(0, 78), [], { flavors: ["if-let"] });
  const branch = { ...flow.nodes[0], concept: "Branch" as const, flavors: ["if-let"], expr: single };
  expect(routeName(branch, "true")).toBe("When the pattern matches");
  expect(executionPinLabel(branch, "true", "True")).toBe("Matched · bindings available");
  expect(executionPinLabel(branch, "false", "False")).toBe("Pattern did not match");
  const guard = expression("Call", "is_ready", span(69, 78));
  const chain = expression("Operation", "and", span(0, 78), [single, guard], { op: "and" });
  const chainedBranch = { ...branch, expr: chain };
  expect(routeName(chainedBranch, "true")).toBe("When the conditions pass");
  expect(executionPinLabel(chainedBranch, "true", "True")).toBe("All conditions passed · bindings available");
  expect(executionPinLabel(chainedBranch, "false", "False")).toBe("A condition did not pass");
  expect(executionPinLabel({ ...loop, expr: chain }, "body", "Loop body")).toBe("All conditions passed · bindings available");
});

it("keeps an embedded let-else reading off the enclosing statement's execution pins", () => {
  const statement = {
    ...flow.nodes[0], outputs: ["out", "else"].map((id) => ({ id, kind: "Exec" as const, label: id, ty: null })),
  };
  const reading: ReadingSite = { node, span: span(70, 74), basis: "Syntax", content: {
    kind: "Paths", input: "Match a value", deferred: false, branches: [
      { label: "Pattern matches", pin: "out", action: "Use the binding", output: "Continue below" },
      { label: "Pattern mismatch", pin: "else", action: "Leave this path", output: "Run else" },
    ],
  } };
  expect(executionReading(statement, [reading])?.site).toBe(reading);
  expect(executionReading(statement, [{ ...reading, path: "0.0" }])).toBeUndefined();
});

it("reuses a statement's existing execution pins for a local question mark", () => {
  const graph = structuredClone(flow);
  graph.nodes[0].outputs = ["out", "error"].map((id) => ({ id, kind: "Exec", label: id, ty: null }));
  graph.reading = [{
    node, path: "0.0", span: span(77, 78), basis: "TypeAnalysis", content: {
      kind: "Paths", input: "candidate", deferred: false, branches: [
        { label: "Found", pin: "out", action: "Unwrap Some", output: "Continue with the value" },
        { label: "Missing", pin: "error", action: "Return from prepare_crew", output: "None" },
      ],
    },
  }];
  const result = flowToGraph(graph, {}, true).nodes[0].data as BlueprintData;
  expect(result.reading).toEqual([]);
  expect(result.readingExec).toEqual({
    out: "Found · Continue with the value", error: "Missing · Return from prepare_crew",
  });
});

it("labels both routes, highlights the binding and inspects the lookup behind the pattern", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const graph = flowToGraph(flow, {}, true);
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, retry: false } } });
  const encoder = new TextEncoder();
  const prefix = "// 🚀\n".padEnd(57, " ") + "|";
  const source = { revision: flow.source_revision, bytes: encoder.encode(prefix + "&(host, _ )| host") };
  expect(encoder.encode(prefix)).toHaveLength(60);
  client.setQueryData(["source", flow.span.file], source);
  const all = flow.reading!;
  const pattern = all.filter((site) => site.content.kind === "Pattern");
  const origin = readingOrigin(all, pattern);
  client.setQueryData<BorrowOrigins>(["borrow-origin", useUi.getState().projectSession, flow.symbol, node, undefined, origin, 0], {
    origins: [{ kind: "Caller", owner: null, borrowed: "encounter.crew_index[crew]", through: [], call: null }],
    partial: false, next_offset: null,
  });
  const previous = useUi.getState();
  try {
    await act(async () => root.render(<BlueprintReading sites={(graph.nodes[0].data as BlueprintData).reading!} inspect={() => {}} />));
    const branches = Array.from(host.querySelectorAll("ol li")).map((item) => item.textContent);
    expect(branches).toEqual([
      "Found→Some(host)Copy the first tuple item as host",
      "Missing→NoneSkip the callback",
    ]);
    expect(host.textContent).toContain("When called");
    expect(host.textContent).toContain("Option<UIntSize>");
    await act(async () => root.render(<QueryClientProvider client={client}>
      <ReadingInspector flow={flow} node={flow.nodes[0]} sites={pattern} origin={origin} />
    </QueryClientProvider>));
    expect(host.textContent).toContain("Copy · usize");
    expect(host.textContent).toContain("Ignore");
    expect(host.textContent).toContain("From caller");
    expect(host.textContent).toContain("encounter.crew_index[crew]");
    expect(host.querySelector(".reading-source .blueprint-code-target")?.textContent).toBe("&(host, _ )");
    const binding = Array.from(host.querySelectorAll("button")).find((button) => button.textContent === ".0 → host")!;
    await act(async () => binding.click());
    expect(useUi.getState().codeFocus).toEqual(span(62, 66));
    expect(useUi.getState().codeFocusRevision).toBe(flow.source_revision);
    await act(async () => {
      client.setQueryData(["source", flow.span.file], { ...source, revision: "edited" });
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    expect(host.querySelector(".reading-source")).toBeNull();
  } finally {
    await act(async () => root.unmount());
    useUi.setState(previous);
    client.clear();
  }
});
