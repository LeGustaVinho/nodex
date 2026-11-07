import { describe, expect, it } from "vitest";
import type { Expr } from "../gen/Expr";
import type { FlowGraph } from "../gen/FlowGraph";
import type { OperationInfo } from "../gen/OperationInfo";
import type { StructureGraph } from "../gen/StructureGraph";
import type { Symbol } from "../gen/Symbol";
import { callGraph } from "./callGraph";
import { type BlueprintData, type ExprData, flowToGraph } from "./flowGraph";
import { StructureIndex, architectureGraph, typesGraph } from "./structureGraph";

const span = { file: "A.cs", start_byte: 0, end_byte: 1, start_line: 1, start_col: 1, end_line: 1, end_col: 2 };

const flow: FlowGraph = {
  symbol: "S.M()",
  name: "M",
  language: "csharp",
  span,
  nodes: [
    {
      id: "a", concept: "Entry", label: "when M is called", code: "void M()", span, flavors: [], purity: "Impure",
      inputs: [], outputs: [{ id: "out", kind: "Exec", label: "", ty: null }], expr: null, writes: [], calls: [],
    },
    {
      id: "b", concept: "Var", label: "x = 1 + 2", code: "int x = 1 + 2;", span, flavors: [], purity: "Impure",
      inputs: [{ id: "in", kind: "Exec", label: "", ty: null }],
      outputs: [{ id: "out", kind: "Exec", label: "", ty: null }, { id: "var:x", kind: "Data", label: "x", ty: "int" }],
      expr: {
        concept: "BinaryOp", label: "+", span, flavors: [], symbol: null, target: null, op: null, ty: null, params: [],
        children: [
          { concept: "Literal", label: "1", span, flavors: [], children: [], symbol: null, target: null, op: null, ty: null, params: [] },
          { concept: "Literal", label: "2", span, flavors: [], children: [], symbol: null, target: null, op: null, ty: null, params: [] },
        ],
      },
      writes: ["x"],
      calls: [],
    },
    {
      id: "c", concept: "Return", label: "return x", code: "return x;", span, flavors: [], purity: "Impure",
      inputs: [{ id: "in", kind: "Exec", label: "", ty: null }, { id: "use:x", kind: "Data", label: "x", ty: "int" }],
      outputs: [], expr: null, writes: [], calls: [],
    },
  ],
  edges: [
    { from: "a", from_pin: "out", to: "b", to_pin: "in", kind: "Exec" },
    { from: "b", from_pin: "out", to: "c", to_pin: "in", kind: "Exec" },
    { from: "b", from_pin: "var:x", to: "c", to_pin: "use:x", kind: "Data" },
  ],
};

const e = (concept: Expr["concept"], label: string, children: Expr[] = [], extra: Partial<Expr> = {}): Expr => ({
  concept, label, span, flavors: [], children, symbol: null, target: null, op: null, ty: null, params: [], ...extra,
});

const operations: Record<string, OperationInfo> = {
  add: {
    title: "Add", symbol: "+", family: "math", pure: true, inputs: ["A:number", "B:number"], output: "Result:number", explain: "",
    cs: "", ts: "", rs: "", calls_cs: [], calls_ts: [], calls_rs: [],
  },
  "math.max": {
    title: "Max", symbol: "max", family: "math", pure: true, inputs: ["A:number", "B:number"], output: "Result:number", explain: "",
    cs: "", ts: "", rs: "", calls_cs: [], calls_ts: [], calls_rs: [],
  },
  "io.print": {
    title: "Print", symbol: "print", family: "io", pure: false, inputs: ["Values:any"], output: "", explain: "",
    cs: "", ts: "", rs: "", calls_cs: [], calls_ts: [], calls_rs: [],
  },
};

/** `int M(int a) { var x = Math.Max(a, 10) + 1; Console.WriteLine(x); items.Select(i => i + a); }` */
const opsFlow: FlowGraph = {
  symbol: "S.M(int)",
  name: "M",
  language: "csharp",
  span,
  nodes: [
    {
      id: "a", concept: "Entry", label: "when M(a) is called", code: "int M(int a)", span, flavors: [], purity: "Impure",
      inputs: [], outputs: [{ id: "out", kind: "Exec", label: "", ty: null }, { id: "var:a", kind: "Data", label: "a", ty: "int" }],
      expr: null, writes: [], calls: [],
    },
    {
      id: "b", concept: "Var", label: "x = Math.Max(a, 10) + 1", code: "var x = Math.Max(a, 10) + 1;", span, flavors: [], purity: "Impure",
      inputs: [{ id: "in", kind: "Exec", label: "", ty: null }, { id: "use:a", kind: "Data", label: "a", ty: "int" }],
      outputs: [{ id: "out", kind: "Exec", label: "", ty: null }, { id: "var:x", kind: "Data", label: "x", ty: "int" }],
      expr: e("Operation", "+", [
        e("Operation", "Max", [e("Var", "Math", [], { flavors: ["target"] }), e("Var", "a"), e("Literal", "10", [], { ty: "number" })], { op: "math.max" }),
        e("Literal", "1", [], { ty: "number" }),
      ], { op: "add", ty: "number" }),
      writes: ["x"], calls: [],
    },
    {
      id: "c", concept: "Call", label: "Console.WriteLine(x)", code: "Console.WriteLine(x);", span, flavors: [], purity: "Impure",
      inputs: [{ id: "in", kind: "Exec", label: "", ty: null }, { id: "use:x", kind: "Data", label: "x", ty: "int" }],
      outputs: [{ id: "out", kind: "Exec", label: "", ty: null }],
      expr: e("Call", "WriteLine", [e("Var", "Console", [], { flavors: ["target"] }), e("Var", "x")], { op: "io.print" }),
      writes: [], calls: [],
    },
    {
      id: "d", concept: "Call", label: "items.Select(i => i + a)", code: "items.Select(i => i + a);", span, flavors: [], purity: "Impure",
      inputs: [{ id: "in", kind: "Exec", label: "", ty: null }, { id: "use:a", kind: "Data", label: "a", ty: "int" }],
      outputs: [{ id: "out", kind: "Exec", label: "", ty: null }],
      expr: e("Call", "Select", [
        e("Var", "items", [], { flavors: ["target"] }),
        e("Lambda", "(i) => …", [e("Operation", "+", [e("Var", "i"), e("Var", "a")], { op: "add" })], { params: ["i"] }),
      ]),
      writes: [], calls: [],
    },
  ],
  edges: [
    { from: "a", from_pin: "out", to: "b", to_pin: "in", kind: "Exec" },
    { from: "b", from_pin: "out", to: "c", to_pin: "in", kind: "Exec" },
    { from: "c", from_pin: "out", to: "d", to_pin: "in", kind: "Exec" },
    { from: "a", from_pin: "var:a", to: "b", to_pin: "use:a", kind: "Data" },
    { from: "b", from_pin: "var:x", to: "c", to_pin: "use:x", kind: "Data" },
    { from: "a", from_pin: "var:a", to: "d", to_pin: "use:a", kind: "Data" },
  ],
};

describe("flowToGraph detail levels", () => {
  it("statements level draws one node per statement", () => {
    const g = flowToGraph(opsFlow, {}, true, { level: "statements", operations });
    expect(g.nodes.every((n) => n.type === "blueprint")).toBe(true);
  });

  it("operations level opens every statement, inlines constants and wires locals from their definition", () => {
    const g = flowToGraph(opsFlow, {}, true, { level: "operations", operations });
    const ids = g.nodes.filter((n) => n.type === "expr").map((n) => n.id);
    // `+` and `Max` for b; nothing for the literals, the static receiver or the local reads.
    expect(ids).toContain("b/e/0");
    expect(ids).toContain("b/e/0.0");
    expect(ids.some((id) => id.startsWith("b/e/0.0."))).toBe(false);
    const max = g.nodes.find((n) => n.id === "b/e/0.0")!.data as ExprData;
    expect(max.pins.map((p) => [p.name, p.value])).toEqual([["A", null], ["B", "10"]]);
    const add = g.nodes.find((n) => n.id === "b/e/0")!.data as ExprData;
    expect(add.pins.map((p) => [p.name, p.value])).toEqual([["A", null], ["B", "1"]]);
    // `a` is wired from the entry straight into Max's A pin; the statement-level use:a wire is gone.
    expect(g.edges.some((x) => x.source === "a" && x.sourceHandle === "var:a" && x.target === "b/e/0.0" && x.targetHandle === "c0")).toBe(true);
    expect(g.edges.some((x) => x.target === "b" && x.targetHandle === "use:a")).toBe(false);
  });

  it("operations level turns a call statement's arguments into its own pins", () => {
    const g = flowToGraph(opsFlow, {}, true, { level: "operations", operations });
    const c = g.nodes.find((n) => n.id === "c")!;
    const data = c.data as BlueprintData;
    // `Console` is a static receiver: Print has one input, Values, wired from x.
    expect(data.argPins?.map((p) => p.name)).toEqual(["Values"]);
    expect(c.ports.some((p) => p.id === "expr")).toBe(false);
    expect(g.edges.some((x) => x.source === "b" && x.sourceHandle === "var:x" && x.target === "c" && x.targetHandle === "x0")).toBe(true);
    expect(g.nodes.some((n) => n.id.startsWith("c/e/"))).toBe(false);
  });

  it("only the everything level opens lambda bodies, wiring parameters from the lambda", () => {
    const ops = flowToGraph(opsFlow, {}, true, { level: "operations", operations });
    expect(ops.nodes.some((n) => n.id === "d/e/0.1")).toBe(true);
    expect(ops.nodes.some((n) => n.id === "d/e/0.1.0")).toBe(false);
    const all = flowToGraph(opsFlow, {}, true, { level: "everything", operations });
    const lambda = all.nodes.find((n) => n.id === "d/e/0.1")!;
    expect((lambda.data as ExprData).params).toEqual(["i"]);
    expect(lambda.ports.some((p) => p.id === "p:i")).toBe(true);
    expect(all.edges.some((x) => x.source === "d/e/0.1" && x.sourceHandle === "p:i" && x.target === "d/e/0.1.0" && x.targetHandle === "c0")).toBe(true);
    // `a` inside the lambda is still the method's parameter.
    expect(all.edges.some((x) => x.source === "a" && x.target === "d/e/0.1.0" && x.targetHandle === "c1")).toBe(true);
  });

  it("a statement can still be closed in the compact levels", () => {
    const g = flowToGraph(opsFlow, { b: false }, true, { level: "operations", operations });
    expect(g.nodes.some((n) => n.id.startsWith("b/e/"))).toBe(false);
    expect(g.edges.some((x) => x.target === "b" && x.targetHandle === "use:a")).toBe(true);
  });
});

describe("flowToGraph", () => {
  it("lays out exec edges and draws data wires without layout weight", () => {
    const g = flowToGraph(flow, {}, true);
    expect(g.nodes).toHaveLength(3);
    const data = g.edges.find((e) => e.kind === "data");
    expect(data?.layout).toBe(false);
    expect((data?.data as { ty: string }).ty).toBe("int");
    expect(g.edges.filter((e) => e.kind === "exec").every((e) => e.layout)).toBe(true);
  });

  it("hides data wires when asked", () => {
    expect(flowToGraph(flow, {}, false).edges.some((e) => e.kind === "data")).toBe(false);
  });

  it("expands expression trees into pure nodes wired into the statement", () => {
    const g = flowToGraph(flow, { b: true }, true);
    const exprNodes = g.nodes.filter((n) => n.type === "expr");
    expect(exprNodes.map((n) => n.id)).toEqual(["b/e/0", "b/e/0.0", "b/e/0.1"]);
    expect(g.edges.some((e) => e.source === "b/e/0" && e.target === "b" && e.targetHandle === "expr")).toBe(true);
    expect(g.nodes.find((n) => n.id === "b")?.ports.some((p) => p.id === "expr")).toBe(true);
  });
});

const sym = (id: string, kind: Symbol["kind"], parent: string | null, extra: Partial<Symbol> = {}): Symbol => ({
  id, kind, name: id.split(".").pop()!, qualified_name: id.replace(/^ns:/, ""), parent, language: "csharp",
  origins: [span], flavors: [], signature: null, type_text: null, has_flow: false, external: false, doc: null, ...extra,
});

const structure: StructureGraph = {
  symbols: [
    sym("ns:App", "Namespace", null),
    sym("ns:System", "Namespace", null, { external: true }),
    sym("App.Shape", "Class", "ns:App"),
    sym("App.Circle", "Class", "ns:App"),
    sym("App.Circle.Area()", "Method", "App.Circle", { has_flow: true }),
    sym("App.Circle._r", "Field", "App.Circle"),
    sym("ext:MonoBehaviour", "Class", null, { external: true }),
  ],
  edges: [
    { from: "ns:App", to: "ns:System", kind: "Imports" },
    { from: "App.Circle", to: "App.Shape", kind: "Inherits" },
    { from: "App.Shape", to: "ext:MonoBehaviour", kind: "Inherits" },
    { from: "App.Circle._r", to: "App.Shape", kind: "References" },
  ],
};

describe("structure graphs", () => {
  const index = new StructureIndex(structure);

  it("architecture shows namespaces and imports", () => {
    const g = architectureGraph(index);
    expect(g.nodes.map((n) => n.id).sort()).toEqual(["ns:App", "ns:System"]);
    expect(g.edges).toHaveLength(1);
  });

  it("types view rolls member references up to their type and pulls in external bases", () => {
    const g = typesGraph(index, "App", true);
    expect(g.nodes.map((n) => n.id).sort()).toEqual(["App.Circle", "App.Shape", "ext:MonoBehaviour"]);
    expect(g.edges.map((e) => e.kind).sort()).toEqual(["inherits", "inherits", "uses"]);
    const uses = g.edges.find((e) => e.kind === "uses")!;
    expect([uses.source, uses.target]).toEqual(["App.Circle", "App.Shape"]);
  });

  it("types view can hide usage edges", () => {
    expect(typesGraph(index, "App", false).edges.some((e) => e.kind === "uses")).toBe(false);
  });
});

describe("callGraph", () => {
  const calls: StructureGraph = {
    symbols: [
      sym("ns:App", "Namespace", null),
      sym("App.S", "Class", "ns:App"),
      sym("App.S.Main()", "Method", "App.S", { has_flow: true }),
      sym("App.S.Ship()", "Method", "App.S", { has_flow: true }),
      sym("App.S.Shipped", "Event", "App.S"),
      sym("App.S.Log()", "Method", "App.S", { has_flow: true }),
      sym("App.S.Unrelated()", "Method", "App.S", { has_flow: true }),
    ],
    edges: [
      { from: "App.S.Main()", to: "App.S.Ship()", kind: "Calls" },
      { from: "App.S.Ship()", to: "App.S.Shipped", kind: "Raises" },
      { from: "App.S.Shipped", to: "App.S.Log()", kind: "Subscribes" },
      { from: "App.S.Unrelated()", to: "App.S.Log()", kind: "Calls" },
    ],
  };
  const index = new StructureIndex(calls);

  it("follows calls, raises and subscriptions in both directions up to the depth", () => {
    const g = callGraph(index, "App.S.Ship()", 2);
    expect(g.nodes.map((n) => n.id).sort()).toEqual(["App.S.Log()", "App.S.Main()", "App.S.Ship()", "App.S.Shipped"]);
    expect(g.edges.map((e) => e.kind).sort()).toEqual(["calls", "raises", "subscribes"]);
  });

  it("limits the depth", () => {
    expect(callGraph(index, "App.S.Ship()", 1).nodes).toHaveLength(3);
  });
});
