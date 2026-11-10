// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { fileMenuItems } from "../fileMenu";
import { ContextMenuHost, useContextMenu } from "./ContextMenu";

it("copies either path in the browser and returns focus to Copy path on Escape", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const writeText = vi.fn().mockResolvedValue(undefined);
  vi.stubGlobal("navigator", Object.create(navigator, { clipboard: { value: { writeText } } }));
  const container = document.body.appendChild(document.createElement("div"));
  const root = createRoot(container);
  const open = () => useContextMenu.getState().open(0, 0, fileMenuItems("src/ação teste.cs"));
  const press = (key: string) => act(() => {
    document.activeElement?.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
  });
  const button = (label: string) => [...container.querySelectorAll("button")].find((item) => item.textContent?.startsWith(label))!;

  try {
    act(() => {
      root.render(<ContextMenuHost projectRoot={"D:\\Meu Projeto"} />);
      open();
    });
    expect(button("Open").disabled).toBe(true);
    expect(button("Reveal in File Explorer").disabled).toBe(true);
    expect(document.activeElement).toBe(button("Copy path"));
    press("ArrowRight");
    expect(document.activeElement).toBe(button("Relative"));
    press("Escape");
    expect(document.activeElement).toBe(button("Copy path"));
    press("ArrowRight");
    await act(async () => button("Absolute").click());
    expect(writeText).toHaveBeenLastCalledWith("D:\\Meu Projeto\\src\\ação teste.cs");

    act(open);
    press("ArrowRight");
    await act(async () => button("Relative").click());
    expect(writeText).toHaveBeenLastCalledWith("src/ação teste.cs");
  } finally {
    act(() => {
      useContextMenu.getState().close();
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  }
});
