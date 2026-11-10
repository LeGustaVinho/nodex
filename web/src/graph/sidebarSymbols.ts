import type { Symbol } from "../gen/Symbol";
import { isScopeKind, isTypeKind, type StructureIndex } from "./structureGraph";

export interface SidebarSymbolRow {
  key: string;
  symbol: Symbol;
  kind: "scope" | "type" | "member";
  ancestors: string[];
}

const layouts = new WeakMap<StructureIndex, {
  unscoped: Symbol[];
  scopes: { scope: Symbol; free: Symbol[]; types: Symbol[] }[];
}>();

function sidebarLayout(index: StructureIndex) {
  let layout = layouts.get(index);
  if (!layout) {
    const scopes = index.graph.symbols.filter((s) => isScopeKind(s.kind) && !s.external);
    scopes.sort((a, b) => a.qualified_name.localeCompare(b.qualified_name));
    layout = {
      unscoped: index.typesIn(null).filter((type) => !type.parent),
      scopes: scopes.map((scope) => ({
        scope,
        free: index.freeMembers(scope.id),
        types: index.typesIn(scope.qualified_name).filter((type) =>
          !type.parent || isScopeKind(index.byId.get(type.parent)?.kind ?? "Class"),
        ),
      })),
    };
    layouts.set(index, layout);
  }
  return layout;
}

export function sidebarSymbolRows(index: StructureIndex, query: string, isOpen: (id: string) => boolean) {
  const rows: SidebarSymbolRow[] = [];
  const matches = (s: Symbol) => !query || s.name.toLowerCase().includes(query);
  const typeVisible = (s: Symbol) => matches(s) || index.members(s.id).some(matches);
  const add = (symbol: Symbol, kind: SidebarSymbolRow["kind"], ancestors: string[]) => {
    rows.push({ key: JSON.stringify([...ancestors, symbol.id]), symbol, kind, ancestors });
  };

  const pushMembers = (owner: Symbol, ancestors: string[]) => {
    for (const member of index.members(owner.id)) {
      const children = index.children.get(member.id) ?? [];
      if (!(matches(member) || matches(owner) || children.some(matches))) continue;
      add(member, "member", ancestors);
      for (const child of children) {
        if (child.has_flow) add(child, "member", [...ancestors, member.id]);
      }
    }
  };

  const pushType = (type: Symbol, ancestors: string[]) => {
    if (!typeVisible(type)) return;
    add(type, "type", ancestors);
    if (!isOpen(type.id)) return;
    const parents = [...ancestors, type.id];
    pushMembers(type, parents);
    for (const child of index.children.get(type.id) ?? []) {
      if (isTypeKind(child.kind)) pushType(child, parents);
    }
  };

  const { unscoped, scopes } = sidebarLayout(index);
  for (const type of unscoped) pushType(type, []);
  for (const { scope, free, types } of scopes) {
    if (query && !types.some(typeVisible) && !free.some(matches) && !matches(scope)) continue;
    add(scope, "scope", []);
    if (!isOpen(scope.id)) continue;
    if (free.length > 0) pushMembers(scope, [scope.id]);
    for (const type of types) pushType(type, [scope.id]);
  }
  return rows;
}

const revealRows = new WeakMap<StructureIndex, Map<string, Map<string, SidebarSymbolRow>>>();

/** Use the same rows as Symbols, including its rules for which declarations are displayed. */
export function sidebarSymbolsInFile(index: StructureIndex | undefined, file: string) {
  if (!index) return undefined;
  let byFile = revealRows.get(index);
  if (!byFile) {
    byFile = new Map();
    for (const row of sidebarSymbolRows(index, "", () => true)) {
      if (row.symbol.external) continue;
      for (const origin of row.symbol.origins) {
        const entries = byFile.get(origin.file) ?? new Map<string, SidebarSymbolRow>();
        if (!entries.has(row.symbol.id)) entries.set(row.symbol.id, row);
        byFile.set(origin.file, entries);
      }
    }
    revealRows.set(index, byFile);
  }
  return byFile.get(file);
}
