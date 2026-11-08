import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type UnityPackageAction } from "../api";
import { useDebug } from "../debug";

/** Unity projects: the Nodex package (Link / Embed / Update / Remove), the open Editor, and
 *  whether the scripts carry trace probes. Run and Debug happen in that Editor. */
export function UnityCard() {
  const queryClient = useQueryClient();
  // The Editor changes on its own (compiling, Play Mode): follow it.
  const status = useQuery({ queryKey: ["unity"], queryFn: api.unity, refetchInterval: 2000 });
  const act = useMutation({
    mutationFn: (action: UnityPackageAction) => api.unityPackage(action),
    onSettled: () => void queryClient.invalidateQueries({ queryKey: ["unity"] }),
  });
  const traceOff = useMutation({
    mutationFn: api.unityTraceOff,
    onSettled: () => void queryClient.invalidateQueries({ queryKey: ["unity"] }),
  });
  // A breakpoint freezes the Editor: its bridge stops reporting until the program continues.
  const debugPaused = useDebug((d) => d.status.state === "paused");
  const s = status.data;
  if (!s) return null;
  const busy = act.isPending;
  const editor = s.editor;
  const installed = s.package !== "missing";

  return (
    <div className="unity-card">
      <div className="section-title">Unity</div>
      {!installed ? (
        <>
          <p>
            Run and Debug need the <strong>Nodex package</strong> in this project ({s.bundled ?? "?"}). It adds trace probes to your scripts
            only while Nodex runs them.
          </p>
          <div className="unity-actions">
            <button className="primary" disabled={busy} onClick={() => act.mutate("embed")} title="Copies the package into Packages/com.nodex.trace, so it travels with the project and keeps working after Nodex updates.">
              Install
            </button>
            <button disabled={busy} onClick={() => act.mutate("link")} title="Adds one line to Packages/manifest.json that points at Nodex's copy of the package. Nothing is copied; the path is this computer's and changes when Nodex is updated or moved.">
              Link instead
            </button>
          </div>
        </>
      ) : (
        <div className="unity-row">
          <span>
            Package{" "}
            {s.package === "linked" ? (
              <span title={s.installed ?? ""}>linked to Nodex</span>
            ) : (
              <>
                embedded {s.installed}
                {s.package === "outdated" && <span className="unity-warn"> · Nodex has {s.bundled}</span>}
              </>
            )}
          </span>
          <span className="unity-actions">
            {s.package === "outdated" && (
              <button disabled={busy} onClick={() => act.mutate("update")}>
                Update
              </button>
            )}
            <button disabled={busy} onClick={() => act.mutate("remove")} title="Removes the package (and its manifest line) from the project.">
              Remove
            </button>
          </span>
        </div>
      )}
      {act.error && <div className="open-error">{(act.error as Error).message.replace(/^\d+ /, "")}</div>}

      <div className="unity-row">
        {editor ? (
          <span>
            Unity {editor.version} is open
            {editor.bridge ? (
              <span className="muted">
                {" "}
                · {editor.compiling ? "compiling…" : editor.playing ? "in Play Mode" : "in Edit Mode"}
                {editor.code_optimization === "Debug" ? " · Debug mode" : ""}
              </span>
            ) : debugPaused ? (
              <span className="muted"> · paused in the debugger</span>
            ) : installed ? (
              <span className="muted"> · loading the Nodex package…</span>
            ) : null}
          </span>
        ) : (
          <span className="muted">Open this project in the Unity Editor to run or debug it there.</span>
        )}
      </div>

      {s.trace_requested && (
        <div className="unity-row">
          <span className="muted">{editor?.trace_compiled ? "Scripts carry trace probes" : "Trace probes requested"}</span>
          <button disabled={traceOff.isPending} onClick={() => traceOff.mutate()} title="Recompiles the scripts without trace probes.">
            Turn tracing off
          </button>
        </div>
      )}
    </div>
  );
}
