// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { SyntaxSite } from "../gen/SyntaxSite";
import { syntaxId } from "../graph/syntaxSites";
import { useUi } from "../store";
import { SyntaxInspector } from "./SyntaxInspector";

vi.mock("../catalog", () => ({
  useCatalog: () => undefined,
  flavorInfo: () => ({ label: "Optional access", cue: "?.", phase: "Runtime",
    explain: "Skip access when the receiver is missing.", effects: [["null", "undefined"]] }),
}));

it("shows the selected occurrence and withholds its source action when the file revision changes", async () => {
  const file = "src/choice.ts";
  const first = "const emoji = '🚀'; a?.name;";
  const second = "b?.name;";
  const text = first + "\n" + second;
  const encoder = new TextEncoder();
  const source = { text, bytes: encoder.encode(text), revision: "current" };
  const occurrence = (start: number, line: number, column: number): SyntaxSite => ({
    key: "safe-access",
    span: { file, start_byte: start, end_byte: start + 2,
      start_line: line, end_line: line, start_col: column, end_col: column + 2 },
  });
  const a = occurrence(encoder.encode(first.split("?.")[0]).length, 1, first.indexOf("?.") + 1);
  const b = occurrence(encoder.encode(first + "\nb").length, 2, 2);
  const query = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, retry: false } } });
  query.setQueryData(["source", file], source);
  const host = document.createElement("div");
  const root = createRoot(host);
  const before = useUi.getState();
  try {
    await act(async () => {
      root.render(<QueryClientProvider client={query}><SyntaxInspector sites={[b, a]}
        language="typescript" revision={() => source.revision} active={syntaxId(b)} /></QueryClientProvider>);
    });
    expect(host.querySelector("pre")?.textContent).toBe(second);
    expect(host.querySelector(".blueprint-code-target")?.textContent).toBe("?.");
    expect((host.querySelector("select") as HTMLSelectElement).value).toBe(syntaxId(b));
    const show = () => Array.from(host.querySelectorAll("button")).find((button) => button.textContent === "Show in source");
    await act(async () => show()?.click());
    expect(useUi.getState().codeFocus).toEqual(b.span);
    expect(useUi.getState().codeFocusRevision).toBe("current");

    await act(async () => {
      query.setQueryData(["source", file], { ...source, revision: "edited" });
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    expect(host.querySelector("pre")).toBeNull();
    expect(show()).toBeUndefined();
    expect(host.textContent).toContain("Example");
  } finally {
    await act(async () => root.unmount());
    useUi.setState(before);
    query.clear();
  }
});
