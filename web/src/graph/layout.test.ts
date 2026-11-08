import { describe, expect, it } from "vitest";
import { layoutSignature, packComponents, type Positions } from "./layout";
import type { GGraph, GNode } from "./model";

const node = (id: string, extra: Partial<GNode> = {}): GNode => ({ id, type: "t", data: {}, width: 100, height: 40, ports: [], ...extra });

const graph = (nodes: GNode[], edges: [string, string][] = []): GGraph => ({
  nodes,
  edges: edges.map(([s, t]) => ({ id: `${s}->${t}`, source: s, target: t, kind: "uses", layout: true })),
  direction: "RIGHT",
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
