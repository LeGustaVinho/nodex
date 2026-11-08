import { create } from "zustand";
import type { OpenProgress } from "./api";
import type { Span } from "./gen/Span";
import type { UmlKind } from "./gen/UmlKind";
import type { UmlOptions } from "./gen/UmlOptions";
import type { UmlScope } from "./gen/UmlScope";
import type { StructEdgeKind } from "./gen/StructEdgeKind";
import { expansionKey, type DetailLevel } from "./graph/flowGraph";
import { contextKey, toUmlScope, type ContextItem } from "./graph/context";
import type { StructureIndex } from "./graph/structureGraph";

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
  classOpen: Record<string, boolean>;
  toggleClassMember: (id: string, shown: boolean) => void;
  setClassOpen: (ids: readonly string[], open: boolean) => void;
  context: ContextItem[];
  contextKey: string | null;
  contextPickerOpen: boolean;
  setContextPickerOpen: (open: boolean) => void;
  loadContext: (root: string, index: StructureIndex) => void;
  setContext: (items: ContextItem[]) => void;
  addToContext: (item: ContextItem) => void;
  removeFromContext: (item: ContextItem) => void;
  clearContext: () => void;
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
  umlExplicitScope: boolean;
  umlOpts: Partial<UmlOptions>;
  /** Shows a UML diagram, using the project context unless a specific scope is chosen. */
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
  toggleExpanded: (flow: string, nodeId: string, shown: boolean) => void;
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

/** A kind alone inherits the project context. A link with a scope pins it. */
export function parseUmlHash(hash: string): { kind: UmlKind; scope?: UmlScope } | null {
  const m = /^#uml=([A-Za-z]+)(?:&scope=(.+))?$/.exec(hash);
  if (!m) return null;
  try {
    const kind = m[1] as UmlKind;
    return m[2] ? { kind, scope: JSON.parse(decodeURIComponent(m[2])) as UmlScope } : { kind };
  } catch {
    return null;
  }
}

export function umlHash(kind: UmlKind, scope?: UmlScope): string {
  return scope ? `#uml=${kind}&scope=${encodeURIComponent(JSON.stringify(scope))}` : `#uml=${kind}`;
}

function writeHash(view: View, flowSymbol: string | null, uml?: { kind: UmlKind; scope?: UmlScope }) {
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
  classOpen: {},
  toggleClassMember: (id, shown) => set((s) => ({ classOpen: { ...s.classOpen, [id]: !shown } })),
  setClassOpen: (ids, open) => set((s) => ({ classOpen: { ...s.classOpen, ...Object.fromEntries(ids.map((id) => [id, open])) } })),
  context: [],
  contextKey: null,
  contextPickerOpen: false,
  setContextPickerOpen: (contextPickerOpen) => set({ contextPickerOpen }),
  loadContext: (root, index) => {
    const key = `nodex.context:${root}`;
    const state = get();
    const sameProject = state.contextKey === key;
    let items = sameProject ? state.context : [];
    if (!sameProject) {
      try {
        const saved: unknown = JSON.parse(window.localStorage.getItem(key) ?? "[]");
        if (Array.isArray(saved)) items = saved.filter((i) => i &&
          ((i.kind === "folder" && typeof i.path === "string") || (i.kind === "symbol" && typeof i.id === "string")));
      } catch {
        // Storage is optional.
      }
      set({ contextKey: key });
    }
    const valid = items.filter((item) => {
      if (item.kind === "folder" || item.id.startsWith("dir:") || item.id.startsWith("grp:ns:")) return true;
      const symbol = index.byId.get(item.id);
      return symbol && !symbol.external;
    });
    if (!sameProject || valid.length !== items.length) state.setContext(valid);
  },
  setContext: (context) => set((s) => {
    try {
      if (s.contextKey) window.localStorage.setItem(s.contextKey, JSON.stringify(context));
    } catch {
      // The context still applies for this visit.
    }
    const umlScope = s.umlExplicitScope ? s.umlScope : toUmlScope(context);
    return {
      context, umlScope, treeAnchor: null,
      umlOpts: s.umlExplicitScope ? s.umlOpts : { ...s.umlOpts, focus: null },
    };
  }),
  addToContext: (item) => {
    const s = get();
    if (!s.context.some((i) => contextKey(i) === contextKey(item))) s.setContext([...s.context, item]);
  },
  removeFromContext: (item) => get().setContext(get().context.filter((i) => contextKey(i) !== contextKey(item))),
  clearContext: () => get().setContext([]),
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
      classOpen: {},
      context: [],
      contextKey: null,
      contextPickerOpen: false,
      detailKey: null,
      projectLoading: null,
      projectProgress: null,
      umlScope: { kind: "Project" },
      umlExplicitScope: false,
      umlOpts: {},
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
  umlExplicitScope: !!startUml?.scope,
  umlOpts: {},
  showUml: (scope, kind) =>
    set((s) => {
      const umlExplicitScope = !!scope || (!!kind && s.view === "uml" && s.umlExplicitScope);
      const next = { kind: kind ?? s.umlKind, scope: scope ?? (umlExplicitScope ? s.umlScope : toUmlScope(s.context)) };
      writeHash("uml", null, { kind: next.kind, scope: umlExplicitScope ? next.scope : undefined });
      // The function a diagram starts from belongs to the old scope.
      const umlOpts = JSON.stringify(next.scope) !== JSON.stringify(s.umlScope) ? { ...s.umlOpts, focus: null } : s.umlOpts;
      return { view: "uml", umlKind: next.kind, umlScope: next.scope, umlExplicitScope, umlOpts };
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
      writeHash(view, s.flowSymbol, { kind: s.umlKind, scope: s.umlExplicitScope ? s.umlScope : undefined });
      return { view };
    }),
  openFlow: (flowSymbol) => {
    writeHash("blueprint", flowSymbol);
    set({ flowSymbol, view: "blueprint", expanded: {}, classOpen: {} });
  },
  setNamespace: (namespace) => set({ namespace }),
  showCalls: (callFocus) => {
    writeHash("calls", null);
    set({ callFocus, view: "calls" });
  },
  setCallDepth: (callDepth) => set({ callDepth }),
  select: (selection, span) => set(span === undefined ? { selection } : { selection, codeFocus: span }),
  focusCode: (codeFocus) => set({ codeFocus }),
  toggleExpanded: (flow, id, shown) => set((s) => ({ expanded: { ...s.expanded, [expansionKey(flow, id)]: !shown } })),
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
    if (get().detailKey === detailKey) return;
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
