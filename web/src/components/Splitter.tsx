import { useRef } from "react";
import { type PanelSize, resetPanelSize, setPanelSize } from "../layout";

type Props = {
  /** The size this splitter changes. */
  size: PanelSize;
  /** "x": a vertical bar that changes a width; "y": a horizontal bar that changes a height. */
  axis: "x" | "y";
  /** The element whose size changes: the splitter's previous or next sibling, or its parent. */
  target: "prev" | "next" | "parent";
  /** 1 when dragging right/down makes the panel bigger, -1 when it makes it smaller. */
  grow: 1 | -1;
  min: number;
  /** Largest size, from the panel's container (measured when the drag starts). */
  max: (container: HTMLElement) => number;
  label: string;
  className?: string;
};

const STEP = 16;

/** A bar the user drags (or moves with the arrow keys) to resize a panel; double-click resets it. */
export function Splitter({ size, axis, target, grow, min, max, label, className }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  const panel = () => {
    const el = ref.current;
    if (!el) return null;
    return target === "parent" ? el.parentElement : target === "prev" ? (el.previousElementSibling as HTMLElement | null) : (el.nextElementSibling as HTMLElement | null);
  };
  const measure = (el: HTMLElement) => (axis === "x" ? el.getBoundingClientRect().width : el.getBoundingClientRect().height);
  const limit = (el: HTMLElement, px: number) => {
    const container = el.parentElement ?? document.body;
    return Math.max(min, Math.min(Math.max(min, max(container)), px));
  };

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const el = panel();
    if (!el) return;
    e.preventDefault();
    const bar = e.currentTarget;
    bar.setPointerCapture(e.pointerId);
    const start = axis === "x" ? e.clientX : e.clientY;
    const from = measure(el);
    let last = from;
    document.body.classList.add(axis === "x" ? "resizing-x" : "resizing-y");
    const move = (ev: PointerEvent) => {
      const delta = (axis === "x" ? ev.clientX : ev.clientY) - start;
      last = limit(el, from + grow * delta);
      setPanelSize(size, last, false);
    };
    const up = () => {
      bar.removeEventListener("pointermove", move);
      bar.removeEventListener("pointerup", up);
      bar.removeEventListener("pointercancel", up);
      document.body.classList.remove("resizing-x", "resizing-y");
      if (last !== from) setPanelSize(size, last);
    };
    bar.addEventListener("pointermove", move);
    bar.addEventListener("pointerup", up);
    bar.addEventListener("pointercancel", up);
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const keys = axis === "x" ? { ArrowLeft: -1, ArrowRight: 1 } : { ArrowUp: -1, ArrowDown: 1 };
    const dir = keys[e.key as keyof typeof keys];
    if (dir === undefined) return;
    const el = panel();
    if (!el) return;
    e.preventDefault();
    setPanelSize(size, limit(el, measure(el) + grow * dir * STEP));
  };

  return (
    <div
      ref={ref}
      className={`splitter splitter-${axis}${className ? ` ${className}` : ""}`}
      role="separator"
      aria-orientation={axis === "x" ? "vertical" : "horizontal"}
      aria-label={label}
      title={`${label} (drag; double-click to reset)`}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      onDoubleClick={() => resetPanelSize(size)}
    />
  );
}
