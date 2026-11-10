// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Symbol } from "./gen/Symbol";
import { blueprintNavigationTarget, navigationMenuItems, useNavigation } from "./navigation";

const span = { file: "A.cs", start_byte: 0, end_byte: 1, start_line: 1, start_col: 1, end_line: 1, end_col: 2 };
const member: Symbol = {
  id: "A.M", kind: "Method", parent: "A", name: "M", qualified_name: "A.M", language: "csharp",
  origins: [span], flavors: [], signature: null, type_text: null, has_flow: false, external: false, doc: null,
};
const fileActions = ["Reveal in File Explorer", "Open", "Copy path"];

describe("navigation menu", () => {
  it("offers implementation navigation for contracts and excludes symbols without source", () => {
    expect(navigationMenuItems(member, { ...member, kind: "Interface" }).map((item) => item.label)).toEqual([
      "Find usages", "Go to declaration", "Go to implementation", ...fileActions,
    ]);
    expect(navigationMenuItems(member, { ...member, kind: "Class" }).map((item) => item.label)).toEqual(["Find usages", "Go to declaration", ...fileActions]);
    expect(navigationMenuItems({ ...member, external: true })).toEqual([]);
    expect(navigationMenuItems({ ...member, origins: [] })).toEqual([]);
    navigationMenuItems(member)[0].run?.(undefined);
    expect(useNavigation.getState().request).toMatchObject({ symbol: member, kind: "usages" });
    useNavigation.getState().close();
  });

  it("offers implementations for object aliases and their members", () => {
    const contract: Symbol = { ...member, kind: "TypeAlias", language: "typescript", flavors: ["object-contract"] };
    const method: Symbol = { ...member, language: "typescript", parent: contract.id };
    const labels = (items: ReturnType<typeof navigationMenuItems>) => items.map((item) => item.label);
    expect(labels(navigationMenuItems(contract))).toContain("Go to implementation");
    expect(labels(navigationMenuItems(method, contract))).toContain("Go to implementation");
    expect(labels(navigationMenuItems({ ...contract, flavors: [] }))).toEqual(["Find usages", "Go to declaration", ...fileActions]);
  });

  it("offers a single entry for each distinct declaration file", () => {
    const symbol = { ...member, origins: [span, { ...span, start_line: 2 }, { ...span, file: "other/A.cs" }] };
    const source = navigationMenuItems(symbol).find((item) => item.label === "Source file");
    expect(source?.children?.map((item) => item.label)).toEqual(["A.cs", "other/A.cs"]);
    expect(source?.children?.[1].children?.map((item) => item.label)).toEqual(fileActions);
  });

  it("offers implementations for an associated Rust type declared by a trait", () => {
    const trait: Symbol = { ...member, kind: "Trait", language: "rust" };
    const associated: Symbol = { ...member, kind: "TypeAlias", language: "rust", flavors: ["associated-type"] };
    expect(navigationMenuItems(associated, trait).map((item) => item.label)).toContain("Go to implementation");
    expect(navigationMenuItems(associated, { ...trait, kind: "Struct" }).map((item) => item.label)).not.toContain("Go to implementation");
  });

  it("offers implementations for public static TypeScript methods", () => {
    const owner: Symbol = { ...member, kind: "Class", language: "typescript" };
    const method: Symbol = { ...member, language: "typescript", flavors: ["static"] };
    expect(navigationMenuItems(method, owner).map((item) => item.label)).toContain("Go to implementation");
    expect(navigationMenuItems({ ...method, flavors: ["static", "private"] }, owner).map((item) => item.label))
      .not.toContain("Go to implementation");
  });

  it("resolves entry nodes and unique targets, and leaves ambiguous statements to their expressions", () => {
    const statement = (concept: string, targets: (string | null)[], root?: string) => ({
      type: "blueprint",
      data: { flow: "Owner", node: { concept, calls: targets.map((symbol) => ({ symbol })), expr: root ? { symbol: root } : null } },
    });
    expect(blueprintNavigationTarget(statement("Entry", []))).toBe("Owner");
    expect(blueprintNavigationTarget(statement("Assign", ["A.M", "A.M"]))).toBe("A.M");
    expect(blueprintNavigationTarget(statement("Assign", ["A.M", "B.M"]))).toBeUndefined();
    expect(blueprintNavigationTarget(statement("Assign", ["A.M", null]))).toBeUndefined();
    expect(blueprintNavigationTarget(statement("Call", ["A.M", null], "A.M"))).toBe("A.M");
    expect(blueprintNavigationTarget(statement("Call", ["A.M", "B.M"], "B.M"))).toBe("B.M");
    expect(blueprintNavigationTarget({ type: "expr", data: { expr: { symbol: "A.M" } } })).toBe("A.M");
  });
});
