// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { parseUmlHash, umlHash, useUi } from "../store";
import { fileTree } from "../graph/fileTree";
import { StructureIndex } from "../graph/structureGraph";
import { elementIdOf, scopeLabel } from "./uml";

describe("UML helpers", () => {
  it("finds element ids inside Mermaid node ids", () => {
    expect(elementIdOf("flowchart-n12-3")).toBe("n12");
    expect(elementIdOf("mermaid-1-classId-n0-0")).toBe("n0");
    expect(elementIdOf("n7")).toBe("n7");
    expect(elementIdOf("g1")).toBeNull();
    expect(elementIdOf("flowchart-nx-1")).toBeNull();
  });

  it("names scopes", () => {
    expect(scopeLabel({ kind: "Project" })).toBe("whole project");
    expect(scopeLabel({ kind: "Folder", path: "src/domain" })).toBe("src/domain/");
    expect(scopeLabel({ kind: "File", path: "src/a/b.ts" })).toBe("b.ts");
    expect(scopeLabel({ kind: "Symbol", id: "dir:Services" })).toBe("Services/");
    expect(scopeLabel({ kind: "Symbol", id: "X.Y" }, () => "Y")).toBe("Y");
  });

  it("round-trips deep links", () => {
    const hash = umlHash("Package", { kind: "Folder", path: "src/a b" });
    expect(parseUmlHash(hash)).toEqual({ kind: "Package", scope: { kind: "Folder", path: "src/a b" } });
    expect(parseUmlHash(umlHash("Class"))).toEqual({ kind: "Class" });
    expect(parseUmlHash("#flow=x")).toBeNull();
    expect(parseUmlHash("#uml=Class&scope=%7Bbad")).toBeNull();
  });

  it("keeps inherited scopes out of the URL and preserves explicit overrides", () => {
    const ui = useUi.getState();
    ui.resetForProject();
    ui.setContext([{ kind: "folder", path: "src" }]);
    ui.setView("uml");
    expect(parseUmlHash(window.location.hash)?.scope).toBeUndefined();
    expect(useUi.getState().umlScope).toEqual({ kind: "Folder", path: "src" });
    ui.clearContext();
    expect(useUi.getState().umlScope).toEqual({ kind: "Project" });

    ui.showUml({ kind: "Folder", path: "other" }, "Package");
    ui.setContext([{ kind: "folder", path: "src" }]);
    expect(parseUmlHash(window.location.hash)?.scope).toEqual({ kind: "Folder", path: "other" });
    expect(useUi.getState().umlScope).toEqual({ kind: "Folder", path: "other" });
    ui.showUml();
    expect(useUi.getState().umlScope).toEqual({ kind: "Folder", path: "src" });
  });

  it("drops stale symbol IDs on restore and after the project is reindexed", () => {
    const ui = useUi.getState();
    ui.resetForProject();
    const root = "/project";
    const key = `nodex.context:${root}`;
    const folder = { kind: "folder", path: "src" } as const;
    const symbol = { kind: "symbol", id: "ClassA" } as const;
    window.localStorage.setItem(key, JSON.stringify([folder, symbol, { kind: "symbol", id: "Removed" }]));
    const index = new StructureIndex({ symbols: [{
      id: "ClassA", name: "ClassA", qualified_name: "ClassA", kind: "Class", language: "csharp",
      parent: null, origins: [], flavors: [], signature: null, type_text: null, doc: null, has_flow: false, external: false,
    }], edges: [] });
    ui.loadContext(root, index);
    expect(useUi.getState().context).toEqual([folder, symbol]);
    ui.loadContext(root, new StructureIndex({ symbols: [], edges: [] }));
    expect(useUi.getState().umlScope).toEqual({ kind: "Folder", path: "src" });
    expect(JSON.parse(window.localStorage.getItem(key)!)).toEqual([folder]);
  });

  it("builds the folder tree of the Files tab", () => {
    const tree = fileTree(["src/a/x.ts", "src/a/y.ts", "src/b.ts", "README.cs", "deep/one/two/z.cs"]);
    expect(tree.map((n) => n.name)).toEqual(["deep/one/two", "src", "README.cs"]);
    const src = tree.find((n) => n.name === "src")!;
    expect(src.files).toBe(3);
    expect(src.children.map((c) => c.name)).toEqual(["a", "b.ts"]);
    expect(tree[0].path).toBe("deep/one/two");
    expect(tree[0].children[0].path).toBe("deep/one/two/z.cs");
  });
});
