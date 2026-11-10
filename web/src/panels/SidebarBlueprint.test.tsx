// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { Symbol } from "../gen/Symbol";
import { StructureIndex } from "../graph/structureGraph";
import { useUi } from "../store";
import { Sidebar } from "./Sidebar";

let structure: StructureIndex;
vi.mock("../views/Views", () => ({ useStructure: () => ({ index: structure }) }));

function declaration(id: string, kind: Symbol["kind"], language: Symbol["language"], file: string): Symbol {
  return {
    id, name: id, qualified_name: id, kind, language, parent: null, flavors: [], external: false, has_flow: false,
    signature: null, type_text: null, doc: null,
    origins: [{ file, start_byte: 0, end_byte: 1, start_line: 1, start_col: 1, end_line: 1, end_col: 2 }],
  };
}

it("opens file modules and types from their respective Blueprint entry points", () => {
  const symbols = [
    declaration("CSharpContract", "Interface", "csharp", "CSharpContract.cs"),
    declaration("TypeScriptState", "Union", "typescript", "TypeScriptState.ts"),
    declaration("RustSource", "Trait", "rust", "RustSource.rs"),
    declaration("rustFlows", "Module", "rust", "flows.rs"),
    declaration("tsFlows", "Module", "typescript", "flows.ts"),
  ];
  structure = new StructureIndex({ symbols, edges: [] });
  const project = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, gcTime: Infinity } } });
  project.setQueryData(["project"], { name: "Blueprint demo", files: symbols.map((s) => ({ path: s.origins[0].file, diagnostics: [] })) });
  const host = document.createElement("div");
  const root = createRoot(host);
  const previous = useUi.getState();
  const previousHash = window.location.hash;
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const click = (name: string) => {
    const target = [...host.querySelectorAll("button")].find((button) => button.textContent?.trim() === name);
    expect(target, name).toBeDefined();
    act(() => target!.click());
  };
  const expectOpen = (id: string, file: string) => {
    expect(useUi.getState()).toMatchObject({ view: "blueprint", flowSymbol: id, codeFocus: { file } });
  };
  try {
    useUi.setState({ view: "overview", flowSymbol: null });
    act(() => root.render(<QueryClientProvider client={project}><Sidebar /></QueryClientProvider>));
    click("Files");
    click("▤flows.rs");
    expectOpen("rustFlows", "flows.rs");
    click("▤flows.ts");
    expectOpen("tsFlows", "flows.ts");
    for (const s of symbols.slice(0, 3)) {
      click(`▤${s.origins[0].file}`);
      expectOpen(s.id, s.origins[0].file);
    }
    click("Symbols");
    for (const [i, icon] of ["I", "U", "T"].entries()) {
      click(`${icon}${symbols[i].id}`);
      expectOpen(symbols[i].id, symbols[i].origins[0].file);
    }
  } finally {
    act(() => root.unmount());
    project.clear();
    useUi.setState(previous);
    window.history.replaceState(null, "", window.location.pathname + window.location.search + previousHash);
    vi.unstubAllGlobals();
  }
});
