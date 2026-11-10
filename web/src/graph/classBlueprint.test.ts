import { describe, expect, it } from "vitest";
import type { FlowGraph } from "../gen/FlowGraph";
import type { Expr } from "../gen/Expr";
import type { FlowNode } from "../gen/FlowNode";
import type { Symbol } from "../gen/Symbol";
import { CLASS_IN, CLASS_OUT, FIELD_OUT, classBlueprintGraph, classMembers } from "./classBlueprint";
import { expansionKey } from "./flowGraph";
import { isBlueprintContainer, StructureIndex } from "./structureGraph";

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
    symbol("Computed", "Property", "Order", 12, true),
  ],
  edges: [],
});
const node = (id: string, concept: FlowNode["concept"], extra: Partial<FlowNode> = {}): FlowNode => ({
  id, concept, label: id, code: id, span, flavors: [], purity: "Impure", inputs: [], outputs: [], expr: null, writes: [], calls: [], ...extra,
});
const target = { symbol: "Audit", display: "Audit()", external: false };
const body = node("n1", "Call", {
  calls: [target, target],
  expr: { concept: "Call", label: "Audit", span, flavors: [], children: [], symbol: "Audit", target: null, op: null, ty: null, params: [] },
});
const expr = (concept: Expr["concept"], label: string, children: Expr[] = [], flavors: string[] = []): Expr =>
  ({ concept, label, children, flavors, span, symbol: null, target: null, op: null, ty: null, params: [] });
const increment = (target: Expr) => expr("UnaryOp", "++", [target], ["postfix"]);
const fieldWrite = node("n2", "SetMember", { expr: increment(expr("GetMember", ".count", [expr("Var", "this")])) });
const localWrite = node("n3", "Assign", { writes: ["count"], expr: increment(expr("Var", "count")) });
const implicitWrite = node("n4", "Assign", { writes: ["count"], flavors: ["implicit-this"], expr: increment(expr("Var", "count")) });
const otherWrite = node("n5", "SetMember", { expr: increment(expr("GetMember", ".count", [expr("Var", "other")])) });
const propertyWrite = node("n6", "SetMember", {
  expr: expr("Assign", "=", [expr("GetMember", ".Status", [expr("Var", "this")]), expr("Literal", "Ready")]),
});
const flow = (symbol: string, nodes: FlowNode[]): FlowGraph => ({
  symbol, name: symbol, language: "csharp", span, nodes,
  edges: [{ from: "entry", to: "n1", from_pin: "out", to_pin: "in", kind: "Exec", access: null }],
});
const flows = [
  flow("Submit", [node("entry", "Entry"), body, fieldWrite, localWrite, implicitWrite, otherWrite, propertyWrite]),
  flow("Audit", [node("entry", "Entry"), { ...body, calls: [], writes: [] }]),
];
const makeGraph = (open: string[], expanded: Record<string, boolean> = {}) =>
  classBlueprintGraph(index, "Order", flows, { open: new Set(open), expanded, showDataWires: true });

describe("class Blueprint", () => {
  it("shows Rust variants and trait declarations without treating them as executable bodies", () => {
    const choice = { ...symbol("Choice", "Union", null, 1), language: "rust" as const };
    const variant = { ...symbol("Choice::Count", "EnumMember", choice.id, 2), signature: "Count(i32)", language: choice.language };
    const source = { ...symbol("Source", "Trait", null, 5), language: choice.language };
    const associated = { ...symbol("Source::Item", "TypeAlias", source.id, 6), flavors: ["associated-type"], language: choice.language };
    const required = { ...symbol("Source::read", "Method", source.id, 7), flavors: ["required", "&self"], language: choice.language };
    const defaultMethod = { ...symbol("Source::reset", "Method", source.id, 8, true), flavors: ["default", "&mut self"], language: choice.language };
    const index = new StructureIndex({ symbols: [choice, variant, source, associated, required, defaultMethod], edges: [] });
    const opts = { open: new Set<string>(), expanded: {}, showDataWires: true };
    const choices = classBlueprintGraph(index, choice.id, [], opts);
    expect(choices.nodes.find((n) => n.id === variant.id)).toMatchObject({ type: "fieldCard", data: { declaration: "Count(i32)", role: "variant" } });
    const trait = classBlueprintGraph(index, source.id, [], opts);
    expect(trait.nodes.find((n) => n.id === associated.id)).toMatchObject({ type: "fieldCard", data: { note: "Associated type" } });
    expect(trait.nodes.find((n) => n.id === required.id)).toMatchObject({ type: "fieldCard", data: { note: expect.stringContaining("Required contract") } });
    expect(trait.nodes.find((n) => n.id === defaultMethod.id)).toMatchObject({ type: "methodCard", data: { note: expect.stringContaining("Default implementation") } });
    expect(classMembers(index, source.id).map((s) => s.id)).toEqual([defaultMethod.id]);
  });

  it("opens a TypeScript file as a module Blueprint with its functions, top-level code and types", () => {
    const file = "src/orders.ts";
    const ts = (id: string, kind: Symbol["kind"], parent: string | null, line: number, hasFlow = false): Symbol => ({
      ...symbol(id, kind, parent, line, hasFlow), language: "typescript", origins: [{ ...span, file, start_line: line }],
    });
    const moduleId = "ns:src/orders";
    const module = ts(moduleId, "Module", null, 1);
    const submit = ts("submit", "Function", moduleId, 2, true);
    const topLevel = ts("top-level", "Function", moduleId, 7, true);
    const order = ts("Order", "Class", moduleId, 4);
    const method = ts("Order.total", "Method", order.id, 5, true);
    const handlers = ts("handlers", "Field", moduleId, 8);
    const handle = ts("handlers.handle", "Method", handlers.id, 9, true);
    const index = new StructureIndex({ symbols: [module, submit, topLevel, order, method, handlers, handle], edges: [] });
    const functionsOnly = new StructureIndex({ symbols: [module, submit, topLevel], edges: [] });

    expect(functionsOnly.primaryTypeInFile(file)).toBeUndefined();
    expect(functionsOnly.fileBlueprint(file, true)).toBe(module);
    expect(index.fileBlueprint(file, false)).toBe(module);
    expect(index.fileBlueprint(file, true)).toBe(order);
    expect(isBlueprintContainer(module)).toBe(true);
    expect(classMembers(index, moduleId)).toEqual([submit, topLevel, handle]);

    const graph = classBlueprintGraph(index, moduleId, [
      flow(submit.id, [node("entry", "Entry"), node("n1", "Call")]),
      flow(topLevel.id, [node("entry", "Entry"), node("n1", "Call", {
        calls: [{ symbol: submit.id, display: "submit()", external: false }],
      })]),
    ], { open: new Set([submit.id, topLevel.id]), expanded: {}, showDataWires: true });
    expect(graph.nodes).toEqual(expect.arrayContaining([
      expect.objectContaining({ id: moduleId, type: "classBox" }),
      expect.objectContaining({ id: "submit::entry", parent: submit.id }),
      expect.objectContaining({ id: order.id, parent: moduleId, type: "fieldCard" }),
    ]));
    expect(graph.edges).toContainEqual(expect.objectContaining({
      kind: "calls", source: "top-level::n1", target: "submit::entry",
    }));
  });

  it("opens a Rust file with free functions and keeps separate source modules out of its graph", () => {
    const file = "src/lib.rs";
    const rust = (id: string, kind: Symbol["kind"], parent: string | null, line: number, hasFlow = false): Symbol => ({
      ...symbol(id, kind, parent, line, hasFlow), language: "rust", origins: [{ ...span, file, start_line: line }],
    });
    const root = rust("ns:sample", "Module", null, 1);
    const run = rust("sample::run", "Function", root.id, 2, true);
    const inline = { ...rust("ns:sample::inline", "Module", root.id, 4), flavors: ["inline-module"] };
    const helper = rust("sample::inline::helper", "Function", inline.id, 5, true);
    const separate = { ...rust("ns:sample::separate", "Module", root.id, 1), origins: [{ ...span, file: "src/separate.rs" }] };
    const other = rust("sample::separate::other", "Function", separate.id, 1, true);
    const index = new StructureIndex({ symbols: [root, run, inline, helper, separate, other], edges: [] });

    expect(index.fileBlueprint(file, false)).toBe(root);
    expect(index.fileBlueprint(file, true)).toBe(root);
    expect(isBlueprintContainer(root)).toBe(true);
    expect(classMembers(index, root.id)).toEqual([run, helper]);
    expect(index.fileBlueprint("src/separate.rs", true)).toBe(separate);

    const order = rust("sample::Order", "Struct", root.id, 8);
    const withType = new StructureIndex({ symbols: [...index.graph.symbols, order], edges: [] });
    expect(withType.fileBlueprint(file, true)).toBe(order);
  });

  it("orders methods and accessors by their source lines", () => {
    expect(classMembers(index, "Order").map((s) => s.id)).toEqual(["Status.get", "Submit", "Audit", "Computed"]);
    expect(makeGraph([]).layoutLinks).toEqual([
      { source: "Status.get", target: "Submit" },
      { source: "Submit", target: "Audit" },
      { source: "Audit", target: "Computed" },
    ]);
  });

  it("gives fields and properties separate nodes while keeping accessor and computed flows", () => {
    const graph = makeGraph([]);
    expect(graph.nodes.filter((n) => n.type === "fieldCard")).toEqual([
      expect.objectContaining({ id: "count", parent: "Order", layoutSide: "right", data: { symbol: expect.objectContaining({ kind: "Field" }) } }),
      expect.objectContaining({ id: "Status", parent: "Order", layoutSide: "right", data: { symbol: expect.objectContaining({ kind: "Property" }) } }),
    ]);
    expect(graph.nodes.find((n) => n.id === "Status.get")?.type).toBe("methodCard");
    expect(graph.nodes.find((n) => n.id === "Computed")?.type).toBe("methodCard");
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
      expect.objectContaining({ kind: "changes", source: "Submit", target: "count", sourceHandle: FIELD_OUT, targetHandle: CLASS_IN }),
      expect.objectContaining({ kind: "changes", source: "Submit", target: "Status" }),
    ]);
    expect(makeGraph(["Submit"]).edges.find((e) => e.kind === "calls")).toMatchObject({ source: "Submit::n1", target: "Audit" });
    const open = makeGraph(["Submit", "Audit"]);
    expect(open.edges.find((e) => e.kind === "calls")).toMatchObject({ source: "Submit::n1", target: "Audit::entry" });
    expect(open.edges.filter((e) => e.kind === "changes")).toEqual([
      expect.objectContaining({ source: "Submit::n2", target: "count", sourceHandle: CLASS_OUT, targetHandle: CLASS_IN }),
      expect.objectContaining({ source: "Submit::n4", target: "count" }),
      expect.objectContaining({ source: "Submit::n6", target: "Status" }),
    ]);
  });

  it("links writes qualified by the owning type but excludes a local with the same name", () => {
    const staticIndex = new StructureIndex({ ...index.graph, symbols: index.graph.symbols.map((s) =>
      s.id === "count" ? { ...s, flavors: ["static"] } : s.id === "Order" ? { ...s, qualified_name: "Shop.Order" } : s,
    ) });
    const short = increment(expr("GetMember", ".count", [expr("Var", "Order")]));
    const qualified = increment(expr("GetMember", ".count", [expr("GetMember", ".Order", [expr("Var", "Shop")])]));
    const statements = [
      node("n1", "SetMember", { expr: short }),
      node("n2", "SetMember", { expr: short, inputs: [{ id: "use:Order", kind: "Data", label: "Order", ty: null }] }),
      node("n3", "SetMember", { expr: qualified }),
      node("n4", "SetMember", { expr: increment(expr("GetMember", ".count", [expr("Var", "Other")])) }),
    ];
    const graph = classBlueprintGraph(staticIndex, "Order", [flow("Submit", [node("entry", "Entry"), ...statements])], {
      open: new Set(["Submit"]), expanded: {}, showDataWires: true,
    });
    expect(graph.edges.filter((e) => e.kind === "changes").map((e) => e.source)).toEqual(["Submit::n1", "Submit::n3"]);
  });

  it("recognizes global and namespace qualifiers on a generic C# type", () => {
    const genericIndex = new StructureIndex({ ...index.graph, symbols: [
      { ...symbol("ns:Shop", "Namespace", null, 1), qualified_name: "Shop" },
      ...index.graph.symbols.map((s) =>
        s.id === "count" ? { ...s, flavors: ["static"] }
          : s.id === "Order" ? { ...s, parent: "ns:Shop", name: "Order<T>", qualified_name: "Shop.Order`1" } : s),
    ] });
    const write = (id: string, receiver: Expr) => node(id, "SetMember", { expr: increment(expr("GetMember", ".count", [receiver])) });
    const graph = classBlueprintGraph(genericIndex, "Order", [flow("Submit", [
      node("entry", "Entry"),
      write("n1", expr("GetMember", ".Order<T>", [expr("Var", "global::Shop")])),
      write("n2", expr("GetMember", ".Order<T>", [expr("Var", "Shop")])),
      write("n3", expr("Var", "global::Order<T>")),
    ])], { open: new Set(["Submit"]), expanded: {}, showDataWires: true });
    expect(graph.edges.filter((e) => e.kind === "changes").map((e) => e.source)).toEqual(["Submit::n1", "Submit::n2"]);
  });
});
