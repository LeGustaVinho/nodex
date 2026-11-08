import { create } from "zustand";
import type { Span } from "./gen/Span";
import type { UmlKind } from "./gen/UmlKind";
import type { UmlOptions } from "./gen/UmlOptions";
import type { UmlScope } from "./gen/UmlScope";
import type { DetailLevel } from "./graph/flowGraph";

export type View = "overview" | "hierarchy" | "knowledge" | "uml" | "architecture" | "types" | "calls" | "timeline" | "blueprint";

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
  /** Folder being opened from the Open dialog. */
  projectLoading: string | null;
  setProjectLoading: (path: string | null) => void;
  /** Another project was opened: forget everything about the old one. */
  resetForProject: () => void;
  connected: boolean;
  /** Containers open in the Hierarchy view. */
  treeExpanded: Record<string, boolean>;
  /** Relation groups drawn by the Hierarchy view (`calls`, `types`, `imports`, `knowledge`). */
  relations: string[];
  paletteOpen: boolean;
  /** Symbol the Knowledge view is centered on (null = every knowledge relation). */
  knowledgeFocus: string | null;
  knowledgeDepth: number;
  /** Confidences drawn by the Knowledge view (`Exact`, `Inferred`, `Unresolved`). */
  confidences: string[];
  /** UML view: which diagram, of what, how. */
  umlKind: UmlKind;
  umlScope: UmlScope;
  umlOpts: Partial<UmlOptions>;
  /** Shows a UML diagram; keeps the current kind or scope when not given. */
  showUml: (scope?: UmlScope, kind?: UmlKind) => void;
  setUmlOpts: (opts: Partial<UmlOptions>) => void;
  setView: (v: View) => void;
  showKnowledge: (symbol: string | null) => void;
  setKnowledgeDepth: (depth: number) => void;
  toggleConfidence: (confidence: string) => void;
  toggleTree: (id: string) => void;
  /** Opens every container in `ids` (revealing an item opens its ancestors). */
  expandTree: (ids: string[]) => void;
  collapseTree: () => void;
  toggleRelation: (group: string) => void;
  setPaletteOpen: (open: boolean) => void;
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

/** Deep link: `#uml=<kind>&scope=<json>` opens a UML diagram. */
export function parseUmlHash(hash: string): { kind: UmlKind; scope: UmlScope } | null {
  const m = /^#uml=([A-Za-z]+)(?:&scope=(.+))?$/.exec(hash);
  if (!m) return null;
  try {
    const scope = m[2] ? (JSON.parse(decodeURIComponent(m[2])) as UmlScope) : { kind: "Project" as const };
    return { kind: m[1] as UmlKind, scope };
  } catch {
    return null;
  }
}

export function umlHash(kind: UmlKind, scope: UmlScope): string {
  return `#uml=${kind}&scope=${encodeURIComponent(JSON.stringify(scope))}`;
}

function writeHash(view: View, flowSymbol: string | null, uml?: { kind: UmlKind; scope: UmlScope }) {
  const hash = view === "blueprint" && flowSymbol ? `#flow=${encodeURIComponent(flowSymbol)}` : view === "uml" && uml ? umlHash(uml.kind, uml.scope) : "";
  if (window.location.hash !== hash) window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}${hash}`);
}

const startFlow = initialFlow();
const startUml = parseUmlHash(window.location.hash);

export const useUi = create<UiState>((set) => ({
  view: startFlow ? "blueprint" : startUml ? "uml" : "overview",
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
  projectLoading: null,
  setProjectLoading: (projectLoading) => set({ projectLoading }),
  resetForProject: () => {
    writeHash("architecture", null);
    set({
      view: "architecture",
      flowSymbol: null,
      namespace: null,
      callFocus: null,
      selection: null,
      codeFocus: null,
      expanded: {},
      detailKey: null,
      projectLoading: null,
      umlScope: { kind: "Project" },
    });
  },
  connected: false,
  treeExpanded: {},
  relations: ["calls", "types", "knowledge"],
  paletteOpen: false,
  knowledgeFocus: null,
  knowledgeDepth: 2,
  confidences: ["Exact", "Inferred", "Unresolved"],
  umlKind: startUml?.kind ?? "Class",
  umlScope: startUml?.scope ?? { kind: "Project" },
  umlOpts: {},
  showUml: (scope, kind) =>
    set((s) => {
      const next = { kind: kind ?? s.umlKind, scope: scope ?? s.umlScope };
      writeHash("uml", null, next);
      // The function a diagram starts from belongs to the old scope.
      const umlOpts = scope && JSON.stringify(scope) !== JSON.stringify(s.umlScope) ? { ...s.umlOpts, focus: null } : s.umlOpts;
      return { view: "uml", umlKind: next.kind, umlScope: next.scope, umlOpts };
    }),
  setUmlOpts: (opts) => set((s) => ({ umlOpts: { ...s.umlOpts, ...opts } })),
  showKnowledge: (knowledgeFocus) => {
    writeHash("knowledge", null);
    set({ knowledgeFocus, view: "knowledge" });
  },
  setKnowledgeDepth: (knowledgeDepth) => set({ knowledgeDepth }),
  toggleConfidence: (c) =>
    set((s) => ({ confidences: s.confidences.includes(c) ? s.confidences.filter((x) => x !== c) : [...s.confidences, c] })),
  toggleTree: (id) => set((s) => ({ treeExpanded: { ...s.treeExpanded, [id]: !s.treeExpanded[id] } })),
  expandTree: (ids) => set((s) => ({ treeExpanded: { ...s.treeExpanded, ...Object.fromEntries(ids.map((id) => [id, true])) } })),
  collapseTree: () => set({ treeExpanded: {} }),
  toggleRelation: (group) =>
    set((s) => ({ relations: s.relations.includes(group) ? s.relations.filter((g) => g !== group) : [...s.relations, group] })),
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  setView: (view) =>
    set((s) => {
      writeHash(view, s.flowSymbol, { kind: s.umlKind, scope: s.umlScope });
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
