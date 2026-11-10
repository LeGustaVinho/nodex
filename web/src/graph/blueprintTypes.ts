import { typeColor } from "../catalog";
import type { TypeShape } from "../gen/TypeShape";

export function formatType(shape: TypeShape | undefined): string {
  if (!shape || shape.kind === "Unknown") return "?";
  const args = (types: TypeShape[]) => types.map(formatType).join(", ");
  switch (shape.kind) {
    case "Scalar":
      return shape.name;
    case "Named":
      return shape.arguments.length ? `${shape.name}<${args(shape.arguments)}>` : shape.name;
    case "Collection": {
      const size = shape.length !== undefined ? `[${shape.length}]` : shape.rank && shape.rank > 1 ? `[${shape.rank}D]` : "";
      return `${shape.family}<${args(shape.arguments)}>${size}`;
    }
    case "Tuple":
      return `(${args(shape.items)})`;
    case "Union":
      return shape.items.map(formatType).join(" | ");
    case "Indirect":
      return `${shape.pointer ? "Pointer" : "Ref"}<${formatType(shape.target)}>`;
  }
}

/** Structured identities take precedence over the legacy text-based palette. */
export function blueprintTypeColor(shape: TypeShape | undefined, legacy?: string | null): string {
  if (!shape) return typeColor(legacy);
  switch (shape.kind) {
    case "Unknown":
      return typeColor(null);
    case "Indirect":
      return blueprintTypeColor(shape.target);
    case "Collection":
      return "#ffb74d";
    case "Tuple":
    case "Union":
      return "#b7a5ec";
    case "Named":
      return "#5dade2";
    case "Scalar":
      if (/^(U?Int|BigInt|Integer|Number)/.test(shape.name)) return typeColor("number");
      if (/^(Float|Decimal)/.test(shape.name)) return typeColor("float");
      return typeColor(shape.name);
  }
}
