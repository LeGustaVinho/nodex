import { describe, expect, it } from "vitest";
import type { FlowGraph } from "../gen/FlowGraph";
import type { StructureGraph } from "../gen/StructureGraph";
import type { Symbol } from "../gen/Symbol";
import { callGraph } from "./callGraph";
import { flowToGraph } from "./flowGraph";
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
        concept: "BinaryOp", label: "+", span, flavors: [], symbol: null, target: null,
        children: [
          { concept: "Literal", label: "1", span, flavors: [], children: [], symbol: null, target: null },
          { concept: "Literal", label: "2", span, flavors: [], children: [], symbol: null, target: null },
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
