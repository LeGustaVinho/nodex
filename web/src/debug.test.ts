import { describe, expect, it, vi } from "vitest";

// The helpers are pure; the API module reads the page URL when imported.
vi.mock("./api", () => ({ api: {} }));
import { hasBreakpoint, hiddenFrames, topFrame, useDebug, visibleFrames } from "./debug";
import type { DebugFrame } from "./gen/DebugFrame";

const frame = (id: number, file: string | null): DebugFrame => ({ id, name: `f${id}`, file, line: 1, flow: null, node: null });

describe("debugger helpers", () => {
  it("keeps frames down to the outermost project frame", () => {
    const frames = [frame(0, "a.ts"), frame(1, null), frame(2, "main.ts"), frame(3, null), frame(4, null)];
    expect(visibleFrames(frames, false).map((f) => f.id)).toEqual([0, 1, 2]);
    expect(hiddenFrames(frames)).toBe(2);
    expect(visibleFrames(frames, true)).toHaveLength(5);
  });

  it("shows the top frame when none is in the project", () => {
    const frames = [frame(0, null), frame(1, null)];
    expect(visibleFrames(frames, false).map((f) => f.id)).toEqual([0]);
    expect(visibleFrames([], false)).toEqual([]);
  });

  it("matches breakpoints by file and line", () => {
    const list = [{ file: "src/a.ts", line: 3, verified: true }];
    expect(hasBreakpoint(list, "src/a.ts", 3)).toBe(true);
    expect(hasBreakpoint(list, "src/a.ts", 4)).toBe(false);
    expect(hasBreakpoint(list, "src/b.ts", 3)).toBe(false);
  });
});

describe("selected frame variables", () => {
  const scope = (name: string, value: string) => ({ name: "Local", reference: 1, variables: [{ name, value, ty: null, reference: 0 }] });
  const stopped = {
    reason: "breakpoint",
    description: null,
    thread: 1,
    frames: [frame(0, null), frame(1, "a.ts"), frame(2, "main.ts")],
    scopes: [scope("total", "120")],
  };

  it("starts on the top project frame with the stop's scopes", () => {
    useDebug.getState().setStopped(stopped);
    expect(topFrame(stopped)).toBe(1);
    expect(useDebug.getState().frame).toBe(1);
    expect(useDebug.getState().scopes?.[0].variables[0].value).toBe("120");
  });

  it("drops them for another frame until its own arrive", () => {
    const store = useDebug.getState();
    store.setFrame(2);
    expect(useDebug.getState().scopes).toBeNull();
    store.setFrameScopes(1, [scope("x", "stale")]);
    expect(useDebug.getState().scopes).toBeNull();
    store.setFrameScopes(2, [scope("orders", "Array(3)")]);
    expect(useDebug.getState().scopes?.[0].variables[0].name).toBe("orders");
    store.setFrame(1);
    expect(useDebug.getState().scopes?.[0].variables[0].name).toBe("total");
  });
});
