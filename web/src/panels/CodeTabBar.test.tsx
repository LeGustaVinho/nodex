// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ContextMenuHost, useContextMenu } from "../components/ContextMenu";
import { showDebugFrame } from "../debug";
import type { ProjectSummary } from "../gen/ProjectSummary";
import type { Symbol } from "../gen/Symbol";
import { StructureIndex } from "../graph/structureGraph";
import { useUi } from "../store";
import { CodeTabBar } from "./CodeTabBar";
import { Sidebar } from "./Sidebar";

let index: StructureIndex;
vi.mock("../views/Views", () => ({ useStructure: () => ({ index }) }));

const span = (file: string, line = 1) => ({
  file, start_byte: 0, end_byte: 1, start_line: line, end_line: line, start_col: 1, end_col: 2,
});
const symbol = (id: string, kind: Symbol["kind"], parent: string | null, file?: string, line?: number): Symbol => ({
  id, kind, parent, name: id.split(".").at(-1)!, qualified_name: id, language: "csharp",
  origins: file ? [span(file, line)] : [], external: false, flavors: [], has_flow: false,
  signature: null, type_text: null, doc: null,
});

const previous = useUi.getState();
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  useUi.getState().resetForProject();
});
afterEach(() => {
  useContextMenu.getState().close();
  useUi.setState(previous);
  window.history.replaceState(null, "", "/");
  vi.unstubAllGlobals();
});

function mount(project?: ProjectSummary) {
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, gcTime: Infinity } } });
  if (project) client.setQueryData(["project"], project);
  const container = document.body.appendChild(document.createElement("div"));
  const root = createRoot(container);
  act(() => root.render(
    <QueryClientProvider client={client}>
      {project && <Sidebar />}
      <CodeTabBar project={project} index={index} />
      <ContextMenuHost projectRoot={project?.root} />
    </QueryClientProvider>,
  ));
  const find = (selector: string, name: string) => {
    const button = [...container.querySelectorAll<HTMLButtonElement>(selector)].find((el) => el.textContent?.trim() === name);
    expect(button, name).toBeDefined();
    return button!;
  };
  return {
    container,
    tab: (name: string) => find("[role=tab]", name),
    click: (name: string) => act(() => find("button", name).click()),
    menu: (name: string) => act(() => find("[role=tab]", name).dispatchEvent(new MouseEvent("contextmenu", { bubbles: true }))),
    cleanup: () => {
      act(() => root.unmount());
      container.remove();
      client.clear();
    },
  };
}

describe("code tabs", () => {
  it("reuses files opened by different entry points and ignores a delayed Blueprint source", () => {
    const ui = useUi.getState();
    ui.openCodeFile("A.cs");
    ui.select({ kind: "symbol", id: "B" }, span("B.cs", 7), "revision-b");
    showDebugFrame({ id: 1, name: "C", file: "C.cs", line: 19, flow: "C", node: null });
    ui.focusCode(span("A.cs", 4));
    expect(useUi.getState()).toMatchObject({
      codeTabs: ["A.cs", "B.cs", "C.cs"], activeCodeFile: "A.cs", codeFocus: { start_line: 4 }, codeFocusRevision: null,
    });

    ui.openFlow("D");
    ui.openCodeFile("B.cs");
    ui.focusBlueprintSource("D", [span("D.cs")]);
    ui.focusCode(null);
    expect(useUi.getState()).toMatchObject({ codeTabs: ["A.cs", "B.cs", "C.cs"], activeCodeFile: "B.cs", codeFocus: null });
    ui.openFlow("D");
    ui.focusBlueprintSource("D", [span("D.cs")]);
    expect(useUi.getState()).toMatchObject({ codeTabs: ["A.cs", "B.cs", "C.cs", "D.cs"], activeCodeFile: "D.cs" });
    ui.resetForProject();
    ui.openCodeFile("B.cs");
    expect(useUi.getState()).toMatchObject({ codeTabs: ["B.cs"], activeCodeFile: "B.cs", codeFocus: null });
  });

  it.each([
    ["Close", "B.cs", ["C.cs", "A.cs"], "C.cs"],
    ["Close Other Tabs", "A.cs", ["A.cs"], "A.cs"],
    ["Close All Tabs", "A.cs", [], null],
    ["Close Unmodified Tabs", "B.cs", [], null],
    ["Close Tabs to the Left", "C.cs", ["C.cs", "A.cs"], "C.cs"],
    ["Close Tabs to the Right", "C.cs", ["B.cs", "C.cs"], "B.cs"],
  ] as const)("%s follows the clicked tab and current order", (command, anchor, remaining, active) => {
    const ui = useUi.getState();
    for (const file of ["A.cs", "B.cs", "C.cs", "B.cs"]) ui.openCodeFile(file);
    const mounted = mount();
    try {
      act(() => mounted.tab("A.cs").dispatchEvent(new KeyboardEvent("keydown", {
        key: "ArrowRight", ctrlKey: true, shiftKey: true, bubbles: true,
      })));
      expect(useUi.getState().codeTabs).toEqual(["B.cs", "A.cs", "C.cs"]);

      const dataTransfer = { setData: vi.fn(), effectAllowed: "" };
      const drag = new MouseEvent("dragstart", { bubbles: true });
      Object.defineProperty(drag, "dataTransfer", { value: dataTransfer });
      const drop = new MouseEvent("drop", { bubbles: true, clientX: 2 });
      act(() => {
        mounted.tab("A.cs").dispatchEvent(drag);
        mounted.tab("C.cs").dispatchEvent(drop);
      });
      expect(useUi.getState().codeTabs).toEqual(["B.cs", "C.cs", "A.cs"]);
      mounted.menu(anchor);
      expect(useUi.getState().activeCodeFile).toBe("B.cs");
      mounted.click(command);
      expect(useUi.getState()).toMatchObject({ codeTabs: remaining, activeCodeFile: active });
    } finally {
      mounted.cleanup();
    }
  });

  it("reveals inactive files and chosen declarations through the virtualized trees", () => {
    const file = "zz/nested/ação teste.cs";
    const others = Array.from({ length: 80 }, (_, i) => `folder${i}/Other.cs`);
    const type = symbol("Root.Target", "Class", "Root", file);
    const first = symbol("Root.Target.First", "Method", type.id, file, 2);
    const second = symbol("Root.Target.Second", "Method", type.id, file, 7);
    index = new StructureIndex({ symbols: [
      ...Array.from({ length: 80 }, (_, i) => symbol(`Namespace${i}`, "Namespace", null)),
      symbol("Root", "Namespace", null), type, first, second,
    ], edges: [] });
    const project: ProjectSummary = {
      open: true, root: "D:\\Test", name: "Test", languages: [], flavors: [], packages: [], semantic: [],
      files: [...others, file].map((path) => ({ path, diagnostics: [], language: "csharp", symbols: 0, flows: 0 })),
    };
    const ui = useUi.getState();
    ui.openCodeFile(file);
    ui.openCodeFile(others[0]);
    const mounted = mount(project);
    try {
      const input = mounted.container.querySelector("input")!;
      const change = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
      act(() => {
        change.call(input, "unrelated");
        input.dispatchEvent(new Event("input", { bubbles: true }));
      });
      expect(input.value).toBe("unrelated");
      mounted.menu("ação teste.cs");
      mounted.click("Select in Files");
      expect(input.value).toBe("");
      expect(document.activeElement?.textContent).toContain("ação teste.cs");
      expect(mounted.container.querySelector(".tree")!.scrollTop).toBeGreaterThan(0);

      mounted.menu("ação teste.cs");
      mounted.click("Select in Symbols ›");
      mounted.click("Root.Target.SecondLine 7");
      expect(document.activeElement?.textContent).toContain("Second");
      expect(document.activeElement?.closest(".active")).not.toBeNull();
      expect(useUi.getState()).toMatchObject({ activeCodeFile: others[0], view: "overview", selection: null });
    } finally {
      mounted.cleanup();
    }
  });
});
