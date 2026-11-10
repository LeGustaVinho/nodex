// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import type { BorrowOrigins } from "../gen/BorrowOrigins";
import { useUi } from "../store";
import { BorrowInspector } from "./BorrowInspector";

it("returns to the actual preceding call page when cursors have different widths", async () => {
  const query = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, retry: false } } });
  const key = ["borrow-origin", useUi.getState().projectSession, "flow", "node", "var:x", undefined];
  for (const [cursor, next, label] of [[0, 12, "first"], [12, 23, "second"], [23, null, "last"]] as const) {
    query.setQueryData<BorrowOrigins>([...key, cursor], {
      origins: [{ kind: "Local", owner: null, borrowed: label, through: [], call: null }],
      next_offset: next,
      partial: false,
    });
  }
  const host = document.createElement("div");
  const root = createRoot(host);
  const click = async (label: string) => {
    const button = Array.from(host.querySelectorAll("button")).find((item) => item.textContent === label);
    expect(button).toBeDefined();
    await act(async () => button!.click());
  };
  try {
    await act(async () => root.render(<QueryClientProvider client={query}>
      <BorrowInspector flow="flow" node="node" pin="var:x" />
    </QueryClientProvider>));
    await click("More calls");
    expect(host.textContent).toContain("second");
    await click("More calls");
    expect(host.textContent).toContain("last");
    await click("Previous calls");
    expect(host.textContent).toContain("second");
    await click("Previous calls");
    expect(host.textContent).toContain("first");
  } finally {
    await act(async () => root.unmount());
    query.clear();
  }
});
