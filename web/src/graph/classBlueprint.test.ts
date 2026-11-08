import { describe, expect, it } from "vitest";
import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import type { Symbol } from "../gen/Symbol";
import { classBlueprintGraph, classMembers } from "./classBlueprint";
import { expansionKey } from "./flowGraph";
import { StructureIndex } from "./structureGraph";

const span = { file: "Order.cs", start_byte: 0, end_byte: 1, start_line: 1, start_col: 1, end_line: 1, end_col: 2 };
function symbol(id: string, kind: Symbol["kind"], parent: string | null, start_line: number, has_flow = false): Symbol {
  return { id, kind, parent, name: id, qualified_name: id, language: "csharp", origins: [{ ...span, start_line }],
    flavors: [], signature: null, type_text: null, has_flow, external: false, doc: null };
}
const index = new StructureIndex({
  symbols: [
    symbol("Order", "Class", null, 1),
    symbol("Audit", "Method", "Order", 9, true),
    symbol("Submit", "Method", "Order", 5, true),
    symbol("count", "Field", "Order", 2),
    symbol("Status", "Property", "Order", 3),
    symbol("Status.get", "Method", "Status", 4, true),
  ],
  edges: [],
});
const node = (id: string, concept: FlowNode["concept"], extra: Partial<FlowNode> = {}): FlowNode => ({
  id, concept, label: id, code: id, span, flavors: [], purity: "Impure", inputs: [], outputs: [], expr: null, writes: [], calls: [], ...extra,
});
const target = { symbol: "Audit", display: "Audit()", external: false };
const body = node("n1", "Call", {
  writes: ["count", "local"], calls: [target, target],
  expr: { concept: "Call", label: "Audit", span, flavors: [], children: [], symbol: "Audit", target: null, op: null, ty: null, params: [] },
});
const flow = (symbol: string, nodes: FlowNode[]): FlowGraph => ({
  symbol, name: symbol, language: "csharp", span, nodes,
  edges: [{ from: "entry", to: "n1", from_pin: "out", to_pin: "in", kind: "Exec", access: null }],
});
const flows = [
  flow("Submit", [node("entry", "Entry"), body]),
  flow("Audit", [node("entry", "Entry"), { ...body, calls: [], writes: [] }]),
];
const makeGraph = (open: string[], expanded: Record<string, boolean> = {}) =>
  classBlueprintGraph(index, "Order", flows, { open: new Set(open), expanded, showDataWires: true });

describe("class Blueprint", () => {
  it("orders methods and accessors by their source lines", () => {
    expect(classMembers(index, "Order").map((s) => s.id)).toEqual(["Status.get", "Submit", "Audit"]);
    expect(makeGraph([]).layoutLinks).toEqual([
      { source: "Status.get", target: "Submit" },
      { source: "Submit", target: "Audit" },
      { source: "Audit", target: "fields:Order" },
    ]);
  });

  it("prefixes canvas IDs, parents and expression attachments while keeping the original runtime IDs", () => {
    const graph = makeGraph(["Submit", "Audit"], { [expansionKey("Submit", "n1")]: true });
    expect(new Set(graph.nodes.map((n) => n.id)).size).toBe(graph.nodes.length);
    expect(graph.nodes.find((n) => n.id === "Submit::n1")).toMatchObject({ parent: "Submit", data: { node: { id: "n1" }, flow: "Submit" } });
    const expr = graph.nodes.find((n) => n.id === "Submit::n1/e/0");
    expect(expr).toMatchObject({ parent: "Submit", attach: { to: "Submit::n1" }, data: { ownerId: "n1", flow: "Submit" } });
    expect(graph.nodes.some((n) => n.id.startsWith("Audit::n1/e/"))).toBe(false);
    expect(graph.edges.find((e) => e.kind === "exec")).toMatchObject({ source: "Submit::entry", target: "Submit::n1", sourceHandle: "out" });
  });

  it("links the method cards or their inner statements and deduplicates calls and field writes", () => {
    const closed = makeGraph([]);
    expect(closed.edges).toEqual([
      expect.objectContaining({ kind: "calls", source: "Submit", target: "Audit" }),
      expect.objectContaining({ kind: "changes", source: "Submit", target: "fields:Order" }),
    ]);
    expect(makeGraph(["Submit"]).edges.find((e) => e.kind === "calls")).toMatchObject({ source: "Submit::n1", target: "Audit" });
    const open = makeGraph(["Submit", "Audit"]);
    expect(open.edges.find((e) => e.kind === "calls")).toMatchObject({ source: "Submit::n1", target: "Audit::entry" });
    expect(open.edges.find((e) => e.kind === "changes")).toMatchObject({ source: "Submit::n1", target: "fields:Order" });
    expect(open.nodes.find((n) => n.id === "fields:Order")?.data).toEqual({ fields: [expect.objectContaining({ id: "count" })] });
  });
});
