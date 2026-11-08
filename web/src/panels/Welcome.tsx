import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { api, isDesktop } from "../api";
import { hasNativeDialogs, pickFolder } from "../desktop";
import { folderName, kindLabel } from "../paths";
import { useUi } from "../store";
import { errorText } from "../toast";
import { ToolRow, useEnvironment } from "./EnvironmentDialog";
import { OpenDialog } from "./OpenDialog";

/** Two Blueprint nodes joined by a wire: the app icon. */
export function Logo({ size = 56 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" className="logo-mark">
      <rect x="2" y="2" width="60" height="60" rx="13" fill="var(--panel)" stroke="var(--line)" strokeWidth="1.5" />
      <rect x="8" y="14" width="22" height="18" rx="3" fill="var(--panel-2)" stroke="#464e5c" />
      <rect x="8" y="14" width="22" height="5" rx="2" fill="var(--accent-ui)" />
      <rect x="34" y="32" width="22" height="18" rx="3" fill="var(--panel-2)" stroke="#464e5c" />
      <rect x="34" y="32" width="22" height="5" rx="2" fill="var(--unity)" />
      <path d="M30 23 C 40 23, 24 41, 34 41" stroke="var(--text)" strokeWidth="2" fill="none" />
    </svg>
  );
}

/** The start screen when no folder is open: open one, reopen a recent one, check the tools. */
export function Welcome() {
  const recent = useQuery({ queryKey: ["recent"], queryFn: api.recent });
  const samples = useQuery({ queryKey: ["samples"], queryFn: api.samples });
  const env = useEnvironment();
  const loading = useUi((s) => s.projectLoading);
  const setEnvironmentOpen = useUi((s) => s.setEnvironmentOpen);
  const [browsing, setBrowsing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const native = hasNativeDialogs();

  const open = async (path: string) => {
    setError(null);
    try {
      await api.openProject(path);
    } catch (e) {
      setError(errorText(e));
    }
  };
  const openSample = async (id: string) => {
    setError(null);
    try {
      await api.openSample(id);
    } catch (e) {
      setError(errorText(e));
    }
  };
  const openFolder = async () => {
    if (!native) {
      setBrowsing(true);
      return;
    }
    try {
      const picked = await pickFolder();
      if (picked) await open(picked);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const toolchains = (env.data?.tools ?? []).filter((t) => t.kind === "toolchain");
  const missing = toolchains.filter((t) => !t.found).length;

  return (
    <div className="welcome">
      <div className="welcome-inner">
        <header className="welcome-hero">
          <Logo />
          <div>
            <h1>Nodex</h1>
            <p className="muted">Read code in any language as node graphs: C# and Unity, TypeScript and JavaScript, Rust.</p>
          </div>
          <button className="icon-button welcome-settings" onClick={() => useUi.getState().setSettingsOpen(true)} title="Settings" aria-label="Settings">
            ⚙
          </button>
        </header>

        <section className="welcome-actions">
          {loading ? (
            <div className="welcome-loading">Opening {folderName(loading)} …</div>
          ) : (
            <>
              <button className="primary big" onClick={() => void openFolder()}>
                Open Folder…
              </button>
              {native && <button onClick={() => setBrowsing(true)}>Browse or paste a path…</button>}
              <span className="muted small">
                {isDesktop ? "or drop a project folder on this window" : "A .NET solution, a Unity project, a package.json or a Cargo project"}
              </span>
            </>
          )}
          {error && <div className="open-error">{error}</div>}
        </section>

        <div className="welcome-columns">
          <section>
            <div className="section-title">Recent</div>
            {recent.data && recent.data.length === 0 && (samples.data ?? []).length === 0 && (
              <div className="empty-note">Folders you open appear here.</div>
            )}
            {(samples.data ?? []).length > 0 && (recent.data ?? []).length < 3 && (
              <div className="welcome-samples">
                <div className="muted small">New to Nodex? Start with a sample:</div>
                {(samples.data ?? []).map((s) => (
                  <button key={s.id} className="welcome-sample" disabled={!!loading} onClick={() => void openSample(s.id)}>
                    <strong>{s.name}</strong>
                    <span className="muted small">{s.description}</span>
                  </button>
                ))}
              </div>
            )}
            {(recent.data ?? []).map((r) => (
              <button key={r.path} className="welcome-recent" disabled={!!loading} onClick={() => void open(r.path)} title={r.path}>
                <span className="recent-name">{r.name || folderName(r.path)}</span>
                {r.languages.map((l) => (
                  <span key={l} className={`badge small kind-${l}`}>
                    {kindLabel(l)}
                  </span>
                ))}
                <span className="muted recent-path">{r.path}</span>
              </button>
            ))}
          </section>
          <section>
            <div className="section-title">
              Your tools
              {env.data && <span className="muted small">{missing === 0 ? " all found" : ` ${missing} missing`}</span>}
            </div>
            {env.isLoading && <div className="empty-note">Checking…</div>}
            {toolchains.map((t) => (
              <ToolRow key={t.id} tool={t} compact />
            ))}
            <button className="link-button" onClick={() => setEnvironmentOpen(true)}>
              What each tool enables…
            </button>
          </section>
        </div>
      </div>
      {browsing && <OpenDialog onClose={() => setBrowsing(false)} />}
    </div>
  );
}
