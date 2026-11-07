import { useQuery } from "@tanstack/react-query";
import { api } from "./api";
import type { Catalog } from "./gen/Catalog";
import type { Concept } from "./gen/Concept";
import type { ConceptInfo } from "./gen/ConceptInfo";
import type { FlavorInfo } from "./gen/FlavorInfo";
import type { Language } from "./gen/Language";

export function useCatalog(): Catalog | undefined {
  return useQuery({ queryKey: ["catalog"], queryFn: api.catalog, staleTime: Infinity }).data;
}

const FALLBACK: ConceptInfo = { title: "Code", category: "escape", shape: "opaque", explain: "" };

export function conceptInfo(catalog: Catalog | undefined, concept: Concept): ConceptInfo {
  return catalog?.concepts[concept] ?? FALLBACK;
}

export function conceptColor(catalog: Catalog | undefined, concept: Concept): string {
  const info = conceptInfo(catalog, concept);
  return catalog?.categories[info.category]?.color ?? "#566573";
}

/** Looks a flavor up in the language table first, then in the shared table. */
export function flavorInfo(catalog: Catalog | undefined, flavor: string, language?: Language): FlavorInfo | undefined {
  if (!catalog) return undefined;
  const lang = language ? catalog.flavors[language]?.[flavor] : undefined;
  if (lang) return lang;
  const common = catalog.flavors["common"]?.[flavor];
  if (common) return common;
  if (flavor.startsWith("attr:")) return { label: `[${flavor.slice(5)}]`, explain: "An attribute (annotation) attached to the declaration." };
  if (flavor.startsWith("#if ") || flavor.startsWith("#elif ") || flavor === "#else")
    return { label: flavor, explain: "Only compiled when this build condition holds (for example DEBUG or UNITY_EDITOR); other builds do not contain this code." };
  if (flavor.startsWith("extends:"))
    return { label: `extends ${flavor.slice(8)}`, explain: "A member added to an existing type from outside it; it is called as if it belonged to that type." };
  if (flavor.startsWith("label:"))
    return { label: `label ${flavor.slice(6)}`, explain: "A named statement: `break`/`continue` (and `goto` in C#) can target it by name." };
  if (flavor.startsWith("discriminated:"))
    return {
      label: `by ${flavor.slice(14)}`,
      explain: `A union whose variants are told apart by the \`${flavor.slice(14)}\` property; a switch on it narrows the type in each case.`,
    };
  return undefined;
}

export function flavorLabel(catalog: Catalog | undefined, flavor: string, language?: Language): string {
  return flavorInfo(catalog, flavor, language)?.label ?? flavor.replace(/^attr:/, "@");
}

/** Shape glyphs shown in node headers. */
export const SHAPE_ICON: Record<string, string> = {
  event: "◆",
  statement: "▸",
  branch: "◇",
  loop: "↻",
  scope: "▣",
  terminal: "■",
  latent: "◷",
  pure: "ƒ",
  opaque: "‹›",
};

/** Data wire color by type text, so the same kind of value has the same color in every language. */
export function typeColor(ty: string | null | undefined): string {
  if (!ty) return "#9aa4b2";
  const t = ty.replace(/\?$/, "").toLowerCase();
  if (t === "bool" || t === "boolean") return "#e74c3c";
  if (/^(u?int|u?long|u?short|s?byte|i\d+|u\d+|usize|isize|number|bigint)$/.test(t)) return "#2ecc71";
  if (/^(float|double|decimal|f32|f64)$/.test(t)) return "#a3e635";
  if (t === "string" || t === "str" || t === "&str" || t === "char") return "#f06292";
  if (/(list|array|ienumerable|vec|\[\])/.test(t)) return "#ffb74d";
  return "#5dade2";
}
