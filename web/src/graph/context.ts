import type { FileSummary } from "../gen/FileSummary";
import type { Language } from "../gen/Language";
import type { UmlScope } from "../gen/UmlScope";
import type { StructureIndex } from "./structureGraph";

export type ContextItem = { kind: "folder"; path: string } | { kind: "symbol"; id: string };

function folderOf(item: ContextItem): string | undefined {
  if (item.kind === "folder") return item.path;
  if (item.id.startsWith("dir:")) return item.id.slice(4);
}

export function contextKey(item: ContextItem): string {
  return item.kind === "folder" ? `folder:${item.path}`
    : item.id.startsWith("dir:") ? `folder:${item.id.slice(4)}` : `symbol:${item.id}`;
}

function inFolder(file: string, folder: string): boolean {
  const path = folder.replace(/^\/+|\/+$/g, "");
  return !path || file.startsWith(`${path}/`);
}

export function contextSymbols(index: StructureIndex, items: readonly ContextItem[]): Set<string> | null {
  if (!items.length) return null;
  const covered = new Set<string>();
  const folders: string[] = [];
  const namespaces: { name: string; language?: Language }[] = [];
  for (const item of items) {
    const folder = folderOf(item);
    if (folder !== undefined) folders.push(folder);
    else if (item.kind === "symbol") {
      if (item.id.startsWith("grp:ns:")) {
        namespaces.push({ name: item.id.slice(7) });
        continue;
      }
      const symbol = index.byId.get(item.id);
      if (symbol?.kind === "Namespace" && !symbol.external) {
        namespaces.push({ name: symbol.qualified_name, language: symbol.language });
      }
      const stack = symbol ? [symbol] : [];
      while (stack.length) {
        const s = stack.pop()!;
        if (s.external || covered.has(s.id)) continue;
        covered.add(s.id);
        stack.push(...(index.children.get(s.id) ?? []));
      }
    }
  }
  if (folders.length || namespaces.length) {
    for (const s of index.graph.symbols) {
      if (s.external || covered.has(s.id)) continue;
      const ns = namespaces.length ? index.namespaceSymbolOf(s.id) : undefined;
      const inNamespace = ns && namespaces.some(({ name, language }) => {
        if (language && (ns.kind !== "Namespace" || ns.language !== language)) return false;
        const qualified = ns.qualified_name;
        return qualified === name || [".", "::", "/"].some((sep) => qualified.startsWith(name + sep));
      });
      if (inNamespace || s.origins.some((o) => folders.some((f) => inFolder(o.file, f)))) covered.add(s.id);
    }
  }
  return covered;
}

export function contextFiles(files: FileSummary[], items: readonly ContextItem[], index?: StructureIndex): FileSummary[] {
  if (!items.length) return files;
  const folders = items.flatMap((item) => folderOf(item) ?? []);
  const paths = new Set<string>();
  const symbols = items.filter((item) => folderOf(item) === undefined);
  const covered = index ? contextSymbols(index, symbols) : null;
  for (const id of covered ?? []) {
    for (const origin of index?.byId.get(id)?.origins ?? []) paths.add(origin.file);
  }
  return files.filter((f) => paths.has(f.path) || folders.some((p) => inFolder(f.path, p)));
}

export function contextLabel(items: readonly ContextItem[], name?: (id: string) => string | undefined): string {
  if (!items.length) return "Whole project";
  if (items.length > 1) return `${items.length} items`;
  const item = items[0];
  const folder = folderOf(item);
  if (folder !== undefined) return `${folder.replace(/\/+$/, "")}/`;
  if (item.kind === "symbol") return item.id.startsWith("grp:ns:") ? item.id.slice(7) : name?.(item.id) ?? item.id;
  return item.path;
}

export function toUmlScope(items: readonly ContextItem[]): UmlScope {
  if (!items.length) return { kind: "Project" };
  if (items.length === 1 && items[0].kind === "folder") return { kind: "Folder", path: items[0].path };
  return { kind: "Symbols", ids: items.map((i) => i.kind === "symbol" ? i.id : `dir:${i.path}`) };
}
