import type { Span } from "./gen/Span";

export type CloseCodeTabs = "one" | "other" | "all" | "unmodified" | "left" | "right";

interface CodeTabsState {
  codeTabs: string[];
  activeCodeFile: string | null;
  codeIntent: number;
}

export function focusCodeFile(state: CodeTabsState, span: Span | null, revision?: string) {
  const codeTabs = span && !state.codeTabs.includes(span.file) ? [...state.codeTabs, span.file] : state.codeTabs;
  return {
    codeTabs,
    activeCodeFile: span?.file ?? state.activeCodeFile,
    codeFocus: span ? { ...span } : null,
    codeFocusRevision: span ? revision ?? null : null,
    codeIntent: state.codeIntent + 1,
  };
}

export function closeCodeTabs(state: CodeTabsState, file: string, mode: CloseCodeTabs) {
  const { codeTabs, activeCodeFile } = state;
  const anchor = codeTabs.indexOf(file);
  if (anchor < 0) return state;
  const remaining = codeTabs.filter((path, index) => {
    switch (mode) {
      case "one": return path !== file;
      case "other": return path === file;
      case "left": return index >= anchor;
      case "right": return index <= anchor;
      case "all":
      case "unmodified": return false;
    }
  });
  if (remaining.length === codeTabs.length) return state;
  const updated = { codeTabs: remaining, codeIntent: state.codeIntent + 1 };
  if (activeCodeFile && remaining.includes(activeCodeFile)) return updated;
  const activeIndex = activeCodeFile ? codeTabs.indexOf(activeCodeFile) : -1;
  const survivors = new Set(remaining);
  const next = codeTabs.slice(activeIndex + 1).find((path) => survivors.has(path)) ?? remaining.at(-1) ?? null;
  return { ...updated, activeCodeFile: next, codeFocus: null, codeFocusRevision: null };
}

export function moveCodeTab(tabs: string[], file: string, target: string, after: boolean) {
  if (file === target || !tabs.includes(file) || !tabs.includes(target)) return tabs;
  const next = tabs.filter((path) => path !== file);
  next.splice(next.indexOf(target) + Number(after), 0, file);
  return next.every((path, i) => path === tabs[i]) ? tabs : next;
}
