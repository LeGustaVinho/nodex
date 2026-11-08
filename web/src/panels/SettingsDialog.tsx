import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api, isDesktop } from "../api";
import { checkForUpdates, hasNativeDialogs } from "../desktop";
import type { Settings } from "../gen/Settings";
import { useUi } from "../store";
import { errorText, toast } from "../toast";
import { useEnvironment } from "./EnvironmentDialog";

const REPOSITORY = "https://github.com/LeGustaVinho/nodex";

/** Opens Settings from the desktop menu (File → Settings…), which sends `nodex:settings`. */
export function useSettingsMenu() {
  useEffect(() => {
    const open = () => useUi.getState().setSettingsOpen(true);
    window.addEventListener("nodex:settings", open);
    return () => window.removeEventListener("nodex:settings", open);
  }, []);
}

export function SettingsDialog() {
  const open = useUi((s) => s.settingsOpen);
  const setOpen = useUi((s) => s.setSettingsOpen);
  if (!open) return null;
  return <SettingsModal onClose={() => setOpen(false)} />;
}

function SettingsModal({ onClose }: { onClose: () => void }) {
  const client = useQueryClient();
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
  const env = useEnvironment();
  const [checking, setChecking] = useState(false);
  const save = useMutation({
    mutationFn: (s: Settings) => api.saveSettings(s),
    onSuccess: (s) => client.setQueryData(["settings"], s),
    onError: (e) => toast.error(`Could not save the settings: ${errorText(e)}`),
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const check = async () => {
    setChecking(true);
    try {
      await checkForUpdates();
    } catch (e) {
      toast.error(`Could not check for updates: ${errorText(e)}`);
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal settings-dialog" role="dialog" aria-label="Settings">
        <div className="modal-head">
          <strong>Settings</strong>
          <button className="modal-close" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </div>
        <div className="settings-body">
          {isDesktop && (
            <section>
              <div className="section-title">Updates</div>
              <label className="settings-row">
                <input
                  type="checkbox"
                  disabled={!settings.data || save.isPending}
                  checked={settings.data?.check_updates ?? true}
                  onChange={(e) => settings.data && save.mutate({ ...settings.data, check_updates: e.target.checked })}
                />
                Check for a new version when Nodex starts
              </label>
              <div className="settings-row">
                <button disabled={checking || !hasNativeDialogs()} onClick={() => void check()}>
                  {checking ? "Checking…" : "Check now"}
                </button>
                <span className="muted small">Updates come from the project's GitHub releases and are signed.</span>
              </div>
            </section>
          )}
          <section>
            <div className="section-title">About</div>
            <div className="settings-facts">
              <span className="muted">Version</span>
              <span>Nodex {env.data?.version ?? "…"}</span>
              <span className="muted">License</span>
              <span>MIT</span>
              <span className="muted">Source</span>
              <a href={REPOSITORY} target="_blank" rel="noreferrer">
                {REPOSITORY.replace("https://", "")}
              </a>
              <span className="muted">Logs</span>
              <span className="mono small">{env.data?.logs ?? "…"}</span>
              <span className="muted">Privacy</span>
              <span>Your code never leaves this computer. Nodex goes online only to check for updates and to download a debug adapter you install.</span>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
