import { Handle, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";
import { conceptColor, conceptInfo, flavorInfo, flavorLabel, operationColor, operationInfo, useCatalog } from "../catalog";
import { codeLanguage } from "../graph/codeTokens";
import type { ExprData } from "../graph/flowGraph";
import { expressionDetailFlavors, isExplanationFlavor, resolvePinDetails, spreadOutput } from "../graph/blueprintExplanation";
import { BlueprintCode } from "./HighlightedCode";
import { syntaxBadges } from "../graph/syntaxSites";
import { ValueBadge } from "./ValueBadge";
import { LanguageBadges } from "./LanguageBadges";
import { TypeLabel } from "./TypeLabel";
import { blueprintTypeColor } from "../graph/blueprintTypes";
import { useUi } from "../store";

const PIN = 18;
/** Flavors that say nothing new on an operation node. */
const QUIET = new Set(["stage", "target", "loose-equality", "truthiness", "prefix", "postfix"]);

/** A pure node from an expression tree: an operation (`≥ Greater or equal`) or another expression. */
function ExprNodeImpl({ data, selected }: NodeProps) {
  const { expr, language, code, pins, params, explanations, paramRoles, flow, ownerId, path } = data as unknown as ExprData;
  const explain = useUi((state) => state.blueprintExplanations);
  const select = useUi((state) => state.select);
  const syntax = codeLanguage(language, expr.span.file);
  const catalog = useCatalog();
  const op = operationInfo(catalog, expr.op);
  const info = conceptInfo(catalog, expr.concept);
  const color = op ? operationColor(catalog, op) : conceptColor(catalog, expr.concept);
  // In expressions a Var is a read of an existing value, not a declaration.
  const title = op ? op.title : expr.concept === "Var" ? "Read" : expr.concept === "GetMember" ? "Get" : info.title;
  const symbol = !code && op ? op.symbol : "ƒ";
  const rows = Math.max(pins.length, 1);
  const outColor = blueprintTypeColor(expr.type_info?.shape, expr.ty);
  const detailFlavors = explain ? expressionDetailFlavors(expr, language) : [];
  const details = explain ? syntaxBadges(expr.syntax_sites ?? [], (key) => flavorInfo(catalog, key, language),
    detailFlavors.flatMap((flavor) => flavorInfo(catalog, flavor, language) ?? [])) : [];
  const flavors = expr.flavors.filter((f) => !QUIET.has(f) && !isExplanationFlavor(f) && !detailFlavors.includes(f));
  const output = op?.output?.split(":")[0] ?? (explain ? spreadOutput(expr) : undefined);
  const inspectType = (at: string) => select({ kind: "expr", flow, nodeId: ownerId, path: at });
  return (
    <div className={`expr ${op ? "expr-op" : ""} ${selected ? "selected" : ""}`} style={{ "--accent": color } as React.CSSProperties}>
      <div className="expr-head" title={op?.explain ?? info.explain}>
        <span className="expr-symbol">{symbol}</span>
        <span className="expr-title" title={!code ? expr.target ?? undefined : undefined}>{title}</span>
        <TypeLabel info={expr.type_info} fallback={expr.ty} onInspect={() => inspectType(path)} />
        {explain && <ValueBadge info={expr.value_info} onInspect={() => select({ kind: "expr", flow, nodeId: ownerId, path })} />}
        <LanguageBadges details={details} onInspect={(syntax) => select({ kind: "expr", flow, nodeId: ownerId, path, syntax })} />
        {(expr.op === "and" || expr.op === "or") && (
          <span className="badge small" title="The right side only runs when it can change the result">
            short-circuit
          </span>
        )}
        {flavors.map((f) => (
          <span key={f} className="badge small">
            {flavorLabel(catalog, f, language)}
          </span>
        ))}
      </div>
      {code && <BlueprintCode preview={code} language={syntax} target={expr.target} />}
      {explanations?.map((label) => <div key={label} className="blueprint-explanation" title={label}>{label}</div>)}
      <div className="expr-pins" style={{ height: rows * PIN + params.length * PIN }}>
        {pins.map((p, i) => (
          <div key={p.id} className="expr-pin" style={{ top: i * PIN }} title={p.ty ? `${p.name ?? "input"}: ${p.ty}` : p.name ?? undefined}>
            <Handle
              type="target"
              position={Position.Left}
              id={p.id}
              className="handle-data"
              style={{ top: PIN / 2, "--pin": blueprintTypeColor(p.typeInfo?.shape, p.ty) } as React.CSSProperties}
              isConnectable={false}
            />
            {p.name && <span className="expr-pin-name">{p.name}</span>}
            <TypeLabel info={p.typeInfo} fallback={p.ty} onInspect={p.path ? () => inspectType(p.path!) : undefined} />
            {explain && p.path && <ValueBadge info={p.valueInfo} onInspect={() => select({ kind: "expr", flow, nodeId: ownerId, path: p.path! })} />}
            {p.languageDetails && <LanguageBadges details={syntaxBadges(p.languageDetails.sites ?? [], (key) => flavorInfo(catalog, key, language),
              resolvePinDetails(p.languageDetails, (flavor) => flavorInfo(catalog, flavor, language)))}
              onInspect={(syntax) => select({ kind: "expr", flow, nodeId: ownerId, path: p.languageDetails!.path, syntax })} />}
            {p.value !== null && (
              <BlueprintCode preview={{ text: p.value, emphasis: p.emphasis }} language={syntax} className="expr-pin-value" />
            )}
          </div>
        ))}
        {params.map((name, i) => {
          const pin = expr.param_pins?.find((pin) => pin.label === name);
          return (
            <div key={name} className="expr-pin expr-param" style={{ top: (rows + i) * PIN }} title={paramRoles?.[i] ?? "A parameter: the value the caller passes in"}>
              <span className="expr-pin-name">{paramRoles?.[i] ? `${name} · ${paramRoles[i]}` : name}</span>
              <TypeLabel info={pin?.type_info} onInspect={() => inspectType(path)} />
              <Handle
                type="source"
                position={Position.Right}
                id={`p:${name}`}
                className="handle-data"
                style={{ top: PIN / 2, "--pin": blueprintTypeColor(pin?.type_info?.shape, pin?.ty) } as React.CSSProperties}
                isConnectable={false}
              />
            </div>
          );
        })}
      </div>
      <Handle type="source" position={Position.Right} id="out" className="handle-data" style={{ "--pin": outColor } as React.CSSProperties} isConnectable={false} />
      {output && <span className="expr-out-name">{output}</span>}
    </div>
  );
}

export const ExprNode = memo(ExprNodeImpl);
