import { useQuery } from "@tanstack/react-query";
import { api } from "../api";
import { conceptColor, conceptInfo, flavorInfo, operationColor, operationInfo, typeColor, useCatalog } from "../catalog";
import type { Access } from "../gen/Access";
import type { Concept } from "../gen/Concept";
import type { Expr } from "../gen/Expr";
import type { Language } from "../gen/Language";
import { useUi } from "../store";
import { useKnowledge } from "../views/Knowledge";
import { useStructure } from "../views/Views";
import type { StructEdge } from "../gen/StructEdge";
import type { StructureIndex } from "../graph/structureGraph";

const KNOWLEDGE_KINDS = new Set(["HttpCalls", "ResolvesDi", "Tests", "Throws", "CaughtBy", "Mutates"]);

const VERB: Record<string, [string, string]> = {
  HttpCalls: ["sends HTTP requests to", "receives HTTP requests from"],
  ResolvesDi: ["is provided by", "provides"],
  Tests: ["tests", "is tested by"],
  Throws: ["throws", "is thrown by"],
  CaughtBy: ["is caught by", "catches errors of"],
  Mutates: ["changes", "is changed by"],
};

/** Knowledge about a symbol: why it runs, what it does to the world, and the evidence. */
function KnowledgeSection({ id, index }: { id: string; index: StructureIndex }) {
  const knowledge = useKnowledge();
  const focusCode = useUi((s) => s.focusCode);
  const select = useUi((s) => s.select);
  const info = knowledge.data?.symbols.find((s) => s.symbol === id);
  const routes = knowledge.data?.routes.filter((r) => r.handler === id) ?? [];
  const model = knowledge.data?.models.find((m) => m.symbol === id)?.model;
  const edges = index.graph.edges.filter((e) => KNOWLEDGE_KINDS.has(e.kind) && (e.from === id || e.to === id));
  if (!info && routes.length === 0 && !model && edges.length === 0) return null;
  const name = (other: string) => index.byId.get(other)?.name ?? other.replace(/^ext:/, "");
  const row = (e: StructEdge) => {
    const out = e.from === id;
    const other = out ? e.to : e.from;
    const [verbOut, verbIn] = VERB[e.kind] ?? [e.kind, e.kind];
    return (
      <div key={`${e.kind}:${e.from}:${e.to}`} className="evidence">
        <div className="value-row">
          <span>{out ? verbOut : verbIn}</span>
          <button className="link" onClick={() => select({ kind: "symbol", id: other }, index.byId.get(other)?.origins[0] ?? null)}>
            {name(other)}
          </button>
          {e.confidence && <span className={`conf conf-${e.confidence.toLowerCase()}`}>{e.confidence.toLowerCase()}</span>}
        </div>
        {(e.evidence || e.site) && (
          <div className="evidence-detail">
            {e.evidence && <span>{e.evidence}</span>}
            {e.site && (
              <button className="link muted" onClick={() => focusCode(e.site!)}>
                {e.site.file}:{e.site.start_line}
              </button>
            )}
          </div>
        )}
      </div>
    );
  };
  return (
    <div className="insp-section">
      <div className="section-title">Knowledge</div>
      {info && (
        <div className="value-row">
          {info.entry && <span className="badge" title="Runs without being called by project code">entry: {info.entry.label}</span>}
          {info.test && <span className="badge">test</span>}
          {info.complexity > 0 && <span className="badge" title="Cyclomatic complexity: 1 + the decisions it makes">complexity {info.complexity}</span>}
          {info.capabilities.map((c) => (
            <span key={c} className="badge small cap">
              {c}
            </span>
          ))}
        </div>
      )}
      {routes.map((r) => (
        <div key={`${r.method} ${r.path}`} className="value-row">
          <span className={`method method-${r.method.toLowerCase()}`}>{r.method}</span>
          <code>{r.path}</code>
          <span className="muted">{r.framework}</span>
        </div>
      ))}
      {model && (
        <table className="model-fields">
          <tbody>
            {model.fields.map((f) => (
              <tr key={f.name}>
                <td className={f.primary_key ? "pk" : ""}>{f.name}</td>
                <td className="muted">{f.ty}</td>
                <td className="muted">{f.target ? `-> ${f.target}` : ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {edges.map(row)}
    </div>
  );
}

function Flavors({ flavors, language }: { flavors: string[]; language?: Language }) {
  const catalog = useCatalog();
  if (flavors.length === 0) return null;
  return (
    <div className="insp-section">
      <div className="section-title">Language details</div>
      {flavors.map((f) => {
        const info = flavorInfo(catalog, f, language);
        return (
          <div key={f} className="flavor">
            <span className={`badge ${f.startsWith("unity:") ? "badge-unity" : ""}`}>{info?.label ?? f}</span>
            {info?.explain && <span className="flavor-explain">{info.explain}</span>}
          </div>
        );
      })}
    </div>
  );
}

function ConceptHeader({ concept, label }: { concept: Concept; label: string }) {
  const catalog = useCatalog();
  const info = conceptInfo(catalog, concept);
  return (
    <>
      <div className="insp-head" style={{ borderColor: conceptColor(catalog, concept) }}>
        <span className="insp-concept">{info.title}</span>
        <span className="muted">{concept}</span>
      </div>
      <div className="insp-label">{label}</div>
      <div className="insp-section">
        <div className="section-title">What this means</div>
        <p className="explain">{info.explain}</p>
      </div>
    </>
  );
}

/** A shared operation: what it does and how each language writes it. */
function OperationSection({ op: key, language }: { op: string | null; language: Language }) {
  const catalog = useCatalog();
  const op = operationInfo(catalog, key);
  if (!op) return null;
  const family = catalog?.families[op.family];
  const rows: [string, Language | "rust", string][] = [
    ["C#", "csharp", op.cs],
    ["TypeScript", "typescript", op.ts],
    ["Rust", "rust", op.rs],
  ];
  return (
    <>
      <div className="insp-section">
        <div className="section-title">Operation</div>
        <div className="value-row">
          <span className="op-chip" style={{ background: operationColor(catalog, op) }}>
            {op.symbol}
          </span>
          <strong>{op.title}</strong>
          {family && <span className="muted">{family.title}</span>}
          {!op.pure && <span className="badge small">has an effect</span>}
        </div>
        <p className="explain">{op.explain}</p>
        <div className="value-row muted">
          {op.inputs.length > 0 ? op.inputs.map((p) => p.replace(":", ": ")).join(", ") : "no inputs"}
          {op.output && ` → ${op.output.replace(":", ": ")}`}
        </div>
      </div>
      <div className="insp-section">
        <div className="section-title">In other languages</div>
        <table className="op-langs">
          <tbody>
            {rows.map(([name, lang, code]) => (
              <tr key={name} className={lang === language ? "current" : ""}>
                <td>{name}</td>
                <td>{code ? <code>{code}</code> : <span className="muted">no direct equivalent</span>}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}

/** How a statement takes a value (Rust ownership), in plain words. */
function usesVerb(access: Access | null | undefined): string {
  switch (access) {
    case "Move":
      return "takes (moves)";
    case "Copy":
      return "copies";
    case "Borrow":
      return "borrows";
    case "BorrowMut":
      return "borrows to change";
    default:
      return "uses";
  }
}

function exprAt(root: Expr | null, path: string): Expr | null {
  let cur = root;
  for (const part of path.split(".").slice(1)) cur = cur?.children[Number(part)] ?? null;
  return cur;
}

export function Inspector() {
  const { selection } = useUi();
  const { index } = useStructure();
  const flowSymbol = selection && selection.kind !== "symbol" ? selection.flow : null;
  const flow = useQuery({ queryKey: ["flow", flowSymbol], queryFn: () => api.flow(flowSymbol!), enabled: !!flowSymbol });
  const openFlow = useUi((s) => s.openFlow);
  const showCalls = useUi((s) => s.showCalls);

  if (!selection) {
    return (
      <div className="inspector empty-note">
        Select a node to see what it does in plain language. Double-click a namespace to see its types, double-click a member to open
        its Blueprint.
      </div>
    );
  }

  if (selection.kind === "flow-node" || selection.kind === "expr") {
    const node = flow.data?.nodes.find((n) => n.id === (selection.kind === "flow-node" ? selection.id : selection.nodeId));
    if (!node || !flow.data) return <div className="inspector">…</div>;
    if (selection.kind === "expr") {
      const expr = exprAt(node.expr, selection.path);
      if (!expr) return <div className="inspector">…</div>;
      return (
        <div className="inspector">
          <ConceptHeader concept={expr.concept} label={expr.label} />
          <OperationSection op={expr.op} language={flow.data.language} />
          {expr.params.length > 0 && (
            <div className="insp-section">
              <div className="section-title">Parameters</div>
              <div className="value-row">
                {expr.params.map((p) => (
                  <code key={p}>{p}</code>
                ))}
              </div>
            </div>
          )}
          {expr.target && (
            <div className="insp-section">
              <div className="section-title">Resolves to</div>
              <div className="value-row">
                <code>{expr.target}</code>
                {expr.symbol && index?.byId.get(expr.symbol)?.has_flow && (
                  <button onClick={() => openFlow(expr.symbol!)}>Open Blueprint</button>
                )}
              </div>
            </div>
          )}
          <Flavors flavors={expr.flavors} language={flow.data.language} />
        </div>
      );
    }
    const owner = node.concept === "Entry" || node.concept === "EventEntry" ? index?.byId.get(flow.data.symbol) : undefined;
    return (
      <div className="inspector">
        <ConceptHeader concept={node.concept} label={node.label} />
        {node.concept === "Call" && <OperationSection op={node.expr?.op ?? null} language={flow.data.language} />}
        {owner?.doc && (
          <div className="insp-section">
            <div className="section-title">Author's description</div>
            <p className="doc">{owner.doc}</p>
          </div>
        )}
        {node.calls.length > 0 && (
          <div className="insp-section">
            <div className="section-title">Calls</div>
            {node.calls.map((c) => (
              <div key={c.display} className="value-row">
                <code title={c.external ? "outside this project" : "declared in this project"}>{c.display}</code>
                {c.external && <span className="muted">external</span>}
                {c.symbol && index?.byId.get(c.symbol)?.has_flow && <button onClick={() => openFlow(c.symbol!)}>Open Blueprint</button>}
              </div>
            ))}
          </div>
        )}
        <Flavors flavors={node.flavors} language={flow.data.language} />
        {(node.inputs.some((p) => p.kind === "Data") || node.outputs.some((p) => p.kind === "Data")) && (
          <div className="insp-section">
            <div className="section-title">Values</div>
            {node.outputs
              .filter((p) => p.kind === "Data")
              .map((p) => (
                <div key={p.id} className="value-row">
                  <span className="dot" style={{ background: typeColor(p.ty) }} /> defines <code>{p.label}</code>
                  {p.ty && <span className="muted"> : {p.ty}</span>}
                </div>
              ))}
            {node.inputs
              .filter((p) => p.kind === "Data")
              .map((p) => (
                <div key={p.id} className="value-row">
                  <span className="dot" style={{ background: typeColor(p.ty) }} /> {usesVerb(flow.data!.edges.find((e) => e.to === node.id && e.to_pin === p.id)?.access)}{" "}
                  <code>{p.label}</code>
                  {p.ty && <span className="muted"> : {p.ty}</span>}
                </div>
              ))}
            {node.writes
              .filter((w) => !node.outputs.some((p) => p.label === w))
              .map((w) => (
                <div key={w} className="value-row">
                  <span className="dot" /> changes <code>{w}</code>
                </div>
              ))}
          </div>
        )}
        <div className="insp-section">
          <div className="section-title">Code</div>
          <pre className="code-snippet">{node.code}</pre>
        </div>
      </div>
    );
  }

  const symbol = index?.byId.get(selection.id);
  if (!symbol || !index) return <div className="inspector">…</div>;
  const structural = (e: StructEdge) => e.kind !== "Contains" && !KNOWLEDGE_KINDS.has(e.kind);
  const outgoing = index.graph.edges.filter((e) => e.from === symbol.id && structural(e));
  const incoming = index.graph.edges.filter((e) => e.to === symbol.id && structural(e));
  const name = (id: string) => index.byId.get(id)?.qualified_name ?? id;
  return (
    <div className="inspector">
      <div className="insp-head">
        <span className="insp-concept">{symbol.name}</span>
        <span className="muted">
          {symbol.external ? "external " : ""}
          {symbol.kind}
        </span>
      </div>
      {symbol.signature && <pre className="code-snippet">{symbol.signature}</pre>}
      {symbol.doc && <p className="doc">{symbol.doc}</p>}
      {symbol.has_flow && (
        <div className="button-row">
          <button className="primary" onClick={() => openFlow(symbol.id)}>
            Open Blueprint
          </button>
          <button onClick={() => showCalls(symbol.id)}>Show calls</button>
        </div>
      )}
      <KnowledgeSection id={symbol.id} index={index} />
      <Flavors flavors={symbol.flavors} language={symbol.language} />
      {(outgoing.length > 0 || incoming.length > 0) && (
        <div className="insp-section">
          <div className="section-title">Relations</div>
          {outgoing.map((e) => (
            <div key={`o${e.to}${e.kind}`} className="value-row">
              {e.kind.toLowerCase()} → <code>{name(e.to)}</code>
            </div>
          ))}
          {incoming.map((e) => (
            <div key={`i${e.from}${e.kind}`} className="value-row">
              <code>{name(e.from)}</code> → {e.kind.toLowerCase()} this
            </div>
          ))}
        </div>
      )}
      {symbol.origins.length > 1 && (
        <div className="insp-section">
          <div className="section-title">Declared in</div>
          {symbol.origins.map((o) => (
            <div key={`${o.file}:${o.start_line}`} className="value-row">
              {o.file}:{o.start_line}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
