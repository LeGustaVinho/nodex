/**
 * Inspector of a symbol, with CodeGraph's tabs: Overview (properties, knowledge, members),
 * Evidence (every relation in and out, by kind, with its confidence and where it shows), Flows
 * (full-stack flow, call hierarchy, error flows), Data (model fields or type contract) and
 * Actions (copy, impact radius, tests, file skeleton, open in an editor). Also the inspector of a
 * folder or namespace box of the Hierarchy.
 */
import { useQuery } from "@tanstack/react-query";
import { type ReactNode, useMemo, useState } from "react";
import { api } from "../api";
import { ValueExplanation } from "../components/ValueBadge";
import { RustContract } from "../components/RustContract";
import { TypeDetails } from "./TypeInspector";
import { SyntaxInspector } from "./SyntaxInspector";
import { flavorInfo, useCatalog } from "../catalog";
import { declarationDetailFlavors } from "../graph/blueprintExplanation";
import { openMenu } from "../components/ContextMenu";
import { ICON, kindLabel } from "../components/HierarchyNodes";
import type { Confidence } from "../gen/Confidence";
import type { StructEdge } from "../gen/StructEdge";
import type { StructEdgeKind } from "../gen/StructEdgeKind";
import type { Symbol } from "../gen/Symbol";
import {
  type TreeNode,
  callTree,
  cardinality,
  complexityLevel,
  coordsOf,
  errorFlows,
  fullstackFlow,
  impactRadius,
  isVendorPath,
  isVendorSymbol,
  linesOf,
  paramsOf,
  shortName,
  skeletonText,
  testsFor,
  typeContract,
  visibilityOf,
} from "../graph/explore";
import { RELATION_LABEL } from "../graph/hierarchy";
import { type StructureIndex, isTypeKind } from "../graph/structureGraph";
import { useUi } from "../store";
import { useToasts } from "../toast";
import { umlMenuItems } from "../uml/menu";
import { contextMenuItems } from "../uml/contextMenu";
import { ConfidenceBadge, relationColor, useContextSet, useUnity } from "../views/Explorer";
import { useStructure } from "../views/Views";
import { openSubtree, revealInHierarchy, useHierarchy, useKnowledge } from "../views/hierarchyAccess";

type Tab = "overview" | "evidence" | "flows" | "data" | "actions";
const TABS: { id: Tab; label: string; hint: string }[] = [
  { id: "overview", label: "Overview", hint: "What it is: signature, properties, knowledge, members" },
  { id: "evidence", label: "Evidence", hint: "Every relation in and out, with how sure Nodex is and where it shows" },
  { id: "flows", label: "Flows", hint: "Full-stack flow, callers and callees, where its errors go" },
  { id: "data", label: "Data", hint: "Fields of a data model, or the contract of a type" },
  { id: "actions", label: "Actions", hint: "Copy, impact radius, tests, file skeleton, open in an editor" },
];

const VERB: Partial<Record<StructEdgeKind, [string, string]>> = {
  HttpCalls: ["sends HTTP requests to", "receives HTTP requests from"],
  ResolvesDi: ["is provided by", "provides"],
  DependsOn: ["is injected with", "is injected into"],
  Tests: ["tests", "is tested by"],
  Throws: ["throws", "is thrown by"],
  CaughtBy: ["is caught by", "catches errors of"],
  Mutates: ["changes", "is changed by"],
  Calls: ["calls", "is called by"],
  Inherits: ["extends", "is extended by"],
  Implements: ["implements", "is implemented by"],
  Instantiates: ["creates", "is created by"],
  References: ["uses", "is used by"],
  Imports: ["imports", "is imported by"],
  Raises: ["raises", "is raised by"],
  Subscribes: ["notifies", "is notified by"],
  Compatible: ["fits", "is fitted by"],
};

function loadTab(): Tab {
  try {
    const t = window.localStorage.getItem("nodex.inspectorTab");
    return TABS.some((x) => x.id === t) ? (t as Tab) : "overview";
  } catch {
    return "overview";
  }
}

async function copy(text: string, what: string) {
  try {
    await navigator.clipboard.writeText(text);
    useToasts.getState().push("info", `Copied ${what}.`);
  } catch {
    useToasts.getState().push("error", "The clipboard is not available here.");
  }
}

/** `vscode://file/...` link that opens the symbol in VS Code. */
function editorUrl(root: string, s: Symbol): string | null {
  const o = s.origins[0];
  if (!o || !root) return null;
  return `vscode://file/${root.replace(/\\/g, "/").replace(/\/$/, "")}/${o.file}:${o.start_line}:${o.start_col}`;
}

/** A link that selects (or reveals) another symbol. */
function SymbolLink({ id, index, children }: { id: string; index: StructureIndex; children?: ReactNode }) {
  const select = useUi((s) => s.select);
  const s = index.byId.get(id);
  if (!s) return <span className="muted">? {id.replace(/^ext:/, "")}</span>;
  return (
    <button className="link" title={`${s.qualified_name}${s.external ? " (outside the project)" : ""}`} onClick={() => select({ kind: "symbol", id }, s.origins[0] ?? null)}>
      {children ?? shortName(index, id)}
    </button>
  );
}

export function SymbolInspector({ symbol, index, syntax, embedded }: { symbol: Symbol; index: StructureIndex; syntax?: string; embedded?: boolean }) {
  const [tab, setTabState] = useState<Tab>(loadTab);
  const setTab = (t: Tab) => {
    setTabState(t);
    try {
      window.localStorage.setItem("nodex.inspectorTab", t);
    } catch {
      // Remembered for this visit only.
    }
  };
  const knowledge = useKnowledge();
  const catalog = useCatalog();
  const details = declarationDetailFlavors(symbol).flatMap((flavor) => flavorInfo(catalog, flavor, symbol.language) ?? []);
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  const h = useHierarchy();
  const unity = useUnity();
  const ui = useUi.getState();
  const info = knowledge.data?.symbols.find((s) => s.symbol === symbol.id);
  const edges = useMemo(() => index.graph.edges.filter((e) => e.kind !== "Contains" && (e.from === symbol.id || e.to === symbol.id)), [index, symbol.id]);
  const vendor = !symbol.external && isVendorSymbol(symbol, unity);
  const url = editorUrl(project.data?.root ?? "", symbol);
  const coords = coordsOf(symbol);
  const revision = (file: string) => index.graph.source_revisions?.[file];
  const parent = symbol.parent ? index.byId.get(symbol.parent) : undefined;
  const associatedType = symbol.language === "rust" && symbol.kind === "TypeAlias"
    && parent?.kind === "Trait" && symbol.flavors.includes("associated-type");

  return (
    <div className={embedded ? undefined : "inspector"}>
      <div className="insp-head">
        <span className="insp-concept">{symbol.name}</span>
        <span className="muted">
          {symbol.external ? "external " : ""}
          {symbol.kind}
        </span>
      </div>
      <div className="badges-row">
        {info?.entry && (
          <span className="badge entry" title="Runs without being called by project code">
            entry · {info.entry.label}
          </span>
        )}
        {info?.test && <span className="badge">test</span>}
        {vendor && (
          <span className="badge" title="ThirdParty/, vendor/, External/ (or Unity's Assets/Plugins and Packages)">
            third-party
          </span>
        )}
      </div>
      {symbol.qualified_name !== symbol.name && <div className="insp-sub">{symbol.qualified_name}</div>}
      {!symbol.external && symbol.origins[0] && (
        <div className="coord-row">
          <button className="link mono" onClick={() => useUi.getState().focusCode(symbol.origins[0])} title="Show the code">
            {coords}
          </button>
          <button className="icon-button sm" onClick={() => void copy(coords, coords)} title="Copy file:line" aria-label="Copy file:line">
            ⧉
          </button>
        </div>
      )}
      <div className="button-row">
        {!symbol.external && (symbol.has_flow || isTypeKind(symbol.kind)) && (
          <button className="primary" onClick={() => ui.openFlow(symbol.id)}>
            {isTypeKind(symbol.kind) ? "Open as Blueprint" : "Open Blueprint"}
          </button>
        )}
        {symbol.has_flow && <button onClick={() => ui.showCalls(symbol.id)}>Show calls</button>}
        {!symbol.external && <button onClick={(e) => openMenu(e, contextMenuItems({ kind: "symbol", id: symbol.id }))}>Context ▾</button>}
        {!symbol.external && (
          <button onClick={() => ui.showKnowledge(symbol.id, shortName(index, symbol.id))} title="HTTP calls, DI, tests, throws and writes around it">
            Knowledge
          </button>
        )}
        {!symbol.external && h && (
          <button onClick={() => revealInHierarchy(h, index, symbol.id)} title="Open its boxes in the Hierarchy and select it">
            Hierarchy
          </button>
        )}
        {!symbol.external && (
          <button onClick={(e) => openMenu(e, umlMenuItems({ kind: "Symbol", id: symbol.id }, `UML of ${symbol.name}`))} title="UML diagrams of this symbol">
            UML ▾
          </button>
        )}
        {url && (
          <a className="btn" href={url} title="Open in VS Code">
            Editor ↗
          </a>
        )}
      </div>
      {(symbol.type_info || symbol.value_info || symbol.kind === "Field" || symbol.kind === "Property") && <div className="insp-section">
        <TypeDetails info={symbol.type_info} native={symbol.type_text} />
        <ValueExplanation info={symbol.value_info} language={symbol.language} />
      </div>}
      {!!symbol.syntax_sites?.length && <SyntaxInspector key={syntax ?? symbol.id} sites={symbol.syntax_sites} language={symbol.language}
        revision={revision} active={syntax} />}
      {(!!symbol.rust_contract?.length || associatedType) && <section className="insp-section" aria-label="Read this Rust declaration">
        <div className="section-title">Read this Rust declaration</div>
        {associatedType && <p className="explain">Each implementation of the trait specifies which type <code>{symbol.name}</code> refers to.</p>}
        {!!symbol.rust_contract?.length && <RustContract items={symbol.rust_contract} inspect={(span) => {
          const expected = revision(span.file);
          if (expected) ui.focusCode(span, expected);
        }} />}
      </section>}
      {details.length > 0 && <div className="insp-section">
        <div className="section-title">Language details</div>
        {details.map((detail) => <div className="flavor" key={detail.label}>
          <span className="badge">{detail.label}</span><span className="flavor-explain">{detail.explain}</span>
        </div>)}
      </div>}
      <nav className="insp-tabs" role="tablist">
        {TABS.map((t) => (
          <button key={t.id} role="tab" aria-selected={tab === t.id} className={tab === t.id ? "on" : ""} title={t.hint} onClick={() => setTab(t.id)}>
            {t.label}
            {t.id === "evidence" && <span className="count">{edges.length}</span>}
          </button>
        ))}
      </nav>
      {tab === "overview" && <OverviewTab symbol={symbol} index={index} edges={edges} />}
      {tab === "evidence" && <EvidenceTab symbol={symbol} index={index} edges={edges} />}
      {tab === "flows" && <FlowsTab symbol={symbol} index={index} />}
      {tab === "data" && <DataTab symbol={symbol} index={index} />}
      {tab === "actions" && <ActionsTab symbol={symbol} index={index} url={url} />}
    </div>
  );
}

// ---------------------------------------------------------------------------- overview

function OverviewTab({ symbol, index, edges }: { symbol: Symbol; index: StructureIndex; edges: StructEdge[] }) {
  const knowledge = useKnowledge();
  const focusCode = useUi((s) => s.focusCode);
  const info = knowledge.data?.symbols.find((s) => s.symbol === symbol.id);
  const routes = knowledge.data?.routes.filter((r) => r.handler === symbol.id) ?? [];
  const lines = linesOf(symbol);
  const params = paramsOf(symbol.signature);
  const visibility = visibilityOf(symbol);
  const parent = symbol.parent ? index.byId.get(symbol.parent) : undefined;
  const bases = edges.filter((e) => e.from === symbol.id && (e.kind === "Inherits" || e.kind === "Implements"));
  const mutations = edges.filter((e) => e.from === symbol.id && e.kind === "Mutates");
  const members = index.children.get(symbol.id) ?? [];
  const cc = info?.complexity ?? 0;
  return (
    <div className="insp-pane">
      {symbol.signature && <pre className="code-snippet">{symbol.signature}</pre>}
      {symbol.doc && <p className="doc">{symbol.doc}</p>}
      <div className="props">
        {lines && (
          <>
            <span className="k">lines</span>
            <span className="mono">
              {lines.start}–{lines.end} <span className="muted">({lines.end - lines.start + 1} lines)</span>
            </span>
            <span className="k">file</span>
            <span className="mono">{lines.file}</span>
          </>
        )}
        <span className="k">language</span>
        <span>{symbol.language}</span>
        {cc > 0 && (
          <>
            <span className="k">complexity</span>
            <span className={`meter meter-${complexityLevel(cc)}`} title="Cyclomatic complexity: 1 + the decisions it makes">
              <b>{cc}</b>
              <span className="meter-bar">
                <span style={{ width: `${Math.min(100, cc * 7)}%` }} />
              </span>
              <span className="muted">{complexityLevel(cc)}</span>
            </span>
          </>
        )}
        {visibility && (
          <>
            <span className="k">visibility</span>
            <span className="mono">{visibility}</span>
          </>
        )}
        {params && (
          <>
            <span className="k">parameters</span>
            <span>
              {params.total} ({params.required} required{params.variadic ? ", variadic" : ""})
            </span>
          </>
        )}
        {symbol.type_text && (
          <>
            <span className="k">type</span>
            <span className="mono">{symbol.type_text}</span>
          </>
        )}
        {parent && (
          <>
            <span className="k">declared in</span>
            <span>
              <SymbolLink id={parent.id} index={index}>
                {parent.kind === "Module" ? (parent.origins[0]?.file ?? parent.name) : parent.qualified_name}
              </SymbolLink>
            </span>
          </>
        )}
      </div>
      {info && (info.auth || info.capabilities.length > 0) && (
        <div className="insp-section">
          <div className="section-title">Capabilities</div>
          <div className="value-row">
            {info.auth?.anonymous && <span className="badge small" title="Explicitly open to anonymous callers">anonymous</span>}
            {info.auth?.roles.map((r) => (
              <span key={`role:${r}`} className="badge small" title="Role required to call it">
                role {r}
              </span>
            ))}
            {info.auth?.policies.map((p) => (
              <span key={`policy:${p}`} className="badge small" title="Policy or guard that protects it">
                {p}
              </span>
            ))}
            {info.capabilities.map((c) => (
              <span key={c} className="badge small cap">
                {c}
              </span>
            ))}
          </div>
        </div>
      )}
      {routes.length > 0 && (
        <div className="insp-section">
          <div className="section-title">Routes</div>
          {routes.map((r) => (
            <div key={`${r.method} ${r.path}`} className="value-row">
              <span className={`method method-${r.method.toLowerCase()}`}>{r.method}</span>
              <code>{r.path}</code>
              <span className="muted">{r.framework}</span>
            </div>
          ))}
        </div>
      )}
      {bases.length > 0 && (
        <div className="insp-section">
          <div className="section-title">Bases</div>
          <div className="value-row">
            {bases.map((e) => (
              <span key={e.to} className="chip">
                {e.kind === "Implements" ? "implements " : "extends "}
                <SymbolLink id={e.to} index={index} />
              </span>
            ))}
          </div>
        </div>
      )}
      {mutations.length > 0 && (
        <div className="insp-section">
          <div className="section-title">Changes</div>
          {mutations.map((e) => (
            <div key={`${e.to}:${e.site?.start_line}`} className="value-row">
              <span className="chip">data</span>
              <SymbolLink id={e.to} index={index} />
              {e.site && (
                <button className="link muted" onClick={() => focusCode(e.site!)}>
                  :{e.site.start_line}
                </button>
              )}
            </div>
          ))}
        </div>
      )}
      <Flavors symbol={symbol} />
      {members.length > 0 && (
        <div className="insp-section">
          <div className="section-title">
            Members <span className="count">{members.length}</span>
          </div>
          {members.slice(0, 80).map((m) => (
            <div key={m.id} className="member-row">
              <span className={`sym-icon sym-${m.kind.toLowerCase()}`}>{ICON[m.kind] ?? "·"}</span>
              <SymbolLink id={m.id} index={index}>
                <span className="mono">{m.signature ?? m.name}</span>
              </SymbolLink>
            </div>
          ))}
          {members.length > 80 && <p className="muted small">+{members.length - 80} more</p>}
        </div>
      )}
      {symbol.origins.length > 1 && (
        <div className="insp-section">
          <div className="section-title">Declared in</div>
          {symbol.origins.map((o) => (
            <div key={`${o.file}:${o.start_line}`} className="value-row">
              <button className="link mono" onClick={() => focusCode(o)}>
                {o.file}:{o.start_line}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** Attributes, preprocessor conditions and modifiers (language flavors), each in its group. */
function Flavors({ symbol }: { symbol: Symbol }) {
  const attrs = symbol.flavors.filter((f) => f.startsWith("attr:") || f.startsWith("decorator:")).map((f) => `@${f.slice(f.indexOf(":") + 1)}`);
  const conds = symbol.flavors.filter((f) => f.startsWith("#") || f.startsWith("cfg:")).map((f) => f.replace(/^cfg:/, "#[cfg] "));
  const mods = symbol.flavors.filter((f) => !f.startsWith("attr:") && !f.startsWith("decorator:") && !f.startsWith("#") && !f.startsWith("cfg:"));
  const group = (title: string, items: string[], cls = "") =>
    items.length > 0 && (
      <div className="insp-section">
        <div className="section-title">{title}</div>
        <div className="value-row">
          {items.map((a) => (
            <span key={a} className={`chip ${cls}`}>
              {a}
            </span>
          ))}
        </div>
      </div>
    );
  return (
    <>
      {group("Attributes", attrs)}
      {group("Conditions", conds, "cond")}
      {group("Modifiers", mods)}
    </>
  );
}

// ---------------------------------------------------------------------------- evidence

function EvidenceTab({ symbol, index, edges }: { symbol: Symbol; index: StructureIndex; edges: StructEdge[] }) {
  const [filter, setFilter] = useState<"all" | Confidence>("all");
  const focusCode = useUi((s) => s.focusCode);
  const keep = (e: StructEdge) => filter === "all" || (e.confidence ?? "Exact") === filter;
  const group = (list: StructEdge[], out: boolean) => {
    const by = new Map<StructEdgeKind, StructEdge[]>();
    for (const e of list.filter(keep)) (by.get(e.kind) ?? by.set(e.kind, []).get(e.kind)!).push(e);
    return [...by.entries()].map(([kind, items]) => (
      <div key={kind} className="ev-group">
        <div className="ev-group-head">
          <span className="swatch" style={{ background: relationColor(kind) }} />
          {(VERB[kind] ?? [kind, kind])[out ? 0 : 1]}
          <span className="count">{items.length}</span>
        </div>
        {items.map((e, i) => {
          const other = out ? e.to : e.from;
          const o = index.byId.get(other);
          return (
            <div key={`${other}:${i}`} className={`ev ev-${(e.confidence ?? "Exact").toLowerCase()}`}>
              <div className="ev-top">
                <span className="ev-dir">{out ? "→" : "←"}</span>
                <SymbolLink id={other} index={index} />
                <ConfidenceBadge c={e.confidence ?? "Exact"} />
              </div>
              {e.evidence && <div className="ev-text">{e.evidence}</div>}
              <div className="ev-site">
                {e.site && (
                  <button className="link muted" onClick={() => focusCode(e.site!)}>
                    {e.site.file}:{e.site.start_line}
                  </button>
                )}
                {o && !o.external && o.origins[0] && <span className="muted"> → {coordsOf(o)}</span>}
                {!e.confidence && <span className="muted"> · proven by the code</span>}
              </div>
            </div>
          );
        })}
      </div>
    ));
  };
  const outs = group(edges.filter((e) => e.from === symbol.id), true);
  const ins = group(edges.filter((e) => e.to === symbol.id), false);
  return (
    <div className="insp-pane">
      <div className="segmented">
        {(["all", "Exact", "Inferred", "Unresolved"] as const).map((c) => (
          <button key={c} className={filter === c ? "on" : ""} onClick={() => setFilter(c)}>
            {c === "all" ? "all" : c.toLowerCase()}
          </button>
        ))}
      </div>
      <div className="section-title">Outgoing</div>
      {outs.length ? outs : <p className="muted small">None.</p>}
      <div className="section-title">Incoming</div>
      {ins.length ? ins : <p className="muted small">None.</p>}
    </div>
  );
}

// ---------------------------------------------------------------------------- flows

function Tree({ node, index, root = true }: { node: TreeNode; index: StructureIndex; root?: boolean }) {
  return (
    <ul className={root ? "tree-list root" : "tree-list"}>
      {node.children.map((c, i) => (
        <li key={`${c.id}:${i}`}>
          <span className="tree-line">
            {c.kind && (
              <span className="rel-tag" style={{ borderColor: relationColor(c.kind) }}>
                {RELATION_LABEL[c.kind] ?? c.kind}
              </span>
            )}
            <SymbolLink id={c.id} index={index} />
            {c.confidence !== "Exact" && <ConfidenceBadge c={c.confidence} />}
            {c.repeat && <span className="muted small">(above)</span>}
          </span>
          {c.children.length > 0 && <Tree node={c} index={index} root={false} />}
        </li>
      ))}
    </ul>
  );
}

function FlowsTab({ symbol, index }: { symbol: Symbol; index: StructureIndex }) {
  const knowledge = useKnowledge();
  const flow = useMemo(() => fullstackFlow(index, knowledge.data, symbol.id), [index, knowledge.data, symbol.id]);
  const callers = useMemo(() => callTree(index, symbol.id, "in", 2), [index, symbol.id]);
  const callees = useMemo(() => callTree(index, symbol.id, "out", 2), [index, symbol.id]);
  const errors = useMemo(() => errorFlows(index, symbol.id), [index, symbol.id]);
  return (
    <div className="insp-pane">
      {flow && (
        <div className="insp-section">
          <div className="section-title">Full-stack flow</div>
          {flow.clients.map((c) => (
            <div key={c.id} className="value-row">
              <span className="chip">client</span>
              <SymbolLink id={c.id} index={index} />
              {c.confidence !== "Exact" && <ConfidenceBadge c={c.confidence} />}
              {c.evidence && <span className="muted small">{c.evidence}</span>}
            </div>
          ))}
          {flow.routes.map((r) => (
            <div key={`${r.method} ${r.path}`} className="value-row">
              <span className={`method method-${r.method.toLowerCase()}`}>{r.method}</span>
              <code>{r.path}</code>
              <span className="muted">→</span>
              <SymbolLink id={r.handler} index={index} />
              <span className="muted small">{r.framework}</span>
            </div>
          ))}
          {flow.downstream.map((d) => (
            <div key={d.id}>
              <div className="value-row">
                <span className="chip">handler</span>
                <SymbolLink id={d.id} index={index} />
              </div>
              {d.children.length ? <Tree node={d} index={index} /> : <p className="muted small">Calls nothing in the project.</p>}
            </div>
          ))}
          {flow.writes.length > 0 && (
            <div className="value-row">
              <span className="chip">writes</span>
              {flow.writes.map((w) => (
                <SymbolLink key={w} id={w} index={index} />
              ))}
            </div>
          )}
        </div>
      )}
      <div className="insp-section">
        <div className="section-title">
          Callers <span className="count">{callers.children.length}</span>
        </div>
        {callers.children.length ? <Tree node={callers} index={index} /> : <p className="muted small">Nothing in the project calls it{symbol.has_flow ? " (or semantic analysis is not ready)" : ""}.</p>}
      </div>
      <div className="insp-section">
        <div className="section-title">
          Callees <span className="count">{callees.children.length}</span>
        </div>
        {callees.children.length ? <Tree node={callees} index={index} /> : <p className="muted small">It calls nothing in the project.</p>}
      </div>
      {errors.map((f) => (
        <div key={f.exception} className="insp-section">
          <div className="section-title">Errors: {shortName(index, f.exception)}</div>
          <div className="value-row">
            <span className="chip">throws</span>
            <SymbolLink id={f.exception} index={index} />
          </div>
          {f.caughtBy.length > 0 ? (
            f.caughtBy.map((c) => (
              <div key={c.id} className="value-row">
                <span className="chip">caught by</span>
                <SymbolLink id={c.id} index={index} />
                {c.confidence !== "Exact" && <ConfidenceBadge c={c.confidence} />}
                {c.evidence && <span className="muted small">{c.evidence}</span>}
              </div>
            ))
          ) : (
            <p className="muted small">Nothing in the project catches it on the way up.</p>
          )}
          {f.alsoThrownBy.length > 0 && (
            <div className="value-row">
              <span className="muted small">also thrown by</span>
              {f.alsoThrownBy.slice(0, 8).map((t) => (
                <SymbolLink key={t} id={t} index={index} />
              ))}
              {f.alsoThrownBy.length > 8 && <span className="muted small">+{f.alsoThrownBy.length - 8}</span>}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------- data

function DataTab({ symbol, index }: { symbol: Symbol; index: StructureIndex }) {
  const knowledge = useKnowledge();
  const setPaletteOpen = useUi((s) => s.setPaletteOpen);
  const model = knowledge.data?.models.find((m) => m.symbol === symbol.id)?.model;
  if (model) {
    const findModel = (name: string) => knowledge.data?.models.find((m) => index.byId.get(m.symbol)?.name === name)?.symbol;
    return (
      <div className="insp-pane">
        <div className="section-title">
          Entity <span className="muted">{model.framework}</span>
          {model.table && <span className="muted"> · table {model.table}</span>}
        </div>
        <table className="grid">
          <thead>
            <tr>
              <th>field</th>
              <th>type</th>
              <th>points at</th>
              <th>how many</th>
            </tr>
          </thead>
          <tbody>
            {model.fields.map((f) => {
              const target = f.target ? findModel(f.target) : undefined;
              return (
                <tr key={f.name}>
                  <td>
                    {f.primary_key && <span className="key pk">PK</span>}
                    {f.foreign_key && <span className="key fk">FK</span>}
                    {f.name}
                  </td>
                  <td>
                    <code>{f.ty ?? ""}</code>
                  </td>
                  <td>
                    {f.target &&
                      (target ? (
                        <SymbolLink id={target} index={index}>
                          {f.target}
                        </SymbolLink>
                      ) : (
                        <button className="link" onClick={() => setPaletteOpen(true, f.target!)} title="Search for it">
                          {f.target}
                        </button>
                      ))}
                  </td>
                  <td className="muted">{cardinality(f) ?? ""}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    );
  }
  if (isTypeKind(symbol.kind)) {
    const contract = typeContract(index, symbol.id);
    return (
      <div className="insp-pane">
        <div className="section-title">Contract</div>
        {contract.bases.length > 0 && (
          <div className="value-row">
            <span className="muted small">bases</span>
            {contract.bases.map((b) => (
              <SymbolLink key={b} id={b} index={index} />
            ))}
          </div>
        )}
        {contract.members.length === 0 && <p className="muted small">No members.</p>}
        {contract.members.map((m) => (
          <div key={m.id} className="member-row">
            <span className={`sym-icon sym-${m.kind.toLowerCase()}`}>{ICON[m.kind] ?? "·"}</span>
            <SymbolLink id={m.id} index={index}>
              <span className="mono">{m.signature ?? `${kindLabel(m.kind)} ${m.name}`}</span>
            </SymbolLink>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div className="insp-pane">
      <p className="muted small">No data shape: this is neither a data model nor a type.</p>
    </div>
  );
}

// ---------------------------------------------------------------------------- actions

function ActionsTab({ symbol, index, url }: { symbol: Symbol; index: StructureIndex; url: string | null }) {
  const knowledge = useKnowledge();
  const [out, setOut] = useState<{ title: string; body: ReactNode } | null>(null);
  const isTest = (id: string) => !!knowledge.data?.symbols.find((s) => s.symbol === id)?.test;
  const file = symbol.origins[0]?.file;
  const action = (title: string, sub: string | null, run: () => void, disabled = false) => (
    <button className="action" onClick={run} disabled={disabled}>
      <span className="a-title">{title}</span>
      {sub && <span className="a-sub">{sub}</span>}
    </button>
  );
  const copyBody = async () => {
    const o = symbol.origins[0];
    if (!o) return;
    const text = await api.source(o.file);
    const body = text.split(/\r?\n/).slice(o.start_line - 1, o.end_line).join("\n");
    setOut({ title: "Body", body: <pre className="code-snippet">{body}</pre> });
    void copy(body, "the body");
  };
  const impact = () => {
    const r = impactRadius(index, symbol.id, 3);
    const total = r.levels.reduce((n, l) => n + l.length, 0);
    setOut({
      title: `Impact radius: ${total} dependents`,
      body: (
        <>
          {r.levels.map((l, i) => (
            <div key={i} className="insp-section">
              <div className="section-title">
                {i === 0 ? "Direct" : `${i + 1} steps away`} <span className="count">{l.length}</span>
              </div>
              <div className="value-row">
                {l.slice(0, 60).map((id) => (
                  <SymbolLink key={id} id={id} index={index} />
                ))}
                {l.length > 60 && <span className="muted small">+{l.length - 60}</span>}
              </div>
            </div>
          ))}
          {total === 0 && <p className="muted small">Nothing in the project depends on it.</p>}
          <div className="section-title">
            Tests that reach it <span className="count">{r.tests.length}</span>
          </div>
          <div className="value-row">
            {r.tests.map((id) => (
              <SymbolLink key={id} id={id} index={index} />
            ))}
          </div>
        </>
      ),
    });
  };
  const tests = () => {
    const hits = testsFor(index, symbol.id, isTest);
    setOut({
      title: `Tests: ${hits.length}`,
      body: hits.length ? (
        hits.map((t) => (
          <div key={t.test} className="value-row">
            <span className="chip">{t.how === "direct" ? "tests it" : "reaches it"}</span>
            <SymbolLink id={t.test} index={index} />
            {t.confidence !== "Exact" && <ConfidenceBadge c={t.confidence} />}
            {t.how === "via" && <span className="muted small">via {t.path.slice(1, -1).map((p) => shortName(index, p)).join(" → ") || "a direct call"}</span>}
          </div>
        ))
      ) : (
        <p className="muted small">No test covers it (directly, by name, or within three calls).</p>
      ),
    });
  };
  const skeleton = () => {
    if (!file) return;
    const text = skeletonText(index, file);
    setOut({ title: `Skeleton of ${file}`, body: <pre className="code-snippet">{text}</pre> });
    void copy(text, "the file skeleton");
  };
  return (
    <div className="insp-pane">
      <div className="actions-grid">
        {action("Copy file:line", coordsOf(symbol), () => void copy(coordsOf(symbol), coordsOf(symbol)), !symbol.origins[0])}
        {action("Copy id", symbol.id, () => void copy(symbol.id, "the id"))}
        {action("Copy body", null, () => void copyBody(), !symbol.origins[0])}
        {action("File skeleton", file ?? null, skeleton, !file)}
        {action("Impact radius", "what breaks if it changes (3 steps)", impact)}
        {action("Tests", "tests that cover it", tests)}
        {action("Expand neighbourhood", "Knowledge, 2 steps", () => {
          useUi.getState().setKnowledgeDepth(2);
          useUi.getState().showKnowledge(symbol.id, shortName(index, symbol.id));
        })}
        {action("Open in editor", "VS Code", () => url && window.open(url), !url)}
      </div>
      {out && (
        <div className="insp-section">
          <div className="section-title">{out.title}</div>
          {out.body}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------- containers

/** A folder or namespace box: what it holds, and opening it. */
export function ContainerInspector({ id, embedded }: { id: string; embedded?: boolean }) {
  const h = useHierarchy();
  const { index } = useStructure();
  const { scope } = useContextSet(index);
  const ui = useUi();
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  const unity = project.data?.flavors.includes("unity") ?? false;
  const item = h?.get(id);
  const vendor = useMemo(() => (h && !ui.includeVendor ? h.vendorItems((p) => isVendorPath(p, unity)) : undefined), [h, ui.includeVendor, unity]);
  const scoped = useMemo(() => scope ? h?.scoped(scope, vendor) : undefined, [h, scope, vendor]);
  const hidden = scoped?.hidden ?? vendor;
  if (!h || !item) return <div className={embedded ? undefined : "inspector"}>…</div>;
  const kids = h.shownChildren(id, hidden).map((k) => h.get(k)!);
  const byKind = new Map<string, number>();
  for (const k of kids) byKind.set(k.kind, (byKind.get(k.kind) ?? 0) + 1);
  const open = !!ui.treeExpanded[id];
  const expanded = new Set(Object.keys(ui.treeExpanded).filter((k) => ui.treeExpanded[k]));
  const choose = (childId: string) => {
    const c = h.get(childId);
    if (!c) return;
    if (c.symbol) ui.select({ kind: "symbol", id: c.symbol.id }, c.symbol.origins[0] ?? null);
    else ui.select({ kind: "container", id: childId }, null);
  };
  return (
    <div className={embedded ? undefined : "inspector"}>
      <div className="insp-head">
        <span className="insp-concept">{item.label}</span>
        <span className="muted">{kindLabel(item.kind)}</span>
      </div>
      {item.title !== item.label && <div className="insp-sub">{item.title}</div>}
      <div className="button-row">
        <button onClick={(e) => openMenu(e, contextMenuItems({ kind: "symbol", id }))}>Context ▾</button>
        {ui.view === "hierarchy" && !scoped?.hidden.has(id) ? (
          <>
            <button className="primary" onClick={() => ui.toggleTree(id)}>
              {open ? "Close" : "Open"}
            </button>
            <button onClick={() => openSubtree(h, id, expanded, hidden, scoped?.roots)} title="Open every box inside it">
              Open everything inside
            </button>
          </>
        ) : (
          <button
            className="primary"
            onClick={() => index && revealInHierarchy(h, index, id, true)}
          >
            Show in Hierarchy
          </button>
        )}
        {(item.kind === "folder" || item.id.startsWith("grp:ns:")) && (
          <button
            onClick={(e) =>
              openMenu(e, umlMenuItems(item.kind === "folder" ? { kind: "Folder", path: item.id.slice(4) } : { kind: "Symbol", id: item.id }, `UML of ${item.label}`))
            }
          >
            UML ▾
          </button>
        )}
      </div>
      <div className="insp-section">
        <div className="section-title">
          Contents <span className="count">{kids.length}</span>
        </div>
        <div className="value-row">
          {[...byKind.entries()]
            .sort((a, b) => b[1] - a[1])
            .map(([k, n]) => (
              <span key={k} className={`chip hkind-${k.toLowerCase()}`}>
                <span className="hnode-icon">{ICON[k] ?? "·"}</span> {kindLabel(k)} · {n}
              </span>
            ))}
        </div>
        {kids.slice(0, 300).map((k) => (
          <button key={k.id} className="ov-row" onClick={() => choose(k.id)} title={k.title}>
            <span className={`hnode-icon hkind-${k.kind.toLowerCase()}`}>{ICON[k.kind] ?? "·"}</span>
            <span className="ov-main">{k.label}</span>
            <span className="ov-sub">{k.symbol ? coordsOf(k.symbol) : kindLabel(k.kind)}</span>
            {h.shownChildren(k.id, hidden).length > 0 && <span className="filter-count">{h.shownChildren(k.id, hidden).length}</span>}
          </button>
        ))}
        {kids.length > 300 && <p className="muted small">+{kids.length - 300} more</p>}
        {kids.length === 0 && <p className="muted small">Empty.</p>}
      </div>
    </div>
  );
}
