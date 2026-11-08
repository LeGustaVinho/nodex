/** Folder-path helpers for the Open dialog (Windows `D:\a\b` and POSIX `/a/b` paths). */

export interface Crumb {
  label: string;
  path: string;
}

const isWindows = (p: string) => /^[A-Za-z]:/.test(p) || p.includes("\\");

/** The folders from the top to `path`: `D:\a\b` → `D:\`, `D:\a`, `D:\a\b`. */
export function breadcrumbs(path: string): Crumb[] {
  const p = path.trim();
  if (!p) return [];
  if (isWindows(p)) {
    const parts = p.replace(/[\\/]+$/, "").split(/[\\/]+/).filter(Boolean);
    const out: Crumb[] = [];
    let acc = "";
    parts.forEach((part, i) => {
      acc = i === 0 ? `${part}\\` : `${acc.replace(/\\$/, "")}\\${part}`;
      out.push({ label: i === 0 ? `${part}\\` : part, path: acc });
    });
    return out;
  }
  const parts = p.split("/").filter(Boolean);
  const out: Crumb[] = [{ label: "/", path: "/" }];
  let acc = "";
  for (const part of parts) {
    acc = `${acc}/${part}`;
    out.push({ label: part, path: acc });
  }
  return out;
}

/** The last folder name of a path (for titles). */
export function folderName(path: string): string {
  const parts = path.replace(/[\\/]+$/, "").split(/[\\/]+/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

const KIND_LABEL: Record<string, string> = { csharp: "C#", unity: "Unity", typescript: "TS/JS", rust: "Rust" };

/** Short badge text for a detected project kind. */
export function kindLabel(kind: string): string {
  return KIND_LABEL[kind] ?? kind;
}
