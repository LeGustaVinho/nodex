// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { TypeLabel } from "../components/TypeLabel";
import { ValueBadge } from "../components/ValueBadge";
import { useDebug } from "../debug";
import type { TypeInfo } from "../gen/TypeInfo";
import type { Symbol } from "../gen/Symbol";
import { blueprintTypeColor } from "../graph/blueprintTypes";
import { StructureIndex } from "../graph/structureGraph";
import { useUi } from "../store";
import { useTrace } from "../trace";
import { ExecutionTypes, TypeDetails } from "./TypeInspector";

const scalar = (name: string) => ({ kind: "Scalar" as const, name });

it("shows the same structured type next to representation badges and in the inspector", () => {
  const insert = { file: "queue.rs", start_byte: 1, end_byte: 2, start_line: 4, end_line: 4, start_col: 1, end_col: 2 };
  const info: TypeInfo = {
    origin: "Compiler",
    shape: { kind: "Collection", family: "Deque", arguments: [scalar("Int32")] },
    native: "VecDeque<i32>",
    role: { role: "Queue", insert, remove: { ...insert, start_line: 5, end_line: 5 } },
  };
  const label = renderToStaticMarkup(<><TypeLabel info={info} /><ValueBadge info={{ kind: "Value", basis: "TypeAnalysis" }} onInspect={() => {}} /></>);
  expect(label).toContain("Deque&lt;Int32&gt;");
  expect(label).toContain("Queue role");
  expect(label).toContain(">V<");
  const details = renderToStaticMarkup(<TypeDetails info={info} />);
  for (const text of ["Deque&lt;Int32&gt;", "VecDeque&lt;i32&gt;", "Inferred by the compiler", "Insertion · line 4", "Removal · line 5"]) {
    expect(details).toContain(text);
  }
  expect(blueprintTypeColor(info.shape)).not.toBe(blueprintTypeColor({ kind: "Named", name: "Deque", arguments: [] }));
});

it("changes runtime types with replay steps and clears reported types on frame changes and resume", () => {
  const env = globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean };
  const old = env.IS_REACT_ACT_ENVIRONMENT;
  env.IS_REACT_ACT_ENVIRONMENT = true;
  const host = document.createElement("div");
  const root = createRoot(host);
  const trace = useTrace.getState();
  const debug = useDebug.getState();
  const ui = useUi.getState();
  const observation = (native: string, name: string) => ({ name: "x", native, shape: scalar(name), slot: 0, parameter: false });
  const frame = (id: number) => ({ id, name: "example", file: "example.js", line: 1, flow: "example", node: "a" });
  const symbol = (id: string, kind: Symbol["kind"], parent: string | null): Symbol => ({
    id, kind, parent, name: id, qualified_name: id, language: "typescript", origins: [],
    flavors: [], signature: null, type_text: null, has_flow: kind === "Method", external: false, doc: null,
  });
  const index = new StructureIndex({
    symbols: [symbol("Owner", "Class", null), symbol("example", "Method", "Owner")], edges: [],
  });
  try {
    act(() => {
      useUi.setState({ view: "blueprint", flowSymbol: "example", selection: null });
      trace.setReplay({ flow: "example", index: 0, page: { total: 3, offset: 0, steps: [
        { index: 0, micros: 0, thread: 1, node: "a", values: ["x = preview"], observations: [observation("string", "String")] },
        { index: 1, micros: 1, thread: 1, node: "b", values: [], observations: [observation("number", "Number")] },
        { index: 2, micros: 2, thread: 1, node: "c", values: ["x = 42"] },
      ] } });
      debug.setStatus({ state: "paused", detail: "", missing_adapter: null });
      debug.setStopped({
        reason: "breakpoint", description: null, thread: 1, frames: [frame(1), frame(2)],
        scopes: [{ name: "Locals", reference: 1, variables: [
          { name: "top", value: "42", ty: "i32", type_shape: scalar("Int32"), reference: 0 },
          { name: "previewOnly", value: "123", ty: null, reference: 0 },
        ] }],
      });
      root.render(<><TypeDetails /><ExecutionTypes index={index} /></>);
    });
    expect(host.textContent).toContain("Type ?");
    expect(host.textContent).toContain("Observed in Run · Step 1");
    expect(host.textContent).toContain("String");
    expect(host.textContent).toContain("Reported by debugger");
    expect(host.textContent).toContain("Int32");
    expect(host.textContent).not.toContain("previewOnly");
    act(() => useUi.setState({ flowSymbol: "unrelated" }));
    expect(host.textContent).not.toContain("Observed in Run");
    act(() => useUi.setState({ view: "calls", flowSymbol: "example" }));
    expect(host.textContent).not.toContain("Observed in Run");
    act(() => useUi.setState({ view: "blueprint" }));
    expect(host.textContent).toContain("Observed in Run · Step 1");
    act(() => useUi.setState({ flowSymbol: "Owner" }));
    expect(host.textContent).toContain("Observed in Run · Step 1");
    act(() => useUi.setState({ selection: { kind: "flow-node", flow: "example", id: "a" } }));
    expect(host.textContent).not.toContain("Observed in Run");
    expect(host.textContent).toContain("Reported by debugger");
    act(() => useUi.setState({ selection: { kind: "expr", flow: "example", nodeId: "a", path: "0.1" } }));
    expect(host.textContent).not.toContain("Observed in Run");
    expect(host.textContent).toContain("Reported by debugger");
    act(() => {
      useUi.setState({ flowSymbol: "example", selection: null });
      trace.setReplayIndex(1);
      debug.setFrame(2);
    });
    expect(host.textContent).toContain("Number");
    expect(host.textContent).not.toContain("String");
    expect(host.textContent).not.toContain("Int32");
    act(() => {
      trace.setReplayIndex(2);
      debug.setFrame(1);
    });
    expect(host.textContent).not.toContain("Observed in Run");
    expect(host.textContent).toContain("Int32");
    act(() => debug.setStatus({ state: "running", detail: "", missing_adapter: null }));
    expect(host.textContent).not.toContain("Reported by debugger");
  } finally {
    act(() => root.unmount());
    trace.setReplay(null);
    debug.reset();
    useUi.setState({ view: ui.view, flowSymbol: ui.flowSymbol, selection: ui.selection });
    env.IS_REACT_ACT_ENVIRONMENT = old;
  }
});
