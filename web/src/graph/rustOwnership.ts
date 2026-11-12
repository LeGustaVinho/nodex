import type { BorrowSource } from "../gen/BorrowSource";
import type { FlowGraph } from "../gen/FlowGraph";
import type { Pin } from "../gen/Pin";

export interface RustOwnershipRow {
  pin: string;
  name: string;
  role: string;
  origin?: string;
}

export function originLabel(sources: readonly BorrowSource[]) {
  if (sources.length > 1) return "Multiple possible origins";
  const source = sources[0];
  if (!source) return "Origin unknown";
  const place = source.place && source.place !== source.owner ? ` · Borrowed: ${source.place}` : "";
  switch (source.kind) {
    case "Local": return source.owner ? `Owner: ${source.owner}${place}` : "Local owner";
    case "Shared": return source.owner ? `Shared owner: ${source.owner}${place}` : "Shared ownership";
    case "Caller": return source.place ? `From caller · ${source.place}` : "From caller";
    case "Static": return "Static storage";
    case "Temporary": return "Temporary value";
    case "Unknown": return "Origin unknown";
  }
}

export function rustOwnershipRows(flow: FlowGraph, visible: boolean) {
  const rows = new Map<string, RustOwnershipRow[]>();
  if (!visible || flow.language !== "rust") return rows;
  const key = (node: string, pin: string) => `${node} ${pin}`;
  const origins = new Map((flow.borrows ?? []).map((site) => [key(site.node, site.pin), site.sources]));
  const incoming = new Map(flow.edges.filter((edge) => edge.kind === "Data").map((edge) => [key(edge.to, edge.to_pin), edge]));
  const outputNames = new Map(flow.nodes.flatMap((node) => node.outputs.map((pin) => [key(node.id, pin.id), pin.label] as const)));
  const owners = new Map<string, Set<string>>();
  for (const { node, pin, sources } of flow.borrows ?? []) {
    const reference = outputNames.get(key(node, pin));
    for (const { binding, kind } of sources) {
      if (!binding || kind !== "Local" && kind !== "Shared") continue;
      const owner = key(binding.node, binding.pin);
      let references = owners.get(owner);
      if (!references) owners.set(owner, references = new Set());
      if (sources.length === 1 && reference) references.add(reference);
    }
  }
  for (const node of flow.nodes) {
    const details = (pin: Pin, output: boolean): RustOwnershipRow[] => {
      if (pin.kind !== "Data") return [];
      if (output && pin.binding?.form === "Let" && !pin.binding.has_initializer) {
        return [{ pin: pin.id, name: pin.label, role: "Uninitialized" }];
      }
      const site = key(node.id, pin.id);
      const sources = origins.get(site);
      const info = pin.value_info;
      const shape = pin.type_info?.shape;
      const borrowed = info?.kind === "Reference" || !!sources || shape?.kind === "Indirect" && !shape.pointer;
      if (borrowed) {
        const role = info?.qualifier === "ExclusiveBorrow" ? "Mutable borrow"
          : info?.qualifier === "SharedBorrow" ? "Shared borrow" : "Borrowed reference";
        return [{ pin: pin.id, name: pin.label, role, origin: originLabel(sources ?? []) }];
      }
      if (!output || !pin.id.startsWith("var:")) return [];
      const role = info?.qualifier === "Shared" ? "Shared owner"
        : info?.qualifier === "Weak" ? "Weak handle"
        : info?.qualifier === "Raw" || shape?.kind === "Indirect" && shape.pointer ? "Raw pointer"
        : info?.qualifier === "Unique" || info?.kind === "Value" || owners.has(site) ? "Owner"
        : "Ownership unknown";
      const references = [...(owners.get(site) ?? [])];
      const transfer = node.concept === "Var" && node.expr?.concept === "Var"
        ? incoming.get(key(node.id, `use:${node.expr.label}`)) : undefined;
      const from = transfer && outputNames.get(key(transfer.from, transfer.from_pin));
      const origin = from && transfer?.access === "Move" ? `Ownership moved from: ${from}`
        : from && transfer?.access === "Copy" ? `Copied from: ${from}`
        : references.length ? `Referenced by: ${references.join(", ")}` : undefined;
      return [{ pin: pin.id, name: pin.label, role, origin }];
    };
    const pins = [...node.outputs.flatMap((pin) => details(pin, true)), ...node.inputs.flatMap((pin) => details(pin, false))];
    if (pins.length) rows.set(node.id, pins);
  }
  return rows;
}

export function rustOwnershipHeight(rows: readonly RustOwnershipRow[]) {
  return rows.length ? 6 + rows.reduce((height, row) => height + (row.origin ? 35 : 21), 0) : 0;
}
