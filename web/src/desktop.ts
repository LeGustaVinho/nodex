/**
 * Native features of the Nodex desktop app, with browser fallbacks: the same UI runs in the
 * desktop window and in a browser opened by `nodex open`.
 *
 * The desktop app grants this page a few Tauri commands (its folder picker and Save dialog);
 * they are called through `window.__TAURI__.core.invoke`. Links with `target="_blank"` and
 * `navigator.clipboard` need nothing here: the app opens new windows in the system browser,
 * and the clipboard API works on the local origin.
 */

import { isDesktop } from "./api";

type Invoke = (cmd: string, args?: unknown, options?: { headers?: Record<string, string> }) => Promise<unknown>;

function tauriInvoke(): Invoke | undefined {
  return (window as unknown as { __TAURI__?: { core?: { invoke?: Invoke } } }).__TAURI__?.core?.invoke;
}

/** True when native dialogs are available (inside the desktop app). */
export function hasNativeDialogs(): boolean {
  return isDesktop && tauriInvoke() !== undefined;
}

/** Asks for a folder with the system's folder picker. `null` when cancelled or unavailable. */
export async function pickFolder(title = "Open a project folder"): Promise<string | null> {
  const invoke = tauriInvoke();
  if (!invoke) return null;
  const picked = await invoke("plugin:dialog|open", { options: { directory: true, multiple: false, title } });
  return typeof picked === "string" ? picked : null;
}

/** Looks for a newer Nodex now; the desktop app answers with its own dialogs. */
export async function checkForUpdates(): Promise<void> {
  const invoke = tauriInvoke();
  if (invoke) await invoke("check_for_updates");
}

/**
 * Saves a file: a native Save dialog in the desktop app, a download in a browser. Returns the
 * saved path in the desktop app (`null` when cancelled), `undefined` in a browser.
 */
export async function saveFile(name: string, type: string, content: Blob | string): Promise<string | null | undefined> {
  const blob = typeof content === "string" ? new Blob([content], { type }) : content;
  const invoke = tauriInvoke();
  if (isDesktop && invoke) {
    const bytes = new Uint8Array(await blob.arrayBuffer());
    const saved = await invoke("save_export", bytes, { headers: { "x-file-name": encodeURIComponent(name) } });
    return typeof saved === "string" ? saved : null;
  }
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return undefined;
}
