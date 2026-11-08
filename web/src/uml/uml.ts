import type { UmlKind } from "../gen/UmlKind";
import type { UmlScope } from "../gen/UmlScope";

/** The seven diagrams, in the order of the questions they answer. */
export const UML_KINDS: { id: UmlKind; label: string; question: string; ready: boolean }[] = [
  { id: "UseCase", label: "Use cases", question: "What does the system let people do?", ready: true },
  { id: "Component", label: "Components", question: "What are the main modules?", ready: true },
  { id: "Class", label: "Classes", question: "How is the code structured?", ready: true },
  { id: "Sequence", label: "Sequence", question: "How do the objects interact?", ready: true },
  { id: "Activity", label: "Activity", question: "How does the logic run?", ready: true },
  { id: "State", label: "States", question: "How does behavior change?", ready: true },
  { id: "Package", label: "Packages", question: "How is the code organized?", ready: true },
];

export function kindLabel(kind: UmlKind): string {
  return UML_KINDS.find((k) => k.id === kind)?.label ?? kind;
}

/** Stable key of a scope for queries and caches. */
export function scopeKey(scope: UmlScope): string {
  return JSON.stringify(scope);
}

/** Short label of a scope for chips and menus. */
export function scopeLabel(scope: UmlScope, name?: (id: string) => string | undefined): string {
  switch (scope.kind) {
    case "Project":
      return "whole project";
    case "Folder":
      return scope.path ? `${scope.path}/` : "whole project";
    case "File":
      return scope.path.split("/").pop() ?? scope.path;
    case "Range":
      return `selection in ${scope.file.split("/").pop()}`;
    case "Symbol":
      if (scope.id.startsWith("dir:")) return `${scope.id.slice(4)}/`;
      if (scope.id.startsWith("grp:ns:")) return scope.id.slice(7);
      return name?.(scope.id) ?? scope.id;
    case "Symbols":
      return `${scope.ids.length} symbols`;
  }
}

/**
 * The element id (`n12`) inside an id Mermaid gives a drawn node (`flowchart-n12-3`,
 * `mermaid-1-classId-n12-0`), or null.
 */
export function elementIdOf(domId: string): string | null {
  const m = /(?:^|[-_])(n\d+)(?=$|[-_])/.exec(domId);
  return m ? m[1] : null;
}
