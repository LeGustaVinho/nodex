// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { parseUmlHash, umlHash } from "../store";
import { fileTree } from "../graph/fileTree";
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
    expect(parseUmlHash("#uml=Class")).toEqual({ kind: "Class", scope: { kind: "Project" } });
    expect(parseUmlHash("#flow=x")).toBeNull();
    expect(parseUmlHash("#uml=Class&scope=%7Bbad")).toBeNull();
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
