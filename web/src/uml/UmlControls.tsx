import type { Granularity } from "../gen/Granularity";
import type { MemberFilter } from "../gen/MemberFilter";
import { contextLabel } from "../graph/context";
import { useUi } from "../store";
import { useStructure } from "../views/Views";
import { UML_KINDS, scopeLabel } from "./uml";
import { useUmlDiagram } from "./useUmlDiagram";

/** UML view controls: the diagram kind, the scope, and the options of the kind. */
export function UmlControls() {
  const ui = useUi();
  const { index } = useStructure();
  const { availability, diagram } = useUmlDiagram();
  const opts = ui.umlOpts;
  const flowKind = ui.umlKind === "Activity" || ui.umlKind === "Sequence";
  const candidates = diagram.data?.kind === ui.umlKind ? diagram.data.candidates : [];
  return (
    <>
      <div className="segmented" role="group" aria-label="Diagram">
        {UML_KINDS.map((k) => {
          const a = availability.data?.kinds.find((x) => x.kind === k.id);
          const suggested = availability.data?.suggested === k.id;
          return (
            <button
              key={k.id}
              className={`${ui.umlKind === k.id ? "on" : ""} ${suggested ? "suggested" : ""}`}
              aria-pressed={ui.umlKind === k.id}
              disabled={a ? !a.available : !k.ready}
              title={[k.question, a?.reason, suggested ? "Suggested for this scope" : ""].filter(Boolean).join("\n")}
              onClick={() => ui.showUml(undefined, k.id)}
            >
              {k.label}
            </button>
          );
        })}
      </div>
      <span className="chip" title="What the diagram is about">
        {ui.umlExplicitScope ? scopeLabel(ui.umlScope, (id) => index?.byId.get(id)?.name) : contextLabel(ui.context, (id) => index?.byId.get(id)?.name)}
        {ui.umlExplicitScope && (
          <button className="chip-x" onClick={() => ui.showUml()} title="Use the project context" aria-label="Use the project context">
            ×
          </button>
        )}
      </span>
      {ui.umlKind === "Class" && (
        <label className="toggle" title="Members shown in each class">
          members
          <select value={opts.members ?? "Auto"} onChange={(e) => ui.setUmlOpts({ members: e.target.value as MemberFilter })}>
            <option value="Auto">auto</option>
            <option value="All">all</option>
            <option value="Public">public</option>
            <option value="None">none</option>
          </select>
        </label>
      )}
      {ui.umlKind === "Package" && (
        <label className="toggle" title="How code is grouped into packages">
          by
          <select value={opts.grouping ?? "Namespace"} onChange={(e) => ui.setUmlOpts({ grouping: e.target.value as "Namespace" | "Folder" })}>
            <option value="Namespace">namespace</option>
            <option value="Folder">folder</option>
          </select>
        </label>
      )}
      {ui.umlKind === "Class" && (
        <label className="toggle" title="Show the related types just outside the scope (dimmed)">
          <input type="checkbox" checked={opts.context_neighbors ?? true} onChange={(e) => ui.setUmlOpts({ context_neighbors: e.target.checked })} /> context
        </label>
      )}
      {ui.umlKind === "Sequence" && (
        <div className="segmented" role="group" aria-label="Sequence source">
          <button className={!opts.runtime ? "on" : ""} aria-pressed={!opts.runtime} onClick={() => ui.setUmlOpts({ runtime: false, focus: null })} title="Read the calls from the code">
            code
          </button>
          <button className={opts.runtime ? "on" : ""} aria-pressed={!!opts.runtime} onClick={() => ui.setUmlOpts({ runtime: true, focus: null })} title="Draw the calls a recorded run made (Run the program first)">
            recorded run
          </button>
        </div>
      )}
      {(flowKind || ui.umlKind === "State") && candidates.length > 1 && (
        <label
          className="toggle"
          title={
            ui.umlKind === "State"
              ? "The state machine to draw (declared machines and the strongest evidence first)"
              : opts.runtime && ui.umlKind === "Sequence"
                ? "The recorded call to start from"
                : "The function the diagram starts from (entry points and the functions with the most logic first)"
          }
        >
          {ui.umlKind === "State" ? "machine" : "from"}
          <select className="uml-candidates" value={opts.focus ?? candidates[0].id} onChange={(e) => ui.setUmlOpts({ focus: e.target.value })}>
            {candidates.map((c) => (
              <option key={c.id} value={c.id} title={c.detail ?? undefined}>
                {c.label}
                {c.detail ? ` — ${c.detail}` : ""}
              </option>
            ))}
          </select>
        </label>
      )}
      {ui.umlKind === "Activity" && (
        <>
          <label className="toggle" title="How much each action shows">
            detail
            <select value={opts.granularity ?? "Grouped"} onChange={(e) => ui.setUmlOpts({ granularity: e.target.value as Granularity })}>
              <option value="Grouped">grouped</option>
              <option value="Statement">every statement</option>
              <option value="Decisions">decisions only</option>
            </select>
          </label>
          <label className="toggle" title="Expand calls to project functions in place, this many levels deep">
            expand
            <select value={opts.depth ?? 0} onChange={(e) => ui.setUmlOpts({ depth: Number(e.target.value) })}>
              {[0, 1, 2, 3].map((d) => (
                <option key={d} value={d}>
                  {d === 0 ? "no calls" : `${d} level${d > 1 ? "s" : ""}`}
                </option>
              ))}
            </select>
          </label>
          <label className="toggle" title="Show the code instead of the plain-language description">
            <input type="checkbox" checked={opts.code_labels ?? false} onChange={(e) => ui.setUmlOpts({ code_labels: e.target.checked })} /> code
          </label>
        </>
      )}
      {ui.umlKind === "Sequence" && (
        <>
          <label className="toggle" title="How deep calls into project code are followed">
            depth
            <select value={opts.depth ?? 3} onChange={(e) => ui.setUmlOpts({ depth: Number(e.target.value) })}>
              {[0, 1, 2, 3, 4, 5, 6].map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
          </label>
          <label className="toggle" title="Show calls to libraries that are not infrastructure (logging, formatting, collections)">
            <input type="checkbox" checked={!(opts.hide_noise ?? true)} onChange={(e) => ui.setUmlOpts({ hide_noise: !e.target.checked })} /> library calls
          </label>
        </>
      )}
      {!flowKind && ui.umlKind !== "State" && ui.umlKind !== "UseCase" && (
        <label className="toggle" title="Show external types and packages">
          <input type="checkbox" checked={opts.include_external ?? false} onChange={(e) => ui.setUmlOpts({ include_external: e.target.checked })} /> external
        </label>
      )}
      <label className="toggle" title="Include test code">
        <input type="checkbox" checked={!(opts.hide_tests ?? true)} onChange={(e) => ui.setUmlOpts({ hide_tests: !e.target.checked })} /> tests
      </label>
    </>
  );
}
