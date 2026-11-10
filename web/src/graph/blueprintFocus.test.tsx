import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { BlueprintCode } from "../components/HighlightedCode";
import type { Expr } from "../gen/Expr";
import type { FlowGraph } from "../gen/FlowGraph";
import { type BlueprintData, type ExprData, DETAIL_LEVELS, expansionKey, flowToGraph } from "./flowGraph";
import { sourceCode } from "./blueprintSource";
import { expressionExplanations, resolvePinDetails, sequenceArgumentRoles, sequenceCallbackRoles, sequencePreview, statementDetailFlavors } from "./blueprintExplanation";
import { bindingDetails } from "./bindingDetails";

const span = { file: "test.ts", start_byte: 0, end_byte: 20, start_line: 1, end_line: 1, start_col: 1, end_col: 21 };
const expr: Expr = { concept: "Call", label: "inner", span, flavors: [], children: [], op: null, ty: null, target: null, symbol: null, params: [] };

it("keeps boxing on the value pin of an expanded assignment", () => {
  const code = "boxed = x;";
  const at = { ...span, file: "test.cs", end_byte: code.length };
  const flow: FlowGraph = {
    symbol: "f", name: "f", language: "csharp", span: at, edges: [], source_revision: "current",
    nodes: [{
      id: "assign", concept: "Assign", label: "set boxed", code, span: at,
      purity: "Impure", flavors: [], inputs: [], outputs: [], writes: [], calls: [],
      expr: { ...expr, concept: "Assign", span: at, children: [
        { ...expr, concept: "Var", label: "boxed", span: { ...at, end_byte: 5 } },
        { ...expr, concept: "Var", label: "x", span: { ...at, start_byte: 8, end_byte: 9 }, flavors: ["boxing"] },
      ] },
    }],
  };
  const source = { revision: "current", bytes: new TextEncoder().encode(code) };
  const flavor = (name: string) => name === "boxing" ? { label: "Box value", explain: "The value is copied into an object." } : undefined;
  expect(statementDetailFlavors(flow.nodes[0], "csharp", false)).toEqual(["boxing"]);
  for (const level of ["operations", "everything"] as const) {
    const data = flowToGraph(flow, {}, false, { level, source }).nodes[0].data as BlueprintData;
    const value = data.argPins?.find((pin) => pin.name === "Value");
    expect(value?.languageDetails?.path).toBe("0.1");
    expect(resolvePinDetails(value!.languageDetails!, flavor).map((detail) => detail.label)).toEqual(["Box value"]);
  }
});

it.each(["Call", "Return"] as const)("keeps boxing on an argument beyond the expansion limit of a %s", (concept) => {
  const args = [...Array.from({ length: 81 }, () => "f()"), "(object)x"];
  const code = `accept(${args.join(", ")});`;
  const at = { ...span, file: "test.cs", end_byte: code.length };
  let start = "accept(".length;
  const children: Expr[] = args.map((arg) => {
    const location = { ...at, start_byte: start, end_byte: start + arg.length };
    start = location.end_byte + ", ".length;
    return arg === "(object)x"
      ? { ...expr, concept: "Convert", label: arg, span: location, flavors: ["boxing"] }
      : { ...expr, label: arg, span: location };
  });
  const flow: FlowGraph = {
    symbol: "f", name: "f", language: "csharp", span: at, edges: [], source_revision: "current",
    nodes: [{
      id: "step", concept, label: "accept", code, span: at,
      purity: "Impure", flavors: [], inputs: [], outputs: [], writes: [], calls: [],
      expr: { ...expr, span: at, children },
    }],
  };
  const source = { revision: "current", bytes: new TextEncoder().encode(code) };
  const nodes = flowToGraph(flow, {}, false, { level: "operations", source }).nodes;
  const pins = concept === "Call"
    ? (nodes.find((node) => node.id === "step")!.data as BlueprintData).argPins
    : (nodes.find((node) => node.id === "step/e/0")!.data as ExprData).pins;
  const boxed = pins?.find((pin) => pin.value === "(object)x");
  expect(boxed?.languageDetails).toMatchObject({ path: "0.81", flavors: ["boxing"] });
});

it("distinguishes C# alias rebinding from assigning through the alias", () => {
  const details = bindingDetails({ form: "RefReadonly", reassignable: true, has_initializer: true }, "csharp");
  expect(details[0].explain).toContain("cannot assign a value through this alias");
  expect(details[1].explain).toContain("alias = ref other");
});

it("keeps syntax details and drops compiler details when the file and graph revisions differ", () => {
  const code = "value ?? fallback";
  const at = { ...span, end_byte: code.length };
  const flow: FlowGraph = {
    symbol: "f", name: "f", language: "typescript", span: at, edges: [], source_revision: "old",
    nodes: [{
      id: "value", concept: "Return", label: "return value", code, span: at,
      purity: "Impure", flavors: [], inputs: [], outputs: [], writes: [], calls: [],
      expr: { ...expr, span: at, concept: "Fallback", children: [
        { ...expr, span: { ...at, end_byte: 5 }, concept: "Var", label: "value", ty: "string", flavors: ["narrowed:string"] },
      ] },
    }],
  };
  const bytes = new TextEncoder().encode(code);
  const same = flowToGraph(flow, {}, false, { source: { revision: "old", bytes } }).nodes[0].data as BlueprintData;
  expect(statementDetailFlavors(same.node, flow.language, false)).toEqual(["fallback", "narrowed:string"]);
  const edited = flowToGraph(flow, {}, false, { source: { revision: "new", bytes } }).nodes[0].data as BlueprintData;
  expect(statementDetailFlavors(edited.node, flow.language, false)).toEqual(["fallback"]);
  expect(edited.node.expr?.children[0].ty).toBeNull();
});

it("labels execution timing only on a recognized collection stage", () => {
  const map = { ...expr, label: "map(f)", flavors: ["stage"] };
  expect(expressionExplanations(map, "typescript")).toEqual([]);
  expect(expressionExplanations({ ...map, flavors: ["stage", "sequence:array"] }, "typescript"))
    .toEqual(["Transform each item", "Runs when called"]);
  expect(expressionExplanations({ ...map, flavors: ["stage", "sequence:iterator"] }, "rust"))
    .toEqual(["Transform each item", "Runs when the sequence is consumed"]);
  expect(expressionExplanations({ ...map, symbol: "project.map", flavors: ["stage", "sequence:array"] }, "typescript")).toEqual([]);
});

it("keeps the order of collection stages and distinguishes seeded reductions", () => {
  const callback: Expr = { ...expr, concept: "Lambda", params: ["a", "b"] };
  const seed: Expr = { ...expr, concept: "Literal", label: "0" };
  const reduce: Expr = { ...expr, label: "reduce(f, 0)", flavors: ["sequence:array"], children: [callback, seed] };
  const pipeline: Expr = { ...expr, concept: "Pipeline", children: [
    { ...expr, concept: "Var", label: "items" },
    { ...expr, label: "filter(test)", flavors: ["sequence:array"] },
    { ...expr, label: "map(f)", symbol: "project.map" },
    reduce,
  ] };
  expect(sequencePreview(pipeline)).toEqual({ source: "items", stages: [
    { action: "Keep items that pass the test", timing: "Runs when called" },
    { action: "map(f)", timing: "Timing not determined" },
    { action: "Update the accumulator for each item", timing: "Runs when called" },
  ] });
  expect(sequenceArgumentRoles(reduce)).toEqual(["Update total", "Initial value"]);
  expect(sequenceCallbackRoles(reduce, 0)).toEqual({
    params: ["Current total", "Item", "Index", "Source"],
    result: "Updated total for the next item",
  });
  expect(expressionExplanations(reduce, "typescript")).toContain("Empty sequence: keep the initial value");
  expect(expressionExplanations({ ...reduce, children: [callback] }, "typescript"))
    .toContain("Start at the first item · empty raises an error");
});

it("marks intermediate values as examples and distinguishes eager arrays from lazy iterators", () => {
  const stage = (label: string, family: string): Expr => ({ ...expr, label, flavors: [family] });
  const pipeline = (stages: Expr[]): Expr => ({ ...expr, concept: "Pipeline", children: [{ ...expr, label: "numbers" }, ...stages] });
  const arrays = pipeline(["filter", "map", "reduce"].map((name) => stage(name, "sequence:array")));
  expect(sequencePreview(arrays)?.example).toEqual(["Keep evens: [1, 2, 3, 4] → [2, 4]", "Square: [2, 4] → [4, 16]", "Add: 0 → 4 → 20"]);
  const iterators = pipeline([stage("iter", "rust-iter:borrow"), ...["filter", "map", "sum"].map((name) => stage(name, "sequence:iterator"))]);
  expect(sequencePreview(iterators)?.example).toContain("Keep evens and square one item at a time");
  const linq = pipeline(["Where", "Select", "Sum"].map((name) => stage(name, "sequence:linq")));
  expect(sequencePreview(linq)?.example).toEqual(sequencePreview(iterators)?.example);
  expect(expressionExplanations(stage("and_then", "option:and_then"), "rust"))
    .toEqual(["Has value: use the Option returned by the function", "Absent: keep None"]);
});

it("shows async mapping and waiting as separate steps in a Promise pipeline", () => {
  const values = { ...expr, concept: "Var" as const, label: "values", flavors: ["target"] };
  const callback = { ...expr, concept: "Lambda" as const, flavors: ["async"] };
  const map = { ...expr, label: "map", flavors: ["sequence:array"], children: [values, callback] };
  const all = { ...expr, label: "all", flavors: ["promise:all"], children: [{ ...values, label: "Promise" }, map] };
  const recover = { ...expr, label: "catch(f)", flavors: ["stage", "sequence:promise"], children: [callback] };
  expect(sequencePreview({ ...expr, concept: "Pipeline", children: [all, recover] })).toEqual({
    source: "values", stages: [
      { action: "Produce a promise for each item", timing: "Starts each callback when called" },
      { action: "Wait for all results", timing: "Rejects when an input rejects" },
      { action: "Handle a rejection from the previous step", timing: "Runs after the previous promise settles" },
    ],
  });
});

it("moves code into expanded expressions without changing node IDs or edges", () => {
  const graph: FlowGraph = {
    symbol: "f", name: "f", language: "typescript", span, edges: [],
    nodes: [{
      id: "statement", label: "host.outer(inner())", code: "host.outer(inner());", span, concept: "Call", purity: "Impure",
      flavors: [], inputs: [], outputs: [], writes: [], calls: [], expr: { ...expr, label: "outer", children: [expr] },
    }],
  };
  const outer = graph.nodes[0];
  const focused: FlowGraph = {
    ...graph,
    nodes: [{
      ...outer,
      focus: { primary: "outer", prefix: "host." },
      expr: { ...outer.expr!, focus: { primary: "outer", prefix: "host." }, children: [{ ...expr, focus: { primary: "inner" } }] },
    }],
  };
  const expanded = { [expansionKey(graph.symbol, outer.id)]: true };
  for (const { id: level } of DETAIL_LEVELS) {
    const old = flowToGraph(graph, expanded, true, { level });
    const next = flowToGraph(focused, expanded, true, { level });
    expect(next.nodes.map((n) => n.id)).toEqual(old.nodes.map((n) => n.id));
    expect(next.edges).toEqual(old.edges);
    const summary = next.nodes.find((n) => n.id === outer.id)!.data as BlueprintData;
    expect(summary.code).toEqual(level === "statements" ? undefined : { text: "outer", emphasis: [0, 5] });
    const inner = "statement/e/0.0";
    expect((next.nodes.find((n) => n.id === inner)!.data as ExprData).code).toEqual({ text: "inner", emphasis: [0, 5] });
    expect(next.nodes.find((n) => n.id === inner)!.height).toBe(old.nodes.find((n) => n.id === inner)!.height);
  }
});

it("renders escaped code once and emphasizes the target inside it", () => {
  const markup = renderToStaticMarkup(
    <BlueprintCode language="javascript" preview={{ text: 'qualifier."<img> 🐈"', emphasis: [10, 20] }} target="symbol" />,
  );
  expect(markup).toContain('title="symbol"');
  expect(markup).toContain('qualifier.<strong class="blueprint-code-target">&quot;&lt;img&gt; 🐈&quot;</strong>');
  expect(markup).not.toContain("<img>");
});

it("uses the written condition in Statements and transfers it to the connected comparison on expansion", () => {
  const text = "if (this.gameEvent != null)";
  const source = { bytes: new TextEncoder().encode(text), revision: "current" };
  const at = (start: number, end: number) => ({ ...span, start_byte: start, end_byte: end });
  const flow: FlowGraph = {
    symbol: "f", name: "f", language: "csharp", span: at(0, text.length), edges: [], source_revision: "current",
    nodes: [{
      id: "condition", concept: "Branch", label: "if this.gameEvent != null", code: text, span: at(0, text.length),
      purity: "Impure", flavors: [], inputs: [], outputs: [], writes: [], calls: [],
      focus: { primary: "this.gameEvent != null", primary_bytes: [4, text.length - 1] },
      expr: { ...expr, concept: "Operation", label: "!=", op: "is_present", span: at(4, text.length - 1),
        children: [{ ...expr, concept: "GetMember", label: ".gameEvent", span: at(4, 18),
          focus: { primary: "gameEvent", primary_bytes: [9, 18] } }] },
    }],
  };
  const closed = flowToGraph(flow, {}, true, { source });
  expect((closed.nodes[0].data as BlueprintData).code).toEqual({ text, emphasis: [4, 26] });
  for (const { id: level } of DETAIL_LEVELS) {
    const opened = flowToGraph(flow, { [expansionKey("f", "condition")]: true }, true, { source, level });
    expect(opened.nodes.find((n) => n.id === "condition")?.data).toMatchObject({ code: undefined, detailsLabel: "Condition" });
    expect(opened.nodes.find((n) => n.id === "condition/e/0")?.data).toMatchObject({ code: { text: "□ != null" } });
    expect(opened.nodes.find((n) => n.id === "condition/e/0.0")?.data).toMatchObject({ code: { text: "this.gameEvent", emphasis: [5, 14] } });
  }
});

it("maps UTF-8 ranges to the exact repeated target and leaves ambiguous stale text unmarked", () => {
  const text = 'obj.é = obj.é + "🐈  a";';
  const bytes = new TextEncoder().encode(text);
  const sourceSpan = { ...span, start_byte: 0, end_byte: bytes.length };
  expect(sourceCode(bytes, sourceSpan, { primary: "é", primary_bytes: [4, 6] }, [], text))
    .toEqual({ text, emphasis: [4, 5] });
  expect(sourceCode(bytes, sourceSpan, { primary: "é" }, [], text)?.emphasis).toBeUndefined();
  expect(sourceCode(bytes, sourceSpan, { primary: "é", primary_bytes: [5, 7] }, [], text)?.emphasis).toBeUndefined();
});

it("keeps the written header separate from a body represented by other nodes", () => {
  const text = 'let Some(message) = parse("a  b") else { return; };';
  const bytes = new TextEncoder().encode(text);
  const whole = { ...span, start_byte: 0, end_byte: bytes.length };
  const header = sourceCode(bytes, whole, undefined, [], 'let Some(message) = parse("a b") else');
  expect(header?.text).toBe('let Some(message) = parse("a  b") else');
});

it("leaves space between focused expressions with multiple inputs", () => {
  const literal: Expr = { ...expr, concept: "Literal", label: "1" };
  const call = (name: string): Expr => ({ ...expr, label: name, focus: { primary: name }, children: [literal, { ...literal, label: "2" }] });
  const flow: FlowGraph = {
    symbol: "f", name: "f", language: "typescript", span, edges: [],
    nodes: [{
      id: "call", concept: "Call", label: "sink(first(1, 2), second(1, 2))", code: "sink(first(1, 2), second(1, 2));", span,
      purity: "Impure", flavors: [], inputs: [], outputs: [], writes: [], calls: [],
      expr: { ...expr, label: "sink", children: [call("first"), call("second")] },
    }],
  };
  const graph = flowToGraph(flow, {}, true, { level: "operations" });
  const [first, second] = graph.nodes.filter((node) => node.type === "expr");
  expect(first.attach!.dx).toBe(second.attach!.dx);
  expect(second.attach!.dy).toBeGreaterThanOrEqual(first.attach!.dy + first.height);
  expect(graph.nodes.find((node) => node.id === "call")!.box!.height).toBeGreaterThanOrEqual(second.attach!.dy + second.height);
});

it("shows arguments in their pins when their expressions cannot be expanded", () => {
  const children = Array.from({ length: 82 }, (_, i) => ({ ...expr, label: `read${i}` }));
  const flow: FlowGraph = {
    symbol: "f", name: "f", language: "typescript", span, edges: [],
    nodes: [{
      id: "call", concept: "Call", label: "send", code: "send(...args);", span, purity: "Impure",
      flavors: [], inputs: [], outputs: [], writes: [], calls: [],
      expr: { ...expr, label: "send", children },
    }],
  };
  const graph = flowToGraph(flow, {}, true, { level: "operations" });
  const node = graph.nodes.find((n) => n.id === "call")!.data as BlueprintData;
  expect(graph.nodes.filter((n) => n.type === "expr")).toHaveLength(80);
  expect(node.argPins?.slice(-2).map((pin) => pin.value)).toEqual(["read80", "read81"]);
});

it("uses matching UTF-8 source bytes for previews and literal pins", () => {
  const encoder = new TextEncoder();
  const before = "\uFEFF// 🐈\r\n";
  const call = 'submit( // comentário\r\n 42, "a  b");';
  const code = 'submit( // comentário 42, "a b");';
  const bytes = encoder.encode(before + call);
  const nodeSpan = { ...span, start_byte: encoder.encode(before).length, end_byte: bytes.length };
  const literal = '"a  b"';
  const literalSpan = { ...span, start_byte: encoder.encode(before + call.slice(0, call.indexOf(literal))).length, end_byte: bytes.length - 2 };
  const flow: FlowGraph = {
    symbol: "f", name: "f", language: "typescript", span, edges: [], source_revision: "matching",
    nodes: [{
      id: "call", concept: "Call", label: code, code, span: nodeSpan, purity: "Impure", flavors: [], inputs: [], outputs: [], writes: [], calls: [],
      expr: { ...expr, label: "submit", children: [{ ...expr, concept: "Literal", label: '"a b"', span: literalSpan }] },
    }],
  };
  const pins = (revision: string) => (flowToGraph(flow, {}, true, { level: "operations", source: { bytes, revision } }).nodes[0].data as BlueprintData).argPins;
  expect(sourceCode(bytes, nodeSpan, undefined, [], code)?.text).toBe(call);
  expect(sourceCode(bytes, nodeSpan, undefined, [], "} // drop(x)")).toBeUndefined();
  expect(pins("matching")?.[0].value).toBe(literal);
  expect(pins("stale")?.[0].value).toBe('"a b"');
  const changed = new TextEncoder().encode(before + call.replace("submit", "remove"));
  const stale = flowToGraph(flow, {}, true, { source: { bytes: changed, revision: "stale" } });
  expect((stale.nodes[0].data as BlueprintData).code?.text).toBe(code);
});
