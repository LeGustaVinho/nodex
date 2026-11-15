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
      ],
      evidence: [], pending: [], suggestions: [], uml: null,
    };
    const overview = refinedProjection(diagram, null, true, 60);
    expect(overview.nodes.map((entry) => entry.id)).toEqual(["purchase", "catalog"]);
    expect(overview.edges[0].edge).toMatchObject({ from: "purchase", to: "catalog", count: 2, evidence: ["e0"] });
    expect(refinedProjection(diagram, "purchase", true, 60).nodes.map((entry) => entry.id)).toEqual(["approval", "receipt"]);
    const detail = refinedProjection(diagram, "approval", true, 60);
    expect(detail.nodes.map((entry) => entry.id)).toEqual(["orders/a", "payments/b"]);
    expect(detail.edges[0].edge).toMatchObject({ from: "orders/a", to: "payments/b" });
  });
});
