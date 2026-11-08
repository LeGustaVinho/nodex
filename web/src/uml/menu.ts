import type { MenuItem } from "../components/ContextMenu";
import type { UmlScope } from "../gen/UmlScope";
import { useUi } from "../store";
import { UML_KINDS } from "./uml";

/** "UML diagram" items for a scope: the ready diagrams, then the ones still to come (disabled). */
export function umlMenuItems(scope: UmlScope, title = "UML diagram"): MenuItem[] {
  const items: MenuItem[] = [{ label: title, header: true }];
  for (const k of UML_KINDS.filter((k) => k.ready)) {
    items.push({ label: k.label, hint: k.question, run: () => useUi.getState().showUml(scope, k.id) });
  }
  for (const k of UML_KINDS.filter((k) => !k.ready)) {
    items.push({ label: k.label, hint: "Coming in a later milestone", disabled: true });
  }
  return items;
}

/** The UML scope of a Hierarchy box or a symbol id (folders and namespace groups included). */
export function scopeOfId(id: string): UmlScope {
  return { kind: "Symbol", id };
}
