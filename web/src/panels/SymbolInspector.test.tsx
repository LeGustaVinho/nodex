// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { Symbol } from "../gen/Symbol";
import { StructureIndex } from "../graph/structureGraph";
import { useUi } from "../store";
import { SymbolInspector } from "./SymbolInspector";

vi.mock("../views/Views", () => ({ useStructure: () => ({}) }));

it("keeps the Rust contract and associated type explanation visible across Inspector tabs", async () => {
  const span = { file: "source.rs", start_byte: 5, end_byte: 16, start_line: 1, end_line: 1, start_col: 6, end_col: 17 };
  const trait: Symbol = {
    id: "Source", name: "Source", qualified_name: "Source", kind: "Trait", language: "rust", parent: null,
    origins: [span], flavors: [], external: false, has_flow: false, signature: null, type_text: null, doc: null,
  };
  const member: Symbol = {
    ...trait, id: "Source::Item", name: "Item", qualified_name: "Source::Item", kind: "TypeAlias", parent: trait.id,
    flavors: ["associated-type"], signature: "type Item: Debug",
    rust_contract: [{ group: "Requires", text: "Item must meet Debug", span }],
  };
  const owner: Symbol = { ...trait, id: "Stream", name: "Stream", kind: "Struct" };
  const implementation = { ...member, id: "Stream::Item", parent: owner.id };
  const index = new StructureIndex({
    symbols: [trait, member, owner, implementation], edges: [], source_revisions: { [span.file]: "source-revision" },
  });
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity } } });
  client.setQueryData(["project"], { root: "", flavors: [] });
  client.setQueryData(["knowledge"], { symbols: [], routes: [] });
  client.setQueryData(["catalog"], { flavors: {} });
  const host = document.createElement("div");
  const root = createRoot(host);
  const previous = useUi.getState();
  const tabPreference = window.localStorage.getItem("nodex.inspectorTab");
  const reading = () => host.querySelector('section[aria-label="Read this Rust declaration"]');
  const button = (name: string) => [...host.querySelectorAll("button")].find((entry) => entry.textContent === name);
  const render = (symbol: Symbol) => root.render(
    <QueryClientProvider client={client}><SymbolInspector symbol={symbol} index={index} /></QueryClientProvider>,
  );
  try {
    window.localStorage.setItem("nodex.inspectorTab", "evidence");
    await act(async () => render(member));
    expect(host.querySelector('[role="tab"][aria-selected="true"]')?.textContent).toContain("Evidence");
    expect(reading()?.textContent).toContain("Item must meet Debug");
    expect(reading()?.textContent).toContain("Each implementation of the trait specifies which type Item refers to.");
    await act(async () => button("Item must meet Debug")!.click());
    expect(useUi.getState().codeFocus).toEqual(span);
    expect(useUi.getState().codeFocusRevision).toBe("source-revision");
    await act(async () => button("Overview")!.click());
    expect(host.querySelectorAll('[aria-label="Rust contract"]')).toHaveLength(1);
    await act(async () => render(implementation));
    expect(reading()?.textContent).not.toContain("Each implementation of the trait");
  } finally {
    await act(async () => root.unmount());
    useUi.setState(previous);
    client.clear();
    if (tabPreference === null) window.localStorage.removeItem("nodex.inspectorTab");
    else window.localStorage.setItem("nodex.inspectorTab", tabPreference);
  }
});
