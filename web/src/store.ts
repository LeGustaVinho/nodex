import { create } from "zustand";
import type { OpenProgress } from "./api";
import type { Span } from "./gen/Span";
import type { UmlKind } from "./gen/UmlKind";
import type { UmlOptions } from "./gen/UmlOptions";
import type { UmlScope } from "./gen/UmlScope";
import type { StructEdgeKind } from "./gen/StructEdgeKind";
import type { DetailLevel } from "./graph/flowGraph";

/** Which edges the explorer views draw: all, or only those of the selected node. */
export type EdgeMode = "all" | "selection" | "in" | "out";
/**
 * How the Knowledge view arranges its nodes: `layered` follows the direction of the relations,
 * `layers` puts the focus first and every step away from it in the next column.
 */
export type LayoutMode = "auto" | "layered" | "layers" | "force" | "concentric" | "circle" | "grid";
export type Spacing = "compact" | "normal" | "wide";
/** Boxes the Knowledge view draws around its nodes. */
export type GroupBy = "none" | "class" | "file" | "namespace";
/** Lists the Overview can show in full. */
export type OverviewList = "summary" | "entries" | "routes" | "hotspots" | "models" | "tests" | "unresolved";

/** Relations shown by default: everything but imports (too many to read on most projects). */
export const DEFAULT_RELATION_KINDS: StructEdgeKind[] = [
  "Calls", "Raises", "Subscribes",
  "Inherits", "Implements", "Compatible", "Instantiates", "References",
  "HttpCalls", "ResolvesDi", "DependsOn", "Tests", "Throws", "CaughtBy", "Mutates",
];

/** Explorer preferences kept between visits. */
interface ExplorerPrefs {
  edgeMode: EdgeMode;
  layoutMode: LayoutMode;
  spacing: Spacing;
  groupBy: GroupBy;
  relationKinds: StructEdgeKind[];
  confidences: string[];
  includeVendor: boolean;
  treeLinks: boolean;
  legendOpen: boolean;
}

const PREFS_KEY = "nodex.explorer";
const DEFAULT_PREFS: ExplorerPrefs = {
  edgeMode: "all",
  layoutMode: "auto",
  spacing: "normal",
  groupBy: "none",
  relationKinds: DEFAULT_RELATION_KINDS,
  confidences: ["Exact", "Inferred", "Unresolved"],
  includeVendor: false,
  treeLinks: true,
  legendOpen: true,
};

function loadPrefs(): ExplorerPrefs {
  try {
    const saved = JSON.parse(window.localStorage.getItem(PREFS_KEY) ?? "{}") as Partial<ExplorerPrefs>;
    return { ...DEFAULT_PREFS, ...saved };
  } catch {
    return DEFAULT_PREFS;
  }
}

function savePrefs(s: ExplorerPrefs) {
  const prefs: ExplorerPrefs = {
    edgeMode: s.edgeMode,
    layoutMode: s.layoutMode,
    spacing: s.spacing,
    groupBy: s.groupBy,
    relationKinds: s.relationKinds,
    confidences: s.confidences,
    includeVendor: s.includeVendor,
    treeLinks: s.treeLinks,
    legendOpen: s.legendOpen,
  };
  try {
    window.localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // Storage may be unavailable (private window): the choice still applies for this visit.
  }
}

export type View = "overview" | "hierarchy" | "knowledge" | "uml" | "architecture" | "types" | "calls" | "timeline" | "blueprint";

/** What the inspector and code panel show. */
export type Selection =
  | { kind: "symbol"; id: string }
  | { kind: "flow-node"; flow: string; id: string }
  | { kind: "expr"; flow: string; nodeId: string; path: string }
  /** A folder or namespace box of the Hierarchy (no symbol behind it). */
  | { kind: "container"; id: string }
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
  /** Progress of the folder being opened, while `projectLoading` is set. */
  projectProgress: OpenProgress | null;
  setProjectProgress: (progress: OpenProgress | null) => void;
  setProjectLoading: (path: string | null) => void;
  /** Another project was opened: forget everything about the old one. */
  resetForProject: () => void;
  connected: boolean;
  /** The Environment dialog (installed toolchains) is open. */
  environmentOpen: boolean;
  setEnvironmentOpen: (open: boolean) => void;
  /** The Settings dialog is open. */
  settingsOpen: boolean;
  setSettingsOpen: (open: boolean) => void;
  /** Containers open in the Hierarchy view. */
  treeExpanded: Record<string, boolean>;
  /** Relations drawn by the Hierarchy and Knowledge views. */
  relationKinds: StructEdgeKind[];
  /** Box whose place on screen stays put while the Hierarchy re-arranges (the one just toggled). */
  treeAnchor: string | null;
  /** Draw the relations rolled up onto the Hierarchy's boxes. */
  treeLinks: boolean;
  /** Which edges the explorer views draw. */
  edgeMode: EdgeMode;
  layoutMode: LayoutMode;
  spacing: Spacing;
  groupBy: GroupBy;
  /** Show third-party code (ThirdParty/, vendor/, Unity's Assets/Plugins and Packages). */
  includeVendor: boolean;
  legendOpen: boolean;
  /** Symbols the Knowledge view was centered on, oldest first (the last one is the focus). */
  knowledgeHistory: { id: string; label: string }[];
  /** List the Overview shows (`summary` = the cards). */
  overviewList: OverviewList;
  /** Text the palette opens with. */
  paletteQuery: string;
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
  /** Centers the Knowledge view on `symbol` (remembered for Back), or shows the whole project. */
  showKnowledge: (symbol: string | null, label?: string) => void;
  setKnowledgeDepth: (depth: number) => void;
  toggleConfidence: (confidence: string) => void;
  toggleTree: (id: string) => void;
  /** Opens every container in `ids` (revealing an item opens its ancestors). */
  expandTree: (ids: string[], anchor?: string) => void;
  collapseTree: () => void;
  /** Shows every relation of `kinds` when one of them is hidden, else hides them all. */
  toggleRelations: (kinds: StructEdgeKind[]) => void;
  setTreeLinks: (on: boolean) => void;
  setEdgeMode: (mode: EdgeMode) => void;
  setLayoutMode: (mode: LayoutMode) => void;
  setSpacing: (spacing: Spacing) => void;
  setGroupBy: (groupBy: GroupBy) => void;
  setIncludeVendor: (on: boolean) => void;
  setLegendOpen: (open: boolean) => void;
  /** Back to the symbol the Knowledge view showed before (or the whole project). */
  knowledgeBack: () => void;
  /** Goes back to `id` in the Knowledge history (`null` = the whole project). */
  knowledgeCrumb: (id: string | null) => void;
  setOverviewList: (list: OverviewList) => void;
  setPaletteOpen: (open: boolean, query?: string) => void;
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

const prefs = loadPrefs();

export const useUi = create<UiState>((set, get) => {
  /** Changes explorer preferences and remembers them. */
  const pref = (change: (s: UiState) => Partial<UiState>) => {
    set(change);
    savePrefs(get());
  };
  return {
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
  setProjectLoading: (projectLoading) => set({ projectLoading, projectProgress: null }),
  projectProgress: null,
  setProjectProgress: (projectProgress) => set({ projectProgress }),
  resetForProject: () => {
    // Overview first: it shows the project at a glance and stays cheap on large projects.
    writeHash("overview", null);
    set({
      view: "overview",
      flowSymbol: null,
      namespace: null,
      callFocus: null,
      selection: null,
      codeFocus: null,
      expanded: {},
      detailKey: null,
      projectLoading: null,
      projectProgress: null,
      umlScope: { kind: "Project" },
      treeExpanded: {},
      treeAnchor: null,
      knowledgeFocus: null,
      knowledgeHistory: [],
      overviewList: "summary",
    });
  },
  connected: false,
  environmentOpen: false,
  setEnvironmentOpen: (environmentOpen) => set({ environmentOpen }),
  settingsOpen: false,
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  treeExpanded: {},
  ...prefs,
  treeAnchor: null,
  knowledgeHistory: [],
  overviewList: "summary",
  paletteQuery: "",
  paletteOpen: false,
  knowledgeFocus: null,
  knowledgeDepth: 2,
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
  showKnowledge: (knowledgeFocus, label) => {
    writeHash("knowledge", null);
    set((s) => {
      if (!knowledgeFocus) return { knowledgeFocus, view: "knowledge", knowledgeHistory: [] };
      const history = s.knowledgeHistory.filter((h) => h.id !== knowledgeFocus);
      history.push({ id: knowledgeFocus, label: label ?? knowledgeFocus });
      return { knowledgeFocus, view: "knowledge", knowledgeHistory: history.slice(-30) };
    });
  },
  knowledgeBack: () =>
    set((s) => {
      const history = s.knowledgeHistory.slice(0, -1);
      return { knowledgeHistory: history, knowledgeFocus: history[history.length - 1]?.id ?? null };
    }),
  knowledgeCrumb: (id) =>
    set((s) => {
      if (!id) return { knowledgeHistory: [], knowledgeFocus: null };
      const at = s.knowledgeHistory.findIndex((h) => h.id === id);
      return at < 0 ? {} : { knowledgeHistory: s.knowledgeHistory.slice(0, at + 1), knowledgeFocus: id };
    }),
  setOverviewList: (overviewList) => set({ overviewList }),
  setKnowledgeDepth: (knowledgeDepth) => set({ knowledgeDepth }),
  toggleConfidence: (c) => pref((s) => ({ confidences: s.confidences.includes(c) ? s.confidences.filter((x) => x !== c) : [...s.confidences, c] })),
  toggleTree: (id) => set((s) => ({ treeExpanded: { ...s.treeExpanded, [id]: !s.treeExpanded[id] }, treeAnchor: id })),
  expandTree: (ids, anchor) =>
    set((s) => ({ treeExpanded: { ...s.treeExpanded, ...Object.fromEntries(ids.map((id) => [id, true])) }, treeAnchor: anchor ?? null })),
  collapseTree: () => set({ treeExpanded: {}, treeAnchor: null }),
  toggleRelations: (kinds) =>
    pref((s) => {
      const on = kinds.some((k) => !s.relationKinds.includes(k));
      return { relationKinds: on ? [...new Set([...s.relationKinds, ...kinds])] : s.relationKinds.filter((k) => !kinds.includes(k)) };
    }),
  setTreeLinks: (treeLinks) => pref(() => ({ treeLinks })),
  setEdgeMode: (edgeMode) => pref(() => ({ edgeMode })),
  setLayoutMode: (layoutMode) => pref(() => ({ layoutMode })),
  setSpacing: (spacing) => pref(() => ({ spacing })),
  setGroupBy: (groupBy) => pref(() => ({ groupBy })),
  setIncludeVendor: (includeVendor) => pref(() => ({ includeVendor })),
  setLegendOpen: (legendOpen) => pref(() => ({ legendOpen })),
  setPaletteOpen: (paletteOpen, paletteQuery = "") => set({ paletteOpen, paletteQuery }),
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
};
});
