import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { contextKey, contextLabel, type ContextItem } from "../graph/context";
import { fileTree, type FileNode } from "../graph/fileTree";
import { rankSymbols } from "../graph/search";
import { useUi } from "../store";
import { useStructure } from "../views/Views";

function FolderChoice({ folder, checked }: { folder: FileNode; checked: ReadonlySet<string> }) {
  const item: ContextItem = { kind: "folder", path: folder.path };
  const selected = checked.has(contextKey(item));
  const [open, setOpen] = useState(false);
  const subfolders = folder.children.filter((f) => f.folder);
  const toggle = () => {
    const ui = useUi.getState();
    if (selected) ui.removeFromContext(item);
    else ui.addToContext(item);
  };
  return (
    <li>
      <div className="context-folder-row">
        {subfolders.length ? (
          <button className="caret-btn" aria-expanded={open} aria-label={`${open ? "Collapse" : "Expand"} ${folder.path}`} onClick={() => setOpen(!open)}>
            {open ? "▾" : "▸"}
          </button>
        ) : <span className="context-folder-spacer" />}
        <label title={folder.path}><input type="checkbox" checked={selected} onChange={toggle} /> {folder.name}/</label>
      </div>
      {open && (
        <ul>{subfolders.map((f) => <FolderChoice key={f.path} folder={f} checked={checked} />)}</ul>
      )}
    </li>
  );
}

export function ContextPicker() {
  const { index } = useStructure();
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  const items = useUi((s) => s.context);
  const [query, setQuery] = useState("");
  const close = () => useUi.getState().setContextPickerOpen(false);
  const folders = useMemo(() => fileTree(project.data?.files.map((f) => f.path) ?? [], false).filter((f) => f.folder), [project.data]);
  const symbols = useMemo(() => rankSymbols(index?.graph.symbols.filter((s) => !s.external) ?? [], query), [index, query]);
  const checked = new Set(items.map(contextKey));
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="modal-backdrop palette-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className="modal context-picker" role="dialog" aria-modal="true" aria-label="Project context">
        <div className="modal-head">
          <strong>Project context</strong>
          <button className="modal-close" aria-label="Close context picker" onClick={close}>×</button>
        </div>
        <div className="context-picker-body">
          <div className="context-picked">
            <button onClick={() => useUi.getState().clearContext()} aria-pressed={items.length === 0}>Whole project</button>
            {items.map((item) => {
              const label = contextLabel([item], (id) => index?.byId.get(id)?.name);
              return (
                <span key={contextKey(item)} className="chip" title={item.kind === "folder" ? item.path : item.id}>
                  {label}
                  <button className="chip-x" aria-label={`Remove ${label} from context`} onClick={() => useUi.getState().removeFromContext(item)}>×</button>
                </span>
              );
            })}
          </div>
          <div className="context-picker-columns">
            <section>
              <h3>Folders</h3>
              {folders.length ? <ul className="context-folders">{folders.map((f) => <FolderChoice key={f.path} folder={f} checked={checked} />)}</ul> : <p className="muted">No folders</p>}
            </section>
            <section>
              <h3>Symbols</h3>
              <input className="search" aria-label="Search symbols" placeholder="Search symbols…" value={query} onChange={(e) => setQuery(e.target.value)} autoFocus />
              {symbols.map((symbol) => {
                const item: ContextItem = { kind: "symbol", id: symbol.id };
                return (
                  <button key={symbol.id} className="context-symbol" disabled={checked.has(contextKey(item))} onClick={() => useUi.getState().addToContext(item)} title={symbol.qualified_name}>
                    <strong>{symbol.name}</strong><span className="muted">{symbol.qualified_name}</span>
                  </button>
                );
              })}
              {query.trim().length >= 2 && !symbols.length && <p className="muted">No matching symbols</p>}
            </section>
          </div>
        </div>
      </div>
    </div>
  );
}
