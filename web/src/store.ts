import { create } from "zustand";
import type { Span } from "./gen/Span";

export type View = "architecture" | "types" | "calls" | "blueprint";

/** What the inspector and code panel show. */
export type Selection =
  | { kind: "symbol"; id: string }
  | { kind: "flow-node"; flow: string; id: string }
  | { kind: "expr"; flow: string; nodeId: string; path: string }
  | null;

interface UiState {
  view: View;
  /** Symbol whose Blueprint is shown. */
  flowSymbol: string | null;
  /** Namespace filter for the Types view (null = all). */
  namespace: string | null;
  /** Function (or type) the Calls view is centered on. */
  callFocus: string | null;
  callDepth: number;
  selection: Selection;
  /** Span to reveal in the code panel. */
  codeFocus: Span | null;
  /** Statement nodes whose expression trees are expanded. */
  expanded: Record<string, boolean>;
  showDataWires: boolean;
  showUses: boolean;
  connected: boolean;
  setView: (v: View) => void;
  openFlow: (symbol: string) => void;
  setNamespace: (ns: string | null) => void;
  showCalls: (symbol: string) => void;
  setCallDepth: (depth: number) => void;
  select: (s: Selection, span?: Span | null) => void;
  focusCode: (span: Span | null) => void;
  toggleExpanded: (nodeId: string) => void;
  collapseAll: () => void;
  setShowDataWires: (v: boolean) => void;
  setShowUses: (v: boolean) => void;
  setConnected: (v: boolean) => void;
}

/** Deep link: `#flow=<symbol>` opens a Blueprint directly. */
function initialFlow(): string | null {
  const m = /^#flow=(.+)$/.exec(window.location.hash);
  return m ? decodeURIComponent(m[1]) : null;
}

function writeHash(view: View, flowSymbol: string | null) {
  const hash = view === "blueprint" && flowSymbol ? `#flow=${encodeURIComponent(flowSymbol)}` : "";
  if (window.location.hash !== hash) window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}${hash}`);
}

const startFlow = initialFlow();

export const useUi = create<UiState>((set) => ({
  view: startFlow ? "blueprint" : "architecture",
  flowSymbol: startFlow,
  namespace: null,
  callFocus: null,
  callDepth: 2,
  selection: null,
  codeFocus: null,
  expanded: {},
  showDataWires: true,
  showUses: true,
  connected: false,
  setView: (view) =>
    set((s) => {
      writeHash(view, s.flowSymbol);
      return { view };
    }),
  openFlow: (flowSymbol) => {
    writeHash("blueprint", flowSymbol);
    set({ flowSymbol, view: "blueprint", expanded: {} });
  },
  setNamespace: (namespace) => set({ namespace }),
  showCalls: (callFocus) => {
    writeHash("calls", null);
    set({ callFocus, view: "calls" });
  },
  setCallDepth: (callDepth) => set({ callDepth }),
  select: (selection, span) => set(span === undefined ? { selection } : { selection, codeFocus: span }),
  focusCode: (codeFocus) => set({ codeFocus }),
  toggleExpanded: (id) => set((s) => ({ expanded: { ...s.expanded, [id]: !s.expanded[id] } })),
  collapseAll: () => set({ expanded: {} }),
  setShowDataWires: (showDataWires) => set({ showDataWires }),
  setShowUses: (showUses) => set({ showUses }),
  setConnected: (connected) => set({ connected }),
}));
