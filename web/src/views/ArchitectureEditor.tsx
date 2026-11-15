import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../api";
import type { ArchitectureDiagram } from "../gen/ArchitectureDiagram";
import type { ArchitectureSettings } from "../gen/ArchitectureSettings";
import type { DeclaredElement } from "../gen/DeclaredElement";
import { useUi } from "../store";
import { errorText } from "../toast";

export function useArchitectureSettings() {
  const session = useUi((s) => s.projectSession);
  return useQuery({
    queryKey: ["architecture", session, "config"],
    queryFn: ({ signal }) => api.architectureSettings(signal),
  });
}

export function ArchitectureEditor({ diagram, close }: { diagram: ArchitectureDiagram; close: () => void }) {
  const settings = useArchitectureSettings();
  return (
    <div className="modal-backdrop">
      <div className="modal architecture-editor" role="dialog" aria-modal="true" aria-label="Edit architecture">
        <div className="modal-head">
          <strong>Edit architecture</strong>
          <button className="modal-close" aria-label="Close architecture editor" onClick={close}>✕</button>
        </div>
        {settings.data ? (
          <EditorForm initial={settings.data} diagram={diagram} close={close} />
        ) : settings.error ? (
          <div className="settings-body" role="alert">{errorText(settings.error)} <button onClick={() => void settings.refetch()}>Retry</button></div>
        ) : <p className="settings-body" role="status">Loading architecture settings…</p>}
      </div>
    </div>
  );
}

function applicationPath(id: string) {
  const [root, entry] = id.slice("app:".length).split("::");
  return entry ? entry.slice(entry.indexOf(":") + 1) : root;
}

function EditorForm({ diagram, initial, close }: { diagram: ArchitectureDiagram; initial: ArchitectureSettings; close: () => void }) {
  const [draft, setDraft] = useState(() => {
    const excludedApps = initial.config.excluded_apps.flatMap((id) => {
      const split = diagram.applications.filter((app) => app.id.startsWith(id + "::"));
      return split.length ? split.map((app) => app.id) : [id];
    });
    return { ...initial, config: { ...initial.config, excluded_apps: [...new Set(excludedApps)] } };
  });
  const client = useQueryClient();
  const session = useUi((s) => s.projectSession);
  const { config } = draft;
  const patch = (changes: Partial<typeof config>) => setDraft((s) => ({ ...s, config: { ...s.config, ...changes } }));
  const save = useMutation({
    mutationFn: api.saveArchitecture,
    onSuccess: (saved) => {
      if (useUi.getState().projectSession !== session) return;
      client.setQueryData(["architecture", session, "config"], saved);
      void client.invalidateQueries({ queryKey: ["architecture", session] });
      close();
    },
  });
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !save.isPending) close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close, save.isPending]);

  const apps = new Map(diagram.applications.map((n) => [n.id, n.name]));
  const splitRoots = new Set(diagram.applications.filter((app) => app.id.includes("::")).map((app) => app.path));
  for (const app of config.applications) {
    if (!splitRoots.has(app.root)) apps.set("app:" + app.root, app.name);
  }
  const included = Array.from(apps).filter(([id]) => !config.excluded_apps.includes(id));
  const stores = new Map(diagram.stores.filter((node) => node.provenance !== "Declared").map((n) => [n.id, n.name]));
  for (const store of config.stores) stores.set("store:" + store.id, store.name);
  const endpoints = new Map([["system", config.system_name || "Project system"], ...included]);
  for (const [id, name] of stores) {
    if (!config.excluded_stores.includes(id)) endpoints.set(id, name);
  }
  for (const element of diagram.nodes) {
    if (element.id.startsWith("host:")) endpoints.set(element.id, element.name);
  }
  for (const [prefix, items] of [["actor", config.actors], ["external", config.external_systems]] as const) {
    for (const item of items) endpoints.set(prefix + ":" + item.id, item.name);
  }

  return (
    <form className="architecture-form" onSubmit={(event) => { event.preventDefault(); save.mutate(draft); }}>
      <div className="settings-body">
        {draft.warning && <p role="alert" className="uml-warnings">{draft.warning}</p>}
        <fieldset disabled={save.isPending}>
          <legend>System boundary</legend>
          <label>System name<input placeholder="Use the project name" value={config.system_name} onChange={(e) => patch({ system_name: e.target.value })} /></label>
          {Array.from(apps, ([id, name]) => (
            <label className="architecture-check" key={id}>
              <input type="checkbox" checked={!config.excluded_apps.includes(id)} onChange={(e) => patch({
                excluded_apps: e.target.checked ? config.excluded_apps.filter((a) => a !== id) : [...config.excluded_apps, id],
              })} />
              <span>{name} <small className="muted">{applicationPath(id) || "(root)"}</small></span>
            </label>
          ))}
          {Array.from(stores, ([id, name]) => (
            <label className="architecture-check" key={id}>
              <input type="checkbox" checked={!config.excluded_stores.includes(id)} onChange={(e) => patch({
                excluded_stores: e.target.checked ? config.excluded_stores.filter((s) => s !== id) : [...config.excluded_stores, id],
              })} />
              <span>{name} <small className="muted">Data store</small></span>
            </label>
          ))}
        </fieldset>
        <fieldset disabled={save.isPending}>
          <legend>Declared applications</legend>
          {config.applications.map((item, i) => (
            <div className="architecture-fields" key={i}>
              <label>Name<input required value={item.name} onChange={(e) => patch({ applications: replace(config.applications, i, { name: e.target.value }) })} /></label>
              <label>Project-relative folder<input value={item.root} placeholder="Project root" onChange={(e) => patch({ applications: replace(config.applications, i, { root: e.target.value }) })} /></label>
              <label>Technology<input value={item.technology} onChange={(e) => patch({ applications: replace(config.applications, i, { technology: e.target.value }) })} /></label>
              <Remove label="application" run={() => patch({ applications: without(config.applications, i) })} />
            </div>
          ))}
          <button type="button" onClick={() => patch({ applications: [...config.applications, { root: "", name: "", technology: "" }] })}>Add application</button>
        </fieldset>
        <fieldset disabled={save.isPending}>
          <legend>Library membership</legend>
          {config.libraries.map((item, i) => (
            <div className="architecture-fields" key={i}>
              <Endpoint label="Application" value={item.application} options={new Map(included)} change={(application) => patch({ libraries: replace(config.libraries, i, { application }) })} />
              <label>Library folder<input required value={item.path} onChange={(e) => patch({ libraries: replace(config.libraries, i, { path: e.target.value }) })} /></label>
              <Remove label="library membership" run={() => patch({ libraries: without(config.libraries, i) })} />
            </div>
          ))}
          <button type="button" disabled={included.length === 0}
            onClick={() => patch({ libraries: [...config.libraries, { application: included[0][0], path: "" }] })}>Link library</button>
        </fieldset>
        <Elements title="People" singular="person" items={config.actors} change={(actors) => patch({ actors })} disabled={save.isPending} />
        <Elements title="External systems" singular="external system" items={config.external_systems} change={(external_systems) => patch({ external_systems })} disabled={save.isPending} />
        <Elements title="Data stores" singular="data store" items={config.stores} change={(stores) => patch({ stores })} disabled={save.isPending} />
        <fieldset disabled={save.isPending}>
          <legend>Relations</legend>
          {config.relations.map((item, i) => (
            <div className="architecture-fields" key={i}>
              <Endpoint label="From" value={item.from} options={endpoints} change={(from) => patch({ relations: replace(config.relations, i, { from }) })} />
              <Endpoint label="To" value={item.to} options={endpoints} change={(to) => patch({ relations: replace(config.relations, i, { to }) })} />
              <label>Purpose<input required value={item.label} onChange={(e) => patch({ relations: replace(config.relations, i, { label: e.target.value }) })} /></label>
              <Remove label="relation" run={() => patch({ relations: without(config.relations, i) })} />
            </div>
          ))}
          <button type="button" disabled={endpoints.size < 2}
            onClick={() => patch({ relations: [...config.relations, { from: "", to: "", label: "" }] })}>Add relation</button>
        </fieldset>
      </div>
      <div className="architecture-form-footer">
        {save.error && <p role="alert">{errorText(save.error)}</p>}
        <button type="button" onClick={close}>Cancel</button>
        <button type="submit" disabled={save.isPending}>{save.isPending ? "Saving…" : "Save architecture"}</button>
      </div>
    </form>
  );
}

function Endpoint({ label, value, options, change }: {
  label: string; value: string; options: Map<string, string>; change: (value: string) => void;
}) {
  return (
    <label>{label}
      <select required value={value} onChange={(e) => change(e.target.value)}>
        <option value="">Select…</option>
        {value && !options.has(value) && <option value={value}>{value} (unavailable)</option>}
        {Array.from(options, ([id, name]) => <option key={id} value={id}>{name}</option>)}
      </select>
    </label>
  );
}

function Elements({ title, singular, items, change, disabled }: {
  title: string; singular: string; items: DeclaredElement[]; change: (items: DeclaredElement[]) => void; disabled: boolean;
}) {
  return (
    <fieldset disabled={disabled}>
      <legend>{title}</legend>
      {items.map((item, i) => (
        <div className="architecture-fields" key={item.id}>
          <label>Name<input required value={item.name} onChange={(e) => change(replace(items, i, { name: e.target.value }))} /></label>
          {singular !== "person" && <label>Technology<input value={item.technology} onChange={(e) => change(replace(items, i, { technology: e.target.value }))} /></label>}
          <Remove label={singular} run={() => change(without(items, i))} />
        </div>
      ))}
      <button type="button" onClick={() => change([...items, { id: crypto.randomUUID(), name: "", technology: "" }])}>Add {singular}</button>
    </fieldset>
  );
}

function Remove({ label, run }: { label: string; run: () => void }) {
  return <button type="button" aria-label={"Remove " + label} title={"Remove " + label} onClick={run}>Remove</button>;
}

function replace<T>(items: T[], index: number, patch: Partial<T>): T[] {
  return items.map((item, i) => i === index ? { ...item, ...patch } : item);
}

function without<T>(items: T[], index: number): T[] {
  return items.filter((_, i) => i !== index);
}
