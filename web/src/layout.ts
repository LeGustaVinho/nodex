/**
 * Panel sizes the user chose by dragging a splitter. Each size is a CSS variable on the root
 * element (`--sidebar-w`, `--right-w`, …) that the stylesheet reads with its own default as the
 * fallback, so an unset size means "the default layout". Sizes are kept for every project in
 * localStorage; private windows or blocked storage simply start from the defaults.
 */

export type PanelSize = "sidebar-w" | "right-w" | "inspector-h" | "problems-h" | "console-h";

const KEY = "nodex.layout";

function load(): Partial<Record<PanelSize, number>> {
  try {
    const saved = JSON.parse(window.localStorage.getItem(KEY) ?? "{}");
    return saved && typeof saved === "object" ? saved : {};
  } catch {
    return {};
  }
}

const sizes = load();

function apply(name: PanelSize, px: number | undefined) {
  const style = document.documentElement.style;
  if (px === undefined) style.removeProperty(`--${name}`);
  else style.setProperty(`--${name}`, `${Math.round(px)}px`);
}

for (const [name, px] of Object.entries(sizes)) {
  if (typeof px === "number" && Number.isFinite(px)) apply(name as PanelSize, px);
}

/** Shows `px` for the panel at once; `persist` also keeps it for the next session. */
export function setPanelSize(name: PanelSize, px: number, persist = true) {
  apply(name, px);
  if (!persist) return;
  sizes[name] = Math.round(px);
  save();
}

/** Goes back to the default size of the panel. */
export function resetPanelSize(name: PanelSize) {
  apply(name, undefined);
  delete sizes[name];
  save();
}

function save() {
  try {
    window.localStorage.setItem(KEY, JSON.stringify(sizes));
  } catch {
    // Storage is unavailable: the size still applies to this session.
  }
}
