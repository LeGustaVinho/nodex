import type { FlavorInfo } from "../gen/FlavorInfo";
import type { Expr } from "../gen/Expr";
import type { FlowNode } from "../gen/FlowNode";
import type { SyntaxSite } from "../gen/SyntaxSite";
import type { LanguageDetail } from "./bindingDetails";

export const syntaxId = (site: SyntaxSite) => `${site.span.file}:${site.span.start_byte}:${site.span.end_byte}:${site.key}`;

export function expressionSites(expr: Expr | null): SyntaxSite[] {
  if (!expr) return [];
  return [...(expr.syntax_sites ?? []), ...(expr.concept === "Lambda" ? [] : expr.children.flatMap(expressionSites))];
}

export function statementSites(node: FlowNode, expanded = false, inlineRoot = false, pin?: string): SyntaxSite[] {
  const sites = (node.syntax_sites ?? []).filter((site) => site.pin === pin);
  if (pin !== undefined) return sites;
  return [...sites, ...(expanded ? inlineRoot ? node.expr?.syntax_sites ?? [] : [] : expressionSites(node.expr))];
}

export function syntaxBadges(sites: readonly SyntaxSite[], resolve: (key: string) => FlavorInfo | undefined, fallback: LanguageDetail[]): LanguageDetail[] {
  const entries = [...sites].sort((a, b) => a.span.start_byte - b.span.start_byte).flatMap((site) => {
    const info = resolve(site.key);
    return info ? [{ label: info.cue ?? info.label, explain: `${info.label} · ${info.explain}`, site: syntaxId(site), fallback: info.label }] : [];
  });
  return [...entries, ...fallback.filter((detail) => !entries.some((entry) => entry.fallback === detail.label))];
}

export function borrowExpressionPath(expr: Expr | null, selected: string, path = "0"): string | undefined {
  if (!expr) return;
  if (expr.syntax_sites?.some((site) => (site.key === "borrow" || site.key === "borrow-mut") && syntaxId(site) === selected)) {
    return path;
  }
  if (expr.concept === "Lambda") return;
  for (let i = 0; i < expr.children.length; i++) {
    const found = borrowExpressionPath(expr.children[i], selected, path + "." + i);
    if (found) return found;
  }
}
