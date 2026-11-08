import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../api";
import type { FolderEntry } from "../gen/FolderEntry";
import { breadcrumbs, folderName, kindLabel } from "../paths";
import { useUi } from "../store";

function Kinds({ kinds }: { kinds: string[] }) {
  return (
    <>
      {kinds.map((k) => (
        <span key={k} className={`badge small kind-${k}`}>
          {kindLabel(k)}
        </span>
      ))}
    </>
  );
}

/** Choose a folder (browse, paste a path, or pick a recent project) and open it. */
export function OpenDialog({ onClose }: { onClose: () => void }) {
  const current = useQuery({ queryKey: ["project"], queryFn: api.project });
  const [path, setPath] = useState<string>("");
  const [typed, setTyped] = useState<string>("");
  const loading = useUi((s) => s.projectLoading);
  const [error, setError] = useState<string | null>(null);
  // Start next to the open project.
  useEffect(() => {
    const root = current.data?.root;
    if (root && !path) {
      const parent = root.replace(/[\\/][^\\/]+[\\/]?$/, "");
      setPath(parent || root);
      setTyped(parent || root);
    }
    // Only for the first render with data.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current.data?.root]);
  const listing = useQuery({ queryKey: ["fs", path], queryFn: () => api.listFolders(path), retry: false });
  const recent = useQuery({ queryKey: ["recent"], queryFn: api.recent });

  const go = (p: string) => {
    setPath(p);
    setTyped(p);
    setError(null);
  };
  const open = async (p: string) => {
    setError(null);
    try {
      await api.openProject(p);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message.replace(/^\d+ /, "") : String(e));
    }
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !loading) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, loading]);

  const entries: FolderEntry[] = listing.data?.entries ?? [];
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && !loading && onClose()}>
      <div className="modal open-dialog" role="dialog" aria-label="Open a project">
        <div className="modal-head">
          <strong>Open a project</strong>
          <button className="modal-close" onClick={onClose} disabled={!!loading} aria-label="Close">
            ✕
          </button>
        </div>
        <form
          className="open-path"
          onSubmit={(e) => {
            e.preventDefault();
            go(typed);
          }}
        >
          <input
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            placeholder="Paste a folder path, for example D:\Projects\MyGame"
            spellCheck={false}
            aria-label="Folder path"
          />
          <button type="submit">Go</button>
        </form>
        <div className="crumbs">
          <button className="crumb-btn" onClick={() => go("")} title="Drives and home folder">
            ⌂
          </button>
          {breadcrumbs(listing.data?.path ?? path).map((c) => (
            <button key={c.path} className="crumb-btn" onClick={() => go(c.path)}>
              {c.label}
            </button>
          ))}
          {listing.data?.kinds && listing.data.kinds.length > 0 && <Kinds kinds={listing.data.kinds} />}
        </div>
        <div className="open-body">
          <div className="folder-list">
            {listing.error && <div className="empty-note">{String((listing.error as Error).message).replace(/^\d+ /, "")}</div>}
            {listing.data?.path === "" && listing.data.home && (
              <button className="folder-row" onClick={() => go(listing.data!.home!)}>
                <span className="folder-icon">⌂</span> Home <span className="muted">{listing.data.home}</span>
              </button>
            )}
            {listing.data?.parent !== null && listing.data?.parent !== undefined && listing.data.path !== "" && (
              <button className="folder-row" onClick={() => go(listing.data!.parent!)}>
                <span className="folder-icon">↰</span> ..
              </button>
            )}
            {entries.map((e) => (
              <button
                key={e.path}
                className={`folder-row ${e.kinds.length ? "is-project" : ""}`}
                onClick={() => go(e.path)}
                onDoubleClick={() => e.kinds.length && void open(e.path)}
                title={e.kinds.length ? "Double-click to open this project" : e.path}
              >
                <span className="folder-icon">▤</span> {e.name} <Kinds kinds={e.kinds} />
              </button>
            ))}
            {listing.data && entries.length === 0 && listing.data.path !== "" && <div className="empty-note">No subfolders.</div>}
          </div>
          <div className="recent-list">
            <div className="section-title">Recent</div>
            {(recent.data ?? []).map((r) => (
              <button key={r.path} className="folder-row" onClick={() => void open(r.path)} title={r.path}>
                <span className="recent-name">{r.name || folderName(r.path)}</span>
                <span className="muted recent-path">{r.path}</span>
              </button>
            ))}
            {recent.data && recent.data.length === 0 && <div className="empty-note">Projects you open appear here.</div>}
          </div>
        </div>
        {error && <div className="open-error">{error}</div>}
        <div className="modal-foot">
          {loading ? (
            <span className="muted">Loading {folderName(loading)} …</span>
          ) : (
            <span className="muted">{listing.data?.path ? listing.data.path : "Choose a folder"}</span>
          )}
          <button className="primary" disabled={!listing.data?.path || !!loading} onClick={() => listing.data?.path && void open(listing.data.path)}>
            {loading ? "Opening…" : "Open this folder"}
          </button>
        </div>
      </div>
    </div>
  );
}
