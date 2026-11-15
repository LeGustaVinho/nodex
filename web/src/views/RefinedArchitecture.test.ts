// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { refinedMermaid, refinedProjection } from "./RefinedArchitecture";
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

  it("shares the display limit with outside nodes and expands them on demand", () => {
    const outside = Array.from({ length: 1000 }, (_, i) => node(`external-${i}`, null));
    const diagram: Refined = {
      title: "Demo", nodes: [node("group", null), node("child", "group"), ...outside],
      edges: outside.map(({ id }) => ({
        id, from: "child", to: id, label: "Calls", kind: "Calls", confidence: "Exact", count: 1, evidence: [],
      })),
      suggestions: [{ id: 0, error: null, suggestion: { kind: "highlight", target: "external-999", text: "Important dependency", members: [], evidence: [] } }],
      evidence: [], pending: [], uml: null,
    };
    const limited = refinedProjection(diagram, "group", false, 60);
    expect(limited.nodes).toHaveLength(60);
    expect(limited.edges).toHaveLength(59);
    expect(limited.outside.has("external-999")).toBe(true);
    expect(limited.omitted).toBe(941);
    const expanded = refinedProjection(diagram, "group", false, 120);
    expect(expanded.nodes).toHaveLength(120);
    expect(expanded.edges).toHaveLength(119);
    expect(expanded.omitted).toBe(881);
  });

  it("exports a visible warning and source note for unresolved routes", async () => {
    const diagram: Refined = {
      title: "C4", nodes: [node("web", null), node("replica", null)], edges: [],
      evidence: [{ id: "legacy-association:0", site: null, path: null, text: "Ambiguous HTTP destination GET /api/orders" }],
      pending: ["legacy-association:0"], suggestions: [], uml: null,
    };
    const text = refinedMermaid(diagram, refinedProjection(diagram, null, false, 60));
    expect(text).toContain('pending["Needs confirmation (1)"]');
    expect(text).toContain("%% Ambiguous HTTP destination GET /api/orders");
    expect(text).not.toContain("-.->");
    const { default: mermaid } = await import("mermaid");
    await expect(mermaid.parse(text)).resolves.toMatchObject({ diagramType: "flowchart-v2" });
  });
});
