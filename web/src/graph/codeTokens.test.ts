import { expect, it, vi } from "vitest";
import { codeLanguage, codeSegments, codeTokens } from "./codeTokens";

const editor = vi.hoisted(() => ({
  colorize: vi.fn(async (_text: string, mode: string) => {
    if (mode === "rust") throw new Error("grammar unavailable");
  }),
  tokenize: vi.fn(() => [[]]),
}));
vi.mock("../monaco", () => ({ monaco: { editor } }));

it("selects the JavaScript grammar by extension", () => {
  for (const file of ["a.js", "a.JSX", "a.mjs", "a.cjs"]) expect(codeLanguage("typescript", file)).toBe("javascript");
  expect(codeLanguage("typescript", "a.tsx")).toBe("typescript");
  expect(codeLanguage("typescript", "Component.vue")).toBe("typescript");
  expect(codeLanguage("csharp", "a.cs")).toBe("csharp");
  expect(codeLanguage("rust", "a.rs")).toBe("rust");
});

it("preserves text, line endings and UTF-16 offsets", () => {
  const code = '"🐈<img>" + 10\r\nlet x\r// text\n';
  const tokens = [
    [{ offset: 0, type: "string.ts" }, { offset: 9, type: "" }, { offset: 10, type: "delimiter.ts" }, { offset: 11, type: "" }, { offset: 12, type: "number.ts" }],
    [{ offset: 0, type: "keyword.ts" }, { offset: 3, type: "" }],
    [{ offset: 0, type: "comment.ts" }],
  ];
  const segments = codeSegments(code, tokens);
  expect(segments.map(({ text }) => text).join("")).toBe(code);
  expect(segments.filter(({ color }) => color)).toEqual([
    { text: '"🐈<img>"', color: "string" },
    { text: "+", color: "operator" },
    { text: "10", color: "number" },
    { text: "let", color: "keyword" },
    { text: "// text", color: "comment" },
  ]);
});

it("reuses grammar initialization and evicts old snippets across languages", async () => {
  const first = codeTokens("typescript", "old")!;
  expect(codeTokens("typescript", "old")).toBe(first);
  expect(codeTokens("javascript", "old")).not.toBe(first);
  for (let i = 0; i < 2047; i++) codeTokens("typescript", `snippet ${i}`);
  const replacement = codeTokens("typescript", "old");
  expect(replacement).not.toBe(first);
  await replacement;
  expect(editor.colorize).toHaveBeenCalledTimes(2);
});

it("falls back for failed grammars and snippets over 512 UTF-16 units", async () => {
  expect(codeTokens("csharp", "🐈".repeat(256) + "a")).toBeNull();
  expect(codeTokens("csharp", "")).toBeNull();
  await expect(codeTokens("rust", "let x")).resolves.toBeNull();
});
