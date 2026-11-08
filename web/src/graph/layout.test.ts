import { describe, expect, it } from "vitest";
import { layoutSignature, packComponents, placeSideColumns, type Positions } from "./layout";
import type { GGraph, GNode } from "./model";

const node = (id: string, extra: Partial<GNode> = {}): GNode => ({ id, type: "t", data: {}, width: 100, height: 40, ports: [], ...extra });

const graph = (nodes: GNode[], edges: [string, string][] = []): GGraph => ({
  nodes,
  edges: edges.map(([s, t]) => ({ id: `${s}->${t}`, source: s, target: t, kind: "uses", layout: true })),
  direction: "RIGHT",
});

describe("side columns", () => {
  it("sorts cards by their writers' height within the class and leaves room for every card", () => {
    const g = graph([
      node("class", { container: true, width: 380, headerHeight: 46 }),
      node("first", { parent: "class", container: true }),
      node("statement", { parent: "first" }),
      node("second", { parent: "class" }),
      node("unwritten", { parent: "class", layoutSide: "right" }),
      node("late", { parent: "class", layoutSide: "right" }),
      node("early", { parent: "class", layoutSide: "right" }),
    ], [["statement", "early"], ["second", "late"]]);
    g.edges.forEach((e) => { e.layout = false; });
    const positions: Positions = new Map([
      ["class", { x: 20, y: 30, width: 500, height: 400 }],
      ["first", { x: 12, y: 46 }],
      ["statement", { x: 100, y: 80 }],
      ["second", { x: 12, y: 350 }],
    ]);
    placeSideColumns(g, positions);
    expect(positions.get("early")).toEqual({ x: 560, y: 126 });
    expect(positions.get("late")).toEqual({ x: 560, y: 350 });
    expect(positions.get("unwritten")).toEqual({ x: 560, y: 406 });
    expect(positions.get("class")).toEqual({ x: 20, y: 30, width: 672, height: 458 });
    expect(layoutSignature(g)).not.toBe(layoutSignature({ ...g, edges: g.edges.slice(1) }));
  });

  it("lays out fields inside a class with no methods", () => {
    const g = graph([
      node("class", { container: true, width: 380, height: 76, headerHeight: 46 }),
      node("field", { parent: "class", layoutSide: "right" }),
    ]);
    const positions: Positions = new Map([["class", { x: 0, y: 0 }]]);
    placeSideColumns(g, positions);
    expect(positions.get("field")).toEqual({ x: 12, y: 46 });
    expect(positions.get("class")).toEqual({ x: 0, y: 0, width: 380, height: 98 });
  });
});

describe("layoutSignature", () => {
  it("is the same for equal graphs and ignores node data", () => {
    const a = graph([node("a", { data: { x: 1 } }), node("b")], [["a", "b"]]);
    const b = graph([node("a", { data: { x: 2 } }), node("b")], [["a", "b"]]);
    expect(layoutSignature(a)).toBe(layoutSignature(b));
  });

  it("changes with anything that moves nodes", () => {
    const base = graph([node("a"), node("b")], [["a", "b"]]);
    const sig = layoutSignature(base);
    expect(layoutSignature(graph([node("a", { width: 120 }), node("b")], [["a", "b"]]))).not.toBe(sig);
    expect(layoutSignature(graph([node("a"), node("b")], [["b", "a"]]))).not.toBe(sig);
    expect(layoutSignature(graph([node("a"), node("b", { parent: "a" })], [["a", "b"]]))).not.toBe(sig);
    expect(layoutSignature({ ...base, direction: "DOWN" })).not.toBe(sig);
    // Edges left out of the layout do not count.
    const drawnOnly = { ...base, edges: [...base.edges, { id: "x", source: "b", target: "a", kind: "uses", layout: false }] };
    expect(layoutSignature(drawnOnly)).toBe(sig);
  });
});

describe("packComponents", () => {
  it("puts unconnected parts side by side and keeps connected ones together", () => {
    const g = graph([node("a"), node("b"), node("c"), node("d")], [["a", "b"]]);
    const positions: Positions = new Map([
      ["a", { x: 0, y: 0 }],
      ["b", { x: 0, y: 100 }],
      ["c", { x: 0, y: 200 }],
      ["d", { x: 0, y: 300 }],
    ]);
    packComponents(g, positions, 4);
    const a = positions.get("a")!;
    const b = positions.get("b")!;
    // The connected pair keeps its relative placement.
    expect(b.x - a.x).toBe(0);
    expect(b.y - a.y).toBe(100);
    // Everything fits in fewer rows than the single column ELK produced.
    const ys = new Set([...positions.values()].map((p) => p.y));
    expect(Math.max(...ys)).toBeLessThan(300);
  });
});
