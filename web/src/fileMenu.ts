import type { MenuItem } from "./components/ContextMenu";
import { hasNativeDialogs, projectFileAction } from "./desktop";
import type { Symbol } from "./gen/Symbol";
import { useToasts } from "./toast";

export function absoluteFilePath(root: string, relative: string): string {
  const windows = /^[A-Za-z]:[\\/]|^\\\\/.test(root);
  if (!windows) return root.replace(/\/+$/, "") + "/" + relative;
  const base = root.replace(/^\\\\\?\\UNC\\/i, "\\\\").replace(/^\\\\\?\\(?=[A-Za-z]:)/, "");
  return base.replace(/[\\/]+$/, "") + "\\" + relative.replaceAll("/", "\\");
}

async function copyPath(path: string) {
  try {
    await navigator.clipboard.writeText(path);
    useToasts.getState().push("info", "Path copied.");
  } catch {
    useToasts.getState().push("error", "Could not copy the file path.");
  }
}

function requireProject(root: string | undefined, run: (root: string) => void) {
  if (root) run(root);
  else useToasts.getState().push("error", "The active project is unavailable.");
}

export function fileMenuItems(relativePath: string): MenuItem[] {
  const native = hasNativeDialogs();
  const nativeItem = (label: string, operation: "open" | "reveal"): MenuItem => ({
    label,
    disabled: !native,
    hint: native ? undefined : "Requires Nodex Desktop",
    run: (root) => requireProject(root, (expectedRoot) => {
      void projectFileAction(operation, expectedRoot, relativePath).catch((error: unknown) => {
        const detail = error instanceof Error ? error.message : String(error);
        useToasts.getState().push("error", "Could not " + operation + " the file. " + detail);
      });
    }),
  });
  return [
    nativeItem("Reveal in File Explorer", "reveal"),
    nativeItem("Open", "open"),
    {
      label: "Copy path",
      children: [
        { label: "Relative", run: () => void copyPath(relativePath) },
        { label: "Absolute", run: (root) => requireProject(root, (base) => void copyPath(absoluteFilePath(base, relativePath))) },
      ],
    },
  ];
}

export function symbolFileMenuItems(symbol: Symbol): MenuItem[] {
  if (symbol.external) return [];
  const paths = [...new Set(symbol.origins.map((origin) => origin.file).filter(Boolean))];
  if (paths.length === 0) return [];
  if (paths.length === 1) return fileMenuItems(paths[0]);
  return [{ label: "Source file", children: paths.map((path) => ({ label: path, children: fileMenuItems(path) })) }];
}
