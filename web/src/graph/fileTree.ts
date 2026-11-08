/** A folder or a file of the project, for the Files tree. */
export interface FileNode {
  /** Shown name; folders with a single subfolder are joined (`src/main/java`). */
  name: string;
  /** Project-relative path. */
  path: string;
  folder: boolean;
  children: FileNode[];
  /** Files below (1 for a file). */
  files: number;
}

/** Builds the folder tree of project-relative file paths: folders first, then files, by name. */
export function fileTree(paths: string[]): FileNode[] {
  const root: FileNode = { name: "", path: "", folder: true, children: [], files: 0 };
  for (const p of paths) {
    const parts = p.split("/").filter(Boolean);
    let cur = root;
    cur.files++;
    parts.forEach((part, i) => {
      const path = parts.slice(0, i + 1).join("/");
      const isFile = i === parts.length - 1;
      let next = cur.children.find((c) => c.name === part && c.folder === !isFile);
      if (!next) {
        next = { name: part, path, folder: !isFile, children: [], files: 0 };
        cur.children.push(next);
      }
      next.files++;
      cur = next;
    });
  }
  const tidy = (n: FileNode): FileNode => {
    let node = n;
    // `a` → `b` with nothing else in `a`: one row `a/b`.
    while (node.folder && node.children.length === 1 && node.children[0].folder) {
      const only = node.children[0];
      node = { ...only, name: node.name ? `${node.name}/${only.name}` : only.name };
    }
    node.children = node.children.map(tidy).sort((a, b) => Number(b.folder) - Number(a.folder) || a.name.localeCompare(b.name));
    return node;
  };
  return tidy(root).children;
}
