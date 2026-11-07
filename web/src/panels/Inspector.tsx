import { useQuery } from "@tanstack/react-query";
import { api } from "../api";
import { conceptColor, conceptInfo, flavorInfo, typeColor, useCatalog } from "../catalog";
import type { Concept } from "../gen/Concept";
import type { Expr } from "../gen/Expr";
import type { Language } from "../gen/Language";
import { useUi } from "../store";
import { useStructure } from "../views/Views";

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
    return (
      <div className="inspector">
        <ConceptHeader concept={node.concept} label={node.label} />
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
                  <span className="dot" style={{ background: typeColor(p.ty) }} /> uses <code>{p.label}</code>
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
  const outgoing = index.graph.edges.filter((e) => e.from === symbol.id && e.kind !== "Contains");
  const incoming = index.graph.edges.filter((e) => e.to === symbol.id && e.kind !== "Contains");
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
      {symbol.has_flow && (
        <div className="button-row">
          <button className="primary" onClick={() => openFlow(symbol.id)}>
            Open Blueprint
          </button>
          <button onClick={() => showCalls(symbol.id)}>Show calls</button>
        </div>
      )}
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
