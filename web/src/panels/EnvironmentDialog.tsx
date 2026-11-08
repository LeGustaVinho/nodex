import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { api } from "../api";
import type { ToolStatus } from "../gen/ToolStatus";
import { useUi } from "../store";

/** The environment report: checks start processes, so it is kept for a minute. */
export function useEnvironment() {
  return useQuery({ queryKey: ["environment"], queryFn: api.environment, staleTime: 60_000 });
}

/** One toolchain or bundled helper: found (with its version) or missing (with how to get it). */
export function ToolRow({ tool, compact = false }: { tool: ToolStatus; compact?: boolean }) {
  return (
    <div className={`tool-row ${tool.found ? "found" : "missing"}`}>
      <span className="tool-mark" aria-label={tool.found ? "installed" : "missing"}>
        {tool.found ? "✓" : "○"}
      </span>
      <div className="tool-main">
        <div>
          <strong>{tool.name}</strong> {tool.version && <span className="muted tool-version">{tool.version}</span>}
        </div>
        {!compact && <div className="tool-enables">{tool.enables}</div>}
        {!tool.found && tool.hint && <div className="tool-hint">{tool.hint}</div>}
      </div>
      {!tool.found && tool.install_url && (
        <a className="tool-get" href={tool.install_url} target="_blank" rel="noreferrer" title={tool.install_url}>
          Get it ↗
        </a>
      )}
    </div>
  );
}

/** What this machine has for each language, and what is missing. */
export function EnvironmentDialog() {
  const open = useUi((s) => s.environmentOpen);
  const setOpen = useUi((s) => s.setEnvironmentOpen);
  if (!open) return null;
  return <EnvironmentModal onClose={() => setOpen(false)} />;
}

function EnvironmentModal({ onClose }: { onClose: () => void }) {
  const env = useEnvironment();
  const client = useQueryClient();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  const tools = env.data?.tools ?? [];
  const toolchains = tools.filter((t) => t.kind === "toolchain");
  const bundled = tools.filter((t) => t.kind === "bundled");
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal environment-dialog" role="dialog" aria-label="Environment">
        <div className="modal-head">
          <strong>Environment</strong>
          <button className="modal-close" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </div>
        <div className="environment-body">
          <p className="muted">
            Nodex reads every project without extra tools. These toolchains add exact (compiler-grade) analysis, ▶ Run with live traces and
            debugging for their language. After installing one, open the folder again.
          </p>
          {env.isLoading && <div className="empty-note">Checking the installed tools…</div>}
          {env.isError && <div className="open-error">Could not check the tools: {String(env.error)}</div>}
          {toolchains.length > 0 && (
            <>
              <div className="section-title">Toolchains on this computer</div>
              {toolchains.map((t) => (
                <ToolRow key={t.id} tool={t} />
              ))}
            </>
          )}
          {bundled.length > 0 && (
            <>
              <div className="section-title">Included with Nodex</div>
              {bundled.map((t) => (
                <ToolRow key={t.id} tool={t} />
              ))}
            </>
          )}
        </div>
        <div className="modal-foot">
          <span className="muted small" title={env.data?.logs}>
            {env.data && `Nodex ${env.data.version} · logs in ${env.data.logs}`}
          </span>
          <button disabled={env.isFetching} onClick={() => void client.invalidateQueries({ queryKey: ["environment"] })}>
            {env.isFetching ? "Checking…" : "Check again"}
          </button>
        </div>
      </div>
    </div>
  );
}
