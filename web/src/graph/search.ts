import type { Symbol } from "../gen/Symbol";

/**
 * Ranks symbols for the command palette: exact name, then name prefix, then a name, qualified
 * name or signature that contains the query (case-insensitive). Project symbols come before
 * external ones; within a tier, shorter names first.
 */
export function rankSymbols(symbols: readonly Symbol[], query: string, limit = 12): Symbol[] {
  const q = query.trim().toLowerCase();
  if (q.length < 2) return [];
  const scored: { s: Symbol; score: number }[] = [];
  for (const s of symbols) {
    const name = s.name.toLowerCase();
    let tier: number;
    if (name === q) tier = 0;
    else if (name.startsWith(q)) tier = 1;
    else if (name.includes(q)) tier = 2;
    else if (s.qualified_name.toLowerCase().includes(q)) tier = 3;
    else if (s.signature?.toLowerCase().includes(q)) tier = 4;
    else continue;
    scored.push({ s, score: tier * 2 + (s.external ? 1 : 0) });
  }
  scored.sort((a, b) => a.score - b.score || a.s.name.length - b.s.name.length || a.s.qualified_name.localeCompare(b.s.qualified_name));
  return scored.slice(0, limit).map((x) => x.s);
}
