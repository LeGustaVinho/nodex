import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import type { ReadingSite } from "../gen/ReadingSite";

export function readingsByNode(flow: FlowGraph, enabled: boolean, stale: boolean) {
  const nodes = new Map<string, ReadingSite[]>();
  if (!enabled || stale) return nodes;
  for (const site of flow.reading ?? []) {
    const entries = nodes.get(site.node) ?? [];
    if (!nodes.has(site.node)) nodes.set(site.node, entries);
    entries.push(site);
  }
  return nodes;
}

export function compactReading(sites: readonly ReadingSite[]): ReadingSite[] {
  const depth = (site: ReadingSite) => site.path?.split(".").length ?? 0;
  const paths = sites.filter((site) => site.content.kind === "Paths").sort((a, b) => depth(a) - depth(b));
  const steps = sites.find((site) => site.content.kind === "Steps");
  const overview = sites.find((site) => site.content.kind === "Detail" && (!site.path || site.path === "0"));
  const primary = steps ?? paths[0] ?? sites.find((site) => site.content.kind === "Detail");
  const earlyExit = paths.find((site) => site.content.kind === "Paths" && site.content.branches.some((branch) => branch.pin === "error"));
  return [...new Set([overview, primary, earlyExit].filter((site) => site !== undefined))];
}

export function executionReading(node: FlowNode, sites: readonly ReadingSite[]) {
  const ports = new Set(node.outputs.filter((pin) => pin.kind === "Exec").map((pin) => pin.id));
  const candidates = sites.flatMap((site) => {
    const { content } = site;
    // A nested let-else or loop cannot use its enclosing statement's pins.
    if (content.kind !== "Paths" || content.deferred
      || node.flavors.includes("while-let") && (site.path || !content.branches.some((branch) => branch.pin === "body"))
      || !content.branches.every((branch) => branch.pin && ports.has(branch.pin) && (!site.path || (branch.pin !== "else" && branch.pin !== "body")))) return [];
    const labels = Object.fromEntries(content.branches.map((branch) => [
      branch.pin!, `${branch.label} · ${branch.pin === "out" ? branch.output : branch.action}`,
    ]));
    return [{ site, labels }];
  });
  // A single exit port can stand for several different question marks.
  return candidates.length === 1 ? candidates[0] : undefined;
}

export function readingHeight(sites: readonly ReadingSite[]) {
  return sites.reduce((height, { content, borrow_sources }) => height + (content.kind === "Steps"
    ? 28 + 35 * content.steps.length
    : content.kind === "Paths"
    ? 48 + 35 * content.branches.length
    : content.kind === "Pattern"
      ? 32 + (content.route ? 17 : 0) + content.parts.reduce((height, part) => height + 29 + (part.borrow_sources?.length ? 18 : 0), 0)
      : 24 + (borrow_sources?.length ? 18 : 0)), 0);
}

export function hasLetChain(node: FlowNode) {
  return node.expr?.op === "and" && (node.flavors.includes("if-let") || node.flavors.includes("while-let"));
}

export function routeName(node: FlowNode, pin: string) {
  if (pin === "body" && node.flavors.includes("while-let")) return "Loop body · when the conditions pass";
  if (pin === "true" && hasLetChain(node)) return "When the conditions pass";
  if (pin === "true" || pin === "out") return "When the pattern matches";
  const label = node.outputs.find((output) => output.id === pin)?.label;
  return pin.startsWith("case:") && label ? `Case · ${label}` : label;
}

export function readingKey(site: ReadingSite) {
  return `${site.path ?? "statement"}:${site.span.start_byte}:${site.span.end_byte}:${site.content.kind}`;
}
