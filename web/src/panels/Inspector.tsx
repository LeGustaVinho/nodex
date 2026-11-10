import { useQuery } from "@tanstack/react-query";
import { api } from "../api";
import { conceptColor, conceptInfo, flavorInfo, operationColor, operationInfo, useCatalog } from "../catalog";
import type { Access } from "../gen/Access";
import type { Concept } from "../gen/Concept";
import type { Expr } from "../gen/Expr";
import type { Language } from "../gen/Language";
import { useUi } from "../store";
import { useStructure } from "../views/Views";
import type { StructureIndex } from "../graph/structureGraph";
import { borrowExpressionPath, expressionSites, statementSites } from "../graph/syntaxSites";
import { ValueExplanation } from "../components/ValueBadge";
import { BorrowInspector } from "./BorrowInspector";
import { SyntaxInspector } from "./SyntaxInspector";
import { ContainerInspector, SymbolInspector } from "./SymbolInspector";
import { bindingDetails } from "../graph/bindingDetails";
import { expressionDetailFlavors, expressionExplanations, isExplanationFlavor, pinSyntaxDetails, statementDetailFlavors, withoutExpressionFacts } from "../graph/blueprintExplanation";
import { pointerBadge } from "../graph/pointers";
import { ACCESS_LABEL, ACCESS_TEXT } from "../components/OwnershipEdge";
import { ExecutionTypes, TypeDetails } from "./TypeInspector";

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
    case "ByValue":
      return "uses";
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
  const { index } = useStructure();
  return (
    <div className="inspector">
      <ExecutionTypes index={index} />
      <InspectorContent index={index} />
    </div>
  );
}

function InspectorContent({ index }: { index?: StructureIndex }) {
  const { selection } = useUi();
  const flowSymbol = selection && (selection.kind === "flow-node" || selection.kind === "expr") ? selection.flow : null;
  const flow = useQuery({ queryKey: ["flow", flowSymbol], queryFn: () => api.flow(flowSymbol!), enabled: !!flowSymbol });
  const file = flow.data?.span.file;
  const source = useQuery({
    queryKey: ["source", file],
    queryFn: ({ signal }) => api.sourceFile(file!, signal),
    enabled: false,
  });
  const openFlow = useUi((s) => s.openFlow);

  if (!selection) {
    return (
      <div className="empty-note">
        Select a node to see what it does in plain language. Double-click a namespace to see its types, double-click a member to open
        its Blueprint.
      </div>
    );
  }

  if (selection.kind === "flow-node" || selection.kind === "expr") {
    const original = flow.data?.nodes.find((n) => n.id === (selection.kind === "flow-node" ? selection.id : selection.nodeId));
    const stale = !!source.data && !!flow.data?.source_revision && source.data.revision !== flow.data.source_revision;
    const node = stale && original?.expr ? { ...original, expr: withoutExpressionFacts(original.expr) } : original;
    if (!node || !flow.data) return <div>…</div>;
    if (selection.kind === "expr") {
      const expr = exprAt(node.expr, selection.path);
      if (!expr) return <div>…</div>;
      return (
        <>
          <SyntaxInspector key={selection.syntax ?? selection.path} sites={expressionSites(expr)} language={flow.data.language}
            revision={() => flow.data?.source_revision} active={selection.syntax} />
          <div className="insp-section">
            <TypeDetails info={expr.type_info} native={expr.ty} revision={flow.data.source_revision ?? undefined} />
            <ValueExplanation info={expr.value_info} language={flow.data.language} />
          </div>
          {selection.path === "0" && node.concept === "Return" && !stale && index?.byId.get(selection.flow)?.value_info && (
            <div className="insp-section">
              <div className="section-title">Return type</div>
              <ValueExplanation info={index.byId.get(selection.flow)?.value_info} language={flow.data.language} />
            </div>
          )}
          {flow.data.language === "rust" && !stale && (expr.value_info?.kind === "Reference" ||
            expr.syntax_sites?.some((site) => site.key === "borrow" || site.key === "borrow-mut")) &&
            <BorrowInspector key={node.id + ":" + selection.path} flow={selection.flow} node={node.id} path={selection.path} />}
          <ConceptHeader concept={expr.concept} label={expr.label} />
          <OperationSection op={expr.op} language={flow.data.language} />
          {expr.params.length > 0 && (
            <div className="insp-section">
              <div className="section-title">Parameters</div>
              {expr.param_pins?.map((pin) => (
                <div key={pin.id}>
                  <code>{pin.label}</code>
                  <TypeDetails info={pin.type_info} native={pin.ty} />
                </div>
              )) ?? <div className="value-row">{expr.params.map((name) => <code key={name}>{name}</code>)}</div>}
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
          <Flavors flavors={[...new Set([...expr.flavors.filter((f) => !isExplanationFlavor(f)), ...expressionDetailFlavors(expr, flow.data.language)])]} language={flow.data.language} />
          {expressionExplanations(expr, flow.data.language).map((explanation) => (
            <p className="explain" key={explanation}>{explanation}</p>
          ))}
        </>
      );
    }
    const owner = node.concept === "Entry" || node.concept === "EventEntry" ? index?.byId.get(flow.data.symbol) : undefined;
    const pins = [...node.inputs, ...node.outputs];
    const borrowPath = flow.data.language === "rust" && !stale && selection.syntax
      ? borrowExpressionPath(node.expr, selection.syntax) : undefined;
    const pinDetails = pins.flatMap((pin) => {
      if (selection.pin !== undefined && selection.pin !== pin.id) return [];
      const details = [...pinSyntaxDetails(node, pin.id), ...(pin.binding ? bindingDetails(pin.binding, flow.data!.language) : [])];
      const pointer = pointerBadge(pin.ty);
      if (pointer) details.push(pointer);
      const accesses = new Set(flow.data!.edges.filter((e) => e.to === node.id && e.to_pin === pin.id).flatMap((e) => e.access ? [e.access] : []));
      for (const access of accesses) details.push({ label: ACCESS_LABEL[access], explain: ACCESS_TEXT[access] });
      return details.length || pin.kind === "Data" ? [{ pin, details }] : [];
    });
    return (
      <>
        <SyntaxInspector key={selection.syntax ?? selection.pin ?? node.id}
          sites={selection.pin !== undefined ? statementSites(node, false, false, selection.pin) : [...statementSites(node), ...pins.flatMap((p) => statementSites(node, false, false, p.id))]}
          language={flow.data.language} revision={() => flow.data?.source_revision} active={selection.syntax} />
        {borrowPath && <BorrowInspector key={node.id + ":" + borrowPath} flow={selection.flow} node={node.id} path={borrowPath} />}
        {pinDetails.map(({ pin, details }) => {
          const shadows = pin.binding?.shadows;
          const previous = shadows && flow.data!.nodes.find((n) => n.id === shadows.node);
          return (
            <div className="insp-section" key={pin.id}>
              <div className="section-title">
                {pin.kind === "Data" && (node.outputs.includes(pin)
                  ? (pin.id.startsWith("write:") ? "Updates" : "Defines")
                  : usesVerb(flow.data!.edges.find((e) => e.to === node.id && e.to_pin === pin.id)?.access)) || "Pin"} · {pin.label}
              </div>
              {pin.kind === "Data" && <TypeDetails info={stale ? undefined : pin.type_info} native={pin.ty} revision={flow.data?.source_revision ?? undefined} />}
              {pin.kind === "Data" && <ValueExplanation info={stale && pin.value_info?.basis === "TypeAnalysis" ? undefined : pin.value_info} language={flow.data!.language} />}
              {flow.data!.language === "rust" && !stale && selection.pin === pin.id && pin.value_info?.kind === "Reference" &&
                <BorrowInspector key={node.id + ":" + pin.id} flow={selection.flow} node={node.id} pin={pin.id} />}
              {details.map((detail) => <div className="flavor" key={detail.label}>
                <span className="badge">{detail.label}</span><span className="flavor-explain">{detail.explain}</span>
              </div>)}
              {previous && <button onClick={() => useUi.getState().select({
                kind: "flow-node", flow: selection.flow, id: previous.id, pin: shadows.pin,
              })}>Previous binding</button>}
            </div>
          );
        })}
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
        <Flavors flavors={[...new Set([...node.flavors.filter((f) => !isExplanationFlavor(f)), ...statementDetailFlavors(node, flow.data.language, false)])]} language={flow.data.language} />
        {node.writes.some((name) => !node.outputs.some((pin) => pin.label === name)) && (
          <div className="insp-section">
            <div className="section-title">Values</div>
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
      </>
    );
  }

  if (selection.kind === "container") return <ContainerInspector id={selection.id} embedded />;
  const symbol = index?.byId.get(selection.id);
  if (!symbol || !index) return <div>…</div>;
  return <SymbolInspector key={symbol.id} symbol={symbol} index={index} syntax={selection.syntax} embedded />;
}
