// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { refinedProjection } from "./RefinedArchitecture";
import type { Refined, RefinedNode } from "../uml/refinement";

const node = (id: string, parent: string | null): RefinedNode => ({
  id, parent, label: id, kind: "Group", layer: null, declared: false, code: null, evidence: [],
});

describe("refined architecture projection", () => {
  it("drills into groups and rolls relationships onto the nearest visible boundary", () => {
    const diagram: Refined = {
      title: "Demo", nodes: [node("purchase", null), node("catalog", null), node("approval", "purchase"),
        node("receipt", "purchase"), node("orders/a", "approval"), node("payments/b", "approval"), node("products", "catalog")],
      edges: [
        { id: "calls", from: "orders/a", to: "products", label: "Reads", kind: "HTTP", confidence: "Inferred", count: 2, evidence: ["e0"] },
        { id: "checks", from: "orders/a", to: "payments/b", label: "Checks", kind: "Calls", confidence: "Exact", count: 1, evidence: ["e1"] },
        { id: "logs", from: "receipt", to: "orders/a", label: "Logs", kind: "Calls", confidence: "Exact", count: 1, evidence: ["e2"] },
        { id: "unrelated", from: "receipt", to: "products", label: "Writes", kind: "Calls", confidence: "Exact", count: 1, evidence: [] },
      ],
      evidence: [], pending: [], uml: null,
      suggestions: [{ id: 0, error: null, suggestion: { kind: "highlight", target: "checks", text: "Checks payment", members: [], evidence: ["e1"] } }],
    };
    const overview = refinedProjection(diagram, null, false, 60);
    expect(overview.nodes.map((entry) => entry.id)).toEqual(["purchase", "catalog"]);
    expect(overview.featured).toBe(false);
    expect(overview.edges[0].edge).toMatchObject({ from: "purchase", to: "catalog", count: 2, evidence: ["e0"] });
    expect(refinedProjection(diagram, "purchase", true, 60).nodes.map((entry) => entry.id)).toEqual(["approval", "receipt", "catalog"]);
    const detail = refinedProjection(diagram, "approval", false, 60);
    expect(detail.featured).toBe(true);
    expect([...detail.outside]).toEqual(["catalog", "receipt"]);
    expect(detail.pathTo("receipt")).toEqual(["purchase", "receipt"]);
    expect(detail.pathTo("catalog")).toEqual(["catalog"]);
    expect(detail.edges.map(({ edge }) => [edge.from, edge.to])).toEqual([
      ["orders/a", "catalog"],
      ["orders/a", "payments/b"],
      ["receipt", "orders/a"],
    ]);
    expect(detail.edges[0].edge).toMatchObject({ count: 2, evidence: ["e0"] });
    expect(detail.edges[2].edge).toMatchObject({ count: 1, evidence: ["e2"] });
  });
});
