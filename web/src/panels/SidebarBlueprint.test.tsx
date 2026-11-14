// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { Symbol } from "../gen/Symbol";
import { classMembers } from "../graph/classBlueprint";
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
  const file = "creatureLevelUp.ts";
  const module = declaration("ts:creatureLevelUp", "Module", "typescript", file);
  const initializer = {
    ...declaration("creatureLevelUp.(module)", "Function", "typescript", file),
    parent: module.id, has_flow: true, flavors: ["top-level"],
  };
  const handler = {
    ...declaration("creatureLevelUp.(module).defineEndpoint callback", "Function", "typescript", file),
    parent: initializer.id, has_flow: true, flavors: ["initializer-callback", "async"],
  };
  const transaction = {
    ...declaration("runTransaction callback", "Function", "typescript", file), parent: handler.id, has_flow: true,
  };
  const symbols = [
    declaration("CSharpContract", "Interface", "csharp", "CSharpContract.cs"),
    declaration("TypeScriptState", "Union", "typescript", "TypeScriptState.ts"),
    declaration("RustSource", "Trait", "rust", "RustSource.rs"),
    declaration("rustFlows", "Module", "rust", "flows.rs"),
    declaration("tsFlows", "Module", "typescript", "flows.ts"),
    module, initializer, handler, transaction,
  ];
  structure = new StructureIndex({ symbols, edges: [] });
  const second = { ...handler, id: "second callback" };
  const multiple = new StructureIndex({ symbols: [...symbols, second], edges: [] });
  expect(multiple.fileBlueprint(file, true)).toBe(module);
  expect(classMembers(multiple, module.id)).toEqual([initializer, handler, second]);
  const project = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, gcTime: Infinity } } });
  const files = [...new Set(symbols.map((s) => s.origins[0].file))].map((path) => ({ path, diagnostics: [] }));
  project.setQueryData(["project"], { name: "Blueprint demo", files });
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
    click("▤creatureLevelUp.ts");
    expectOpen(handler.id, file);
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
