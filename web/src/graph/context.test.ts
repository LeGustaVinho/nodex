import { describe, expect, it } from "vitest";
import type { FileSummary } from "../gen/FileSummary";
import type { Symbol } from "../gen/Symbol";
import { contextFiles, contextLabel, contextSymbols, toUmlScope } from "./context";
import { Hierarchy } from "./hierarchy";
import { StructureIndex } from "./structureGraph";

const origin = (file: string) => ({ file, start_byte: 0, end_byte: 1, start_line: 1, start_col: 1, end_line: 1, end_col: 2 });
const symbol = (id: string, kind: Symbol["kind"], file: string, parent: string | null = null, qualified_name = id): Symbol => ({
  id, kind, name: id, qualified_name, parent, language: "typescript", origins: [origin(file)], flavors: [],
  signature: null, type_text: null, has_flow: kind === "Function", external: false, doc: null,
});
const index = new StructureIndex({
  symbols: [
    symbol("module", "Module", "web/src/api/orders.ts", null, "web/src/api/orders"),
    symbol("fetch", "Function", "web/src/api/orders.ts", "module"),
    symbol("ns", "Namespace", "backend/A.cs", null, "Shop.Services"),
    symbol("OrderService", "Class", "backend/A.cs", "ns"),
    symbol("Save", "Method", "backend/A.cs", "OrderService"),
    symbol("other", "Namespace", "backend-old/B.cs", null, "Shopper"),
    { ...symbol("external", "Class", "web/ext.ts"), external: true },
  ],
  edges: [],
});
const file = (path: string): FileSummary => ({ path, language: "typescript", diagnostics: [], flows: 0, symbols: 0 });

describe("project context", () => {
  it("unions folders and symbol descendants, excluding external symbols and sibling folder names", () => {
    expect(contextSymbols(index, [])).toBeNull();
    expect(contextSymbols(index, [{ kind: "folder", path: "backend/" }, { kind: "symbol", id: "module" }]))
      .toEqual(new Set(["ns", "OrderService", "Save", "module", "fetch"]));
    expect(contextSymbols(index, [{ kind: "symbol", id: "OrderService" }])).toEqual(new Set(["OrderService", "Save"]));
  });

  it("resolves synthetic namespace and directory IDs", () => {
    expect(contextSymbols(index, [{ kind: "symbol", id: "grp:ns:Shop" }])).toEqual(new Set(["ns", "OrderService", "Save"]));
    expect(contextSymbols(index, [{ kind: "symbol", id: "dir:web/src" }])).toEqual(new Set(["module", "fetch"]));
  });

  it("uses the compacted folder as a hierarchy root when its ancestor folder is selected", () => {
    const hierarchy = new Hierarchy(index);
    const covered = contextSymbols(index, [{ kind: "folder", path: "web" }])!;
    const roots = hierarchy.scopedRoots(covered);
    expect(roots).toEqual(["dir:web/src/api"]);
    expect(hierarchy.visible(new Set(), 4, undefined, roots).ids).toEqual(roots);
    expect(hierarchy.nextLevel(new Set(), 2, undefined, roots)).toEqual({ open: roots, capped: false });
  });

  it("includes files without declarations in a folder and origin files for symbol selections", () => {
    const files = ["web/src/api/orders.ts", "web/empty.ts", "backend/A.cs", "backend-old/B.cs"].map(file);
    expect(contextFiles(files, [{ kind: "folder", path: "web" }, { kind: "symbol", id: "Save" }], index).map((f) => f.path))
      .toEqual(["web/src/api/orders.ts", "web/empty.ts", "backend/A.cs"]);
  });

  it("labels items and converts them into UML scopes", () => {
    const folder = { kind: "folder", path: "web" } as const;
    const selected = { kind: "symbol", id: "ns" } as const;
    expect(contextLabel([])).toBe("Whole project");
    expect(contextLabel([folder])).toBe("web/");
    expect(contextLabel([selected], () => "Shop.Services")).toBe("Shop.Services");
    expect(contextLabel([folder, selected])).toBe("2 items");
    expect(toUmlScope([])).toEqual({ kind: "Project" });
    expect(toUmlScope([folder])).toEqual({ kind: "Folder", path: "web" });
    expect(toUmlScope([selected])).toEqual({ kind: "Symbols", ids: ["ns"] });
    expect(toUmlScope([folder, selected])).toEqual({ kind: "Symbols", ids: ["dir:web", "ns"] });
  });
});
