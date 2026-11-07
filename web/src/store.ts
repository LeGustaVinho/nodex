import { create } from "zustand";
import type { Span } from "./gen/Span";
import type { DetailLevel } from "./graph/flowGraph";

export type View = "architecture" | "types" | "calls" | "timeline" | "blueprint";

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
  /** How much of each statement the Blueprint draws; remembered per project. */
  detail: DetailLevel;
  /** localStorage key for `detail` (per project root). */
  detailKey: string | null;
  connected: boolean;
  setView: (v: View) => void;
  openFlow: (symbol: string) => void;
  setNamespace: (ns: string | null) => void;
  showCalls: (symbol: string) => void;
  setCallDepth: (depth: number) => void;
  select: (s: Selection, span?: Span | null) => void;
  focusCode: (span: Span | null) => void;
  /** Opens or closes a statement's details; `shown` is whether they are shown now. */
  toggleExpanded: (nodeId: string, shown: boolean) => void;
  setDetail: (level: DetailLevel) => void;
  /** Reads the remembered detail level of the project at `root`. */
  loadDetail: (root: string) => void;
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
  detail: "statements",
  detailKey: null,
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
  toggleExpanded: (id, shown) => set((s) => ({ expanded: { ...s.expanded, [id]: !shown } })),
  setDetail: (detail) =>
    set((s) => {
      try {
        if (s.detailKey) window.localStorage.setItem(s.detailKey, detail);
      } catch {
        // Storage may be unavailable (private window); the level still applies for this visit.
      }
      return { detail, expanded: {} };
    }),
  loadDetail: (root) => {
    const detailKey = `nodex.detail:${root}`;
    let detail: DetailLevel = "statements";
    try {
      const saved = window.localStorage.getItem(detailKey);
      if (saved === "operations" || saved === "everything" || saved === "statements") detail = saved;
    } catch {
      // Ignore: default level.
    }
    set({ detailKey, detail });
  },
  collapseAll: () => set({ expanded: {} }),
  setShowDataWires: (showDataWires) => set({ showDataWires }),
  setShowUses: (showUses) => set({ showUses }),
  setConnected: (connected) => set({ connected }),
}));
