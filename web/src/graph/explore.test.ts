import { describe, expect, it } from "vitest";
import type { StructEdge } from "../gen/StructEdge";
import type { StructureGraph } from "../gen/StructureGraph";
import type { Symbol } from "../gen/Symbol";
import {
  callTree,
  cardinality,
  customLayout,
  edgeFocus,
  errorFlows,
  fileSkeleton,
  fullstackFlow,
  impactRadius,
  isVendorPath,
  paramsOf,
  resolveLayout,
  testsFor,
  visibilityOf,
} from "./explore";
import { Hierarchy, hierarchyGraph, kindsOf } from "./hierarchy";
import { groupGraph, knowledgeGraph } from "./knowledgeGraph";
import type { GGraph } from "./model";
import { StructureIndex } from "./structureGraph";

const span = (file: string, line = 1) => ({ file, start_byte: 0, end_byte: 1, start_line: line, start_col: 1, end_line: line + 2, end_col: 2 });

function sym(id: string, kind: Symbol["kind"], parent: string | null, extra: Partial<Symbol> = {}): Symbol {
  const name = id.replace(/^ns:/, "").split(/[./]/).pop()!.replace(/\(.*\)$/, "");
  return {
    id, kind, name, qualified_name: id.replace(/^ns:/, ""), parent, language: "csharp", origins: [span("App/Orders.cs")], flavors: [],
    signature: null, type_text: null, has_flow: kind === "Method" || kind === "Function", external: false, doc: null, ...extra,
  };
}

const edge = (from: string, to: string, kind: StructEdge["kind"], confidence?: StructEdge["confidence"]): StructEdge => ({ from, to, kind, confidence });

// A controller calls a service, which writes an entity; a React client requests the controller;
// a test calls the service; a vendored helper sits in ThirdParty/.
const graph: StructureGraph = {
  symbols: [
    sym("ns:App", "Namespace", null),
    sym("App.OrdersController", "Class", "ns:App"),
    sym("App.OrdersController.Get()", "Method", "App.OrdersController", { origins: [span("App/Orders.cs", 10)] }),
    sym("App.OrderService", "Class", "ns:App"),
    sym("App.OrderService.Place()", "Method", "App.OrderService", { origins: [span("App/Orders.cs", 30)] }),
    sym("App.Order", "Class", "ns:App"),
    sym("App.OrderTests.Places()", "Method", null, { origins: [span("Tests/OrderTests.cs")] }),
    sym("App.Lib.Helper()", "Method", null, { origins: [span("ThirdParty/Lib/Helper.cs")] }),
    { ...sym("web/src/api.load", "Function", null), language: "typescript", origins: [span("web/src/api.ts")] },
    { ...sym("ext:InvalidOperationException", "Class", null), external: true },
  ],
  edges: [
    edge("web/src/api.load", "App.OrdersController.Get()", "HttpCalls", "Inferred"),
    edge("App.OrdersController.Get()", "App.OrderService.Place()", "Calls"),
    edge("App.OrderService.Place()", "App.Order", "Mutates"),
    edge("App.OrderService.Place()", "App.Lib.Helper()", "Calls"),
    edge("App.OrderTests.Places()", "App.OrderService.Place()", "Calls"),
    edge("App.OrderService.Place()", "ext:InvalidOperationException", "Throws"),
    edge("App.OrderService.Place()", "App.OrdersController.Get()", "CaughtBy"),
  ],
};
const index = new StructureIndex(graph);

describe("third-party code", () => {
  it("recognizes vendor folders, and Unity's plugins only in Unity projects", () => {
    expect(isVendorPath("ThirdParty/Lib/Helper.cs", false)).toBe(true);
    expect(isVendorPath("src/vendor/x.ts", false)).toBe(true);
    expect(isVendorPath("src/vendors.ts", false)).toBe(false);
    expect(isVendorPath("Assets/Plugins/Foo.cs", false)).toBe(false);
    expect(isVendorPath("Assets/Plugins/Foo.cs", true)).toBe(true);
    expect(isVendorPath("Packages/com.x/Runtime/A.cs", true)).toBe(true);
    expect(isVendorPath("Assets/Scripts/A.cs", true)).toBe(false);
  });

  it("hides vendor items of the hierarchy, with their edges", () => {
    const h = new Hierarchy(index);
    const vendor = h.vendorItems((p) => isVendorPath(p, false));
    expect(vendor.has("App.Lib.Helper()")).toBe(true);
    expect([...vendor].some((id) => id.startsWith("dir:ThirdParty"))).toBe(true);
    expect(vendor.has("ns:App")).toBe(false);
    const all = hierarchyGraph(h, index, new Set(), kindsOf(["calls", "knowledge"]));
    const hidden = hierarchyGraph(h, index, new Set(), kindsOf(["calls", "knowledge"]), { hidden: vendor });
    expect(all.nodes.length).toBeGreaterThan(hidden.nodes.length);
    expect(hidden.nodes.some((n) => vendor.has(n.id))).toBe(false);
  });

  it("drops vendor symbols from the knowledge graph but keeps the focus", () => {
    const vendor = (id: string) => id === "App.Lib.Helper()";
    const g = knowledgeGraph(index, "App.OrderService.Place()", { kinds: kindsOf(["calls", "knowledge"]), confidences: new Set(["Exact", "Inferred", "Unresolved"]), depth: 1, hidden: vendor });
    expect(g.nodes.map((n) => n.id)).not.toContain("App.Lib.Helper()");
    const focused = knowledgeGraph(index, "App.Lib.Helper()", { kinds: kindsOf(["calls"]), confidences: new Set(["Exact"]), depth: 1, hidden: vendor });
    expect(focused.nodes.map((n) => n.id)).toContain("App.Lib.Helper()");
  });
});

describe("selection focus", () => {
  const g: GGraph = {
    direction: "RIGHT",
    nodes: [
      { id: "box", type: "hgroup", data: {}, width: 1, height: 1, ports: [], container: true },
      { id: "a", type: "hnode", data: {}, width: 1, height: 1, ports: [], parent: "box" },
      { id: "b", type: "hnode", data: {}, width: 1, height: 1, ports: [] },
      { id: "c", type: "hnode", data: {}, width: 1, height: 1, ports: [] },
    ],
    edges: [
      { id: "a>b", source: "a", target: "b", kind: "calls", layout: true },
      { id: "c>a", source: "c", target: "a", kind: "calls", layout: true },
      { id: "b>c", source: "b", target: "c", kind: "calls", layout: true },
    ],
  };

  it("lights the selection's edges and dims the rest", () => {
    const f = edgeFocus(g, "a", "all");
    expect([...f.lit].sort()).toEqual(["a>b", "c>a"]);
    expect(f.hidden.size).toBe(0);
    expect(f.litNodes?.has("box")).toBe(true);
  });

  it("counts a box with what is inside it", () => {
    const f = edgeFocus(g, "box", "selection");
    expect([...f.outgoing]).toEqual(["a>b"]);
    expect([...f.incoming]).toEqual(["c>a"]);
    expect([...f.hidden]).toEqual(["b>c"]);
  });

  it("keeps only one direction, and nothing without a selection", () => {
    expect([...edgeFocus(g, "a", "in").hidden].sort()).toEqual(["a>b", "b>c"]);
    expect([...edgeFocus(g, "a", "out").hidden].sort()).toEqual(["b>c", "c>a"]);
    expect(edgeFocus(g, null, "selection").hidden.size).toBe(3);
    expect(edgeFocus(g, null, "all").litNodes).toBeNull();
  });
});

describe("layouts", () => {
  const g: GGraph = {
    direction: "RIGHT",
    nodes: ["f", "callee", "deeper", "caller", "alone"].map((id) => ({ id, type: "hnode", data: {}, width: 100, height: 40, ports: [] })),
    edges: [
      { id: "1", source: "f", target: "callee", kind: "calls", layout: true },
      { id: "2", source: "callee", target: "deeper", kind: "calls", layout: true },
      { id: "3", source: "caller", target: "f", kind: "calls", layout: true },
    ],
  };

  it("puts callers left of the focus and callees right, one column per step", () => {
    const p = customLayout(g, "layers", "f", 1);
    expect(p.get("caller")!.x).toBeLessThan(p.get("f")!.x);
    expect(p.get("f")!.x).toBeLessThan(p.get("callee")!.x);
    expect(p.get("callee")!.x).toBeLessThan(p.get("deeper")!.x);
    expect(p.get("alone")!.x).toBeGreaterThan(p.get("deeper")!.x);
  });

  it("rings nodes around the focus by distance", () => {
    const p = customLayout(g, "concentric", "f", 1);
    const dist = (id: string) => Math.hypot(p.get(id)!.x, p.get(id)!.y);
    expect(dist("f")).toBe(0);
    expect(dist("callee")).toBeLessThan(dist("deeper"));
    expect(Math.abs(dist("callee") - dist("caller"))).toBeLessThan(1);
  });

  it("places every node for circle and grid", () => {
    expect(customLayout(g, "circle", null, 1).size).toBe(5);
    expect(customLayout(g, "grid", null, 1).size).toBe(5);
  });

  it("uses ELK for grouped graphs and layers when there is a focus", () => {
    expect(resolveLayout("auto", "f", false)).toBe("layers");
    expect(resolveLayout("auto", null, false)).toBe("layered");
    expect(resolveLayout("force", null, false)).toBe("stress");
    expect(resolveLayout("circle", null, true)).toBe("layered");
  });
});

describe("grouping", () => {
  it("nests knowledge nodes in their class and namespace", () => {
    const g = knowledgeGraph(index, null, { kinds: kindsOf(["knowledge"]), confidences: new Set(["Exact", "Inferred", "Unresolved"]), depth: 1 });
    const grouped = groupGraph(g, index, "namespace");
    const byId = new Map(grouped.nodes.map((n) => [n.id, n]));
    expect(byId.get("App.OrderService.Place()")?.parent).toBe("App.OrderService");
    expect(byId.get("App.OrderService")?.parent).toBe("ns:App");
    expect(byId.get("App.OrderService")?.container).toBe(true);
    // Parents come before their children.
    const order = grouped.nodes.map((n) => n.id);
    expect(order.indexOf("ns:App")).toBeLessThan(order.indexOf("App.OrderService"));
    expect(groupGraph(g, index, "class").nodes.find((n) => n.id === "App.OrderService")?.parent).toBeUndefined();
  });
});

describe("symbol details", () => {
  it("reads parameters from signatures", () => {
    expect(paramsOf("public void Place(Order order, int count = 1)")).toEqual({ total: 2, required: 1, variadic: false });
    expect(paramsOf("function f(a: string, b?: number, ...rest: Item[])")).toEqual({ total: 3, required: 1, variadic: true });
    expect(paramsOf("pub fn get(&self, map: HashMap<String, (i32, i32)>) -> i32")).toEqual({ total: 1, required: 1, variadic: false });
    expect(paramsOf("public int Count { get; }")).toBeNull();
    expect(paramsOf("void M(Func<int, bool> f, params string[] xs)")).toEqual({ total: 2, required: 1, variadic: true });
  });

  it("reads visibility", () => {
    expect(visibilityOf(sym("x", "Method", null, { signature: "[HttpGet] public async Task Get()" }))).toBe("public");
    expect(visibilityOf(sym("x", "Function", null, { signature: "pub(crate) fn go()" }))).toBe("pub(crate)");
    expect(visibilityOf(sym("x", "Function", null, { signature: "export function go()" }))).toBe("export");
    expect(visibilityOf(sym("x", "Function", null, { signature: "fn go()" }))).toBeNull();
  });

  it("tells one from many", () => {
    expect(cardinality({ name: "Items", ty: "List<OrderItem>", primary_key: false, foreign_key: false, target: "OrderItem" })).toBe("many");
    expect(cardinality({ name: "Customer", ty: "Customer", primary_key: false, foreign_key: false, target: "Customer" })).toBe("one");
    expect(cardinality({ name: "Id", ty: "int", primary_key: true, foreign_key: false, target: null })).toBeNull();
  });
});

describe("analyses", () => {
  it("measures the impact radius by steps, with the tests that reach it", () => {
    const r = impactRadius(index, "App.OrderService.Place()", 3);
    expect(r.levels[0].sort()).toEqual(["App.OrderTests.Places()", "App.OrdersController.Get()"]);
    expect(r.levels[1]).toEqual(["web/src/api.load"]);
  });

  it("finds tests that call the symbol directly or through others", () => {
    const isTest = (id: string) => id.includes("Tests");
    expect(testsFor(index, "App.OrderService.Place()", isTest).map((t) => [t.test, t.how])).toEqual([["App.OrderTests.Places()", "via"]]);
    const withTests = new StructureIndex({ ...graph, edges: [...graph.edges, edge("App.OrderTests.Places()", "App.Order", "Tests", "Inferred")] });
    const viaController = testsFor(withTests, "App.Order", isTest);
    expect(viaController[0]).toMatchObject({ test: "App.OrderTests.Places()", how: "direct", confidence: "Inferred" });
  });

  it("builds call trees both ways", () => {
    const out = callTree(index, "App.OrdersController.Get()", "out", 2);
    expect(out.children.map((c) => c.id)).toEqual(["App.OrderService.Place()"]);
    expect(out.children[0].children.map((c) => c.id)).toEqual(["App.Lib.Helper()"]);
    const into = callTree(index, "App.OrdersController.Get()", "in", 1);
    expect(into.children.map((c) => [c.id, c.kind, c.confidence])).toEqual([["web/src/api.load", "HttpCalls", "Inferred"]]);
  });

  it("follows a request from the client to the data it writes", () => {
    const knowledge = { routes: [{ handler: "App.OrdersController.Get()", method: "GET", path: "/api/orders", normalized: "/api/orders", framework: "aspnet", span: span("App/Orders.cs") }] };
    const f = fullstackFlow(index, knowledge as never, "App.OrdersController.Get()")!;
    expect(f.clients.map((c) => c.id)).toEqual(["web/src/api.load"]);
    expect(f.routes[0].path).toBe("/api/orders");
    expect(f.writes).toEqual(["App.Order"]);
    // From the client: the flow starts at the handler it requests.
    expect(fullstackFlow(index, knowledge as never, "web/src/api.load")!.handlers).toEqual(["App.OrdersController.Get()"]);
    expect(fullstackFlow(index, knowledge as never, "App.Order")).toBeNull();
  });

  it("shows where thrown errors are caught", () => {
    expect(errorFlows(index, "App.OrderService.Place()")).toEqual([
      { exception: "ext:InvalidOperationException", alsoThrownBy: [], caughtBy: [{ id: "App.OrdersController.Get()", confidence: "Exact", evidence: null }] },
    ]);
  });

  it("lists a file's declarations in order", () => {
    expect(fileSkeleton(index, "App/Orders.cs").map((x) => [x.symbol.name, x.depth])).toContainEqual(["Place", 1]);
  });
});

describe("hierarchy levels", () => {
  const h = new Hierarchy(index);
  it("opens one more level within the cap", () => {
    const first = h.nextLevel(new Set(), 100);
    expect(first.open.length).toBeGreaterThan(0);
    expect(h.nextLevel(new Set(), 1).capped).toBe(true);
  });

  it("opens a whole subtree", () => {
    const root = h.roots.find((r) => r.includes("App"))!;
    const { open } = h.subtree(root, new Set(), 100);
    expect(open).toContain(root);
    expect(open).toContain("App.OrderService");
  });
});
