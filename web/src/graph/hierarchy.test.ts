import { describe, expect, it } from "vitest";
import type { StructEdge } from "../gen/StructEdge";
import type { StructureGraph } from "../gen/StructureGraph";
import type { Symbol } from "../gen/Symbol";
import { Hierarchy, hierarchyGraph, kindsOf, rollup } from "./hierarchy";
import { knowledgeGraph } from "./knowledgeGraph";
import { rankSymbols } from "./search";
import { StructureIndex } from "./structureGraph";

const span = (file: string) => ({ file, start_byte: 0, end_byte: 1, start_line: 1, start_col: 1, end_line: 1, end_col: 2 });

function sym(id: string, kind: Symbol["kind"], parent: string | null, extra: Partial<Symbol> = {}): Symbol {
  const name = id.replace(/^ns:/, "").split(/[./]/).pop()!.replace(/\(.*\)$/, "");
  return {
    id, kind, name, qualified_name: id.replace(/^ns:/, ""), parent, language: "csharp", origins: [span("A.cs")], flavors: [],
    signature: null, type_text: null, has_flow: kind === "Method", external: false, doc: null, ...extra,
  };
}

const edge = (from: string, to: string, kind: StructEdge["kind"], confidence?: StructEdge["confidence"]): StructEdge => ({ from, to, kind, confidence });

// C#: namespaces App.Services and App.Domain (no `App` declaration), TypeScript modules in folders.
const graph: StructureGraph = {
  symbols: [
    sym("ns:App.Services", "Namespace", null),
    sym("ns:App.Domain", "Namespace", null),
    sym("App.Services.OrderService", "Class", "ns:App.Services"),
    sym("App.Services.OrderService.Ship()", "Method", "App.Services.OrderService"),
    sym("App.Services.OrderService.Find()", "Method", "App.Services.OrderService"),
    sym("App.Domain.Order", "Class", "ns:App.Domain"),
    sym("App.Domain.Order.Total()", "Method", "App.Domain.Order"),
    { ...sym("ns:web/src/api/orders", "Module", null), language: "typescript", origins: [span("web/src/api/orders.ts")] },
    { ...sym("web/src/api/orders.fetchOrders", "Function", "ns:web/src/api/orders"), language: "typescript" },
    { ...sym("ext:Exception", "Class", null), external: true },
  ],
  edges: [
    edge("App.Services.OrderService.Ship()", "App.Domain.Order.Total()", "Calls"),
    edge("App.Services.OrderService.Find()", "App.Domain.Order", "Instantiates"),
    edge("App.Services.OrderService.Ship()", "App.Services.OrderService.Find()", "Calls"),
    edge("web/src/api/orders.fetchOrders", "App.Services.OrderService.Find()", "HttpCalls", "Inferred"),
    edge("App.Services.OrderService.Ship()", "ext:Exception", "Throws"),
    edge("App.Services.OrderService", "App.Services.OrderService.Ship()", "Contains"),
  ],
};

describe("hierarchy", () => {
  const index = new StructureIndex(graph);
  const h = new Hierarchy(index);

  it("groups namespaces under synthetic parents and modules under compacted folders", () => {
    expect(h.get("ns:App.Services")?.parent).toBe("grp:ns:App");
    expect(h.get("grp:ns:App")?.kind).toBe("namespace");
    // `web` → `web/src` → `web/src/api` is one folder chain: compacted into a single box.
    const folder = h.get(h.parentOf("ns:web/src/api/orders")!)!;
    expect(folder.label).toBe("web/src/api");
    expect(folder.parent).toBeNull();
    expect(h.roots).toEqual(["grp:ns:App", folder.id]);
    expect(h.get("ext:Exception")).toBeUndefined();
  });

  it("lists visible items parents first", () => {
    expect(h.visible(new Set()).ids).toEqual(h.roots);
    const open = new Set(["grp:ns:App", "ns:App.Services"]);
    expect(h.visible(open).ids).toEqual(["grp:ns:App", "ns:App.Domain", "ns:App.Services", "App.Services.OrderService", h.roots[1]]);
    expect(h.visible(open, 2)).toEqual({ ids: ["grp:ns:App", "ns:App.Domain"], truncated: true });
  });

  it("rolls relations up to the visible items", () => {
    const kinds = kindsOf(["calls", "types", "knowledge"]);
    // Only the roots: App ↔ web folder, both C# relations stay inside App.
    const top = rollup(h, graph.edges, new Set(h.roots), kinds);
    expect(top).toHaveLength(1);
    expect(top[0]).toMatchObject({ source: h.roots[1], target: "grp:ns:App", count: 1, kind: "HttpCalls", confidence: "Inferred" });

    // Namespaces open: Services → Domain stands for a call and a construction.
    const mid = new Set([...h.roots, "ns:App.Services", "ns:App.Domain"]);
    const services = rollup(h, graph.edges, mid, kinds).find((r) => r.source === "ns:App.Services");
    expect(services).toMatchObject({ target: "ns:App.Domain", count: 2, kinds: { Calls: 1, Instantiates: 1 }, confidence: "Exact" });

    // Relations inside one visible item, and to its own members, disappear; external targets too.
    const deep = new Set([...mid, "App.Services.OrderService", "App.Services.OrderService.Ship()"]);
    const fromClass = rollup(h, graph.edges, deep, kinds).filter((r) => r.source === "App.Services.OrderService");
    expect(fromClass.map((r) => r.target)).toEqual(["ns:App.Domain"]);
  });

  it("drops empty scopes but keeps modules with code of their own", () => {
    const ts = (id: string, hasFlow: boolean): Symbol => ({
      ...sym(id, "Module", null),
      language: "typescript",
      origins: [span(`${id.replace(/^ns:/, "")}.ts`)],
      has_flow: hasFlow,
    });
    const scripts = new Hierarchy(new StructureIndex({ symbols: [ts("ns:server/src/main", true), ts("ns:server/src/types", false)], edges: [] }));
    expect(scripts.get("ns:server/src/main")).toBeDefined();
    expect(scripts.get("ns:server/src/types")).toBeUndefined();
  });

  it("builds nested nodes for open containers", () => {
    const g = hierarchyGraph(h, index, new Set(["grp:ns:App"]), kindsOf(["calls"]));
    const app = g.nodes.find((n) => n.id === "grp:ns:App")!;
    expect(app).toMatchObject({ type: "hgroup", container: true, parent: undefined });
    expect(g.nodes.find((n) => n.id === "ns:App.Services")).toMatchObject({ type: "hnode", parent: "grp:ns:App" });
    expect(g.edges).toEqual([expect.objectContaining({ source: "ns:App.Services", target: "ns:App.Domain", kind: "calls", weight: 1 })]);
  });
});

describe("rankSymbols", () => {
  it("ranks exact, prefix, then substring matches, project symbols first", () => {
    const names = rankSymbols(graph.symbols, "order").map((s) => s.id);
    expect(names[0]).toBe("App.Domain.Order");
    // A name prefix (OrderService) beats a name that only contains the query (fetchOrders).
    expect(names.indexOf("App.Services.OrderService")).toBeLessThan(names.indexOf("web/src/api/orders.fetchOrders"));
    expect(rankSymbols(graph.symbols, "o")).toEqual([]);
    expect(rankSymbols(graph.symbols, "exception")[0].id).toBe("ext:Exception");
  });
});

describe("knowledgeGraph", () => {
  const index = new StructureIndex(graph);
  const all = { kinds: kindsOf(["calls", "types", "knowledge"]), confidences: new Set(["Exact", "Inferred", "Unresolved"] as const), depth: 1 };

  it("maps every knowledge relation when nothing is focused", () => {
    const g = knowledgeGraph(index, null, all);
    expect(g.edges.map((e) => e.kind).sort()).toEqual(["httpcalls", "throws"]);
    expect(g.edges.find((e) => e.kind === "httpcalls")?.confidence).toBe("Inferred");
  });

  it("follows relations around a focus, filtered by confidence and depth", () => {
    const find = "App.Services.OrderService.Find()";
    const one = knowledgeGraph(index, find, all);
    expect(new Set(one.nodes.map((n) => n.id))).toEqual(new Set([find, "App.Domain.Order", "App.Services.OrderService.Ship()", "web/src/api/orders.fetchOrders"]));
    const exactOnly = knowledgeGraph(index, find, { ...all, confidences: new Set(["Exact"] as const) });
    expect(exactOnly.nodes.some((n) => n.id === "web/src/api/orders.fetchOrders")).toBe(false);
    const two = knowledgeGraph(index, find, { ...all, depth: 2 });
    expect(two.nodes.some((n) => n.id === "App.Domain.Order.Total()")).toBe(true);
    // A focused type stands for its members.
    const type = knowledgeGraph(index, "App.Services.OrderService", all);
    expect(type.nodes.some((n) => n.id === "App.Domain.Order")).toBe(true);
  });
});
