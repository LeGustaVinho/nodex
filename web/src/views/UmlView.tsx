import { useEffect, useMemo, useRef, useState } from "react";
import { openMenu } from "../components/ContextMenu";
import type { UmlDiagram } from "../gen/UmlDiagram";
import type { UmlElement } from "../gen/UmlElement";
import { useUi } from "../store";
import { umlMenuItems } from "../uml/menu";
import { elementIdOf, kindLabel, scopeLabel } from "../uml/uml";
import { UmlControls } from "../uml/UmlControls";
import { useUmlDiagram } from "../uml/useUmlDiagram";
import { ConfidenceToggles } from "../components/ConfidenceToggles";
import { saveFile } from "../desktop";
import { errorText, toast } from "../toast";
import { useStructure } from "./Views";
import { Loading, LoadingBadge, LoadingOverlay } from "../components/Loading";

type Mermaid = typeof import("mermaid").default;
let loading: Promise<Mermaid> | null = null;

/** Loads Mermaid once, on first use (it is large), themed like the app. */
function mermaid(): Promise<Mermaid> {
  loading ??= import("mermaid").then(({ default: m }) => {
    const css = getComputedStyle(document.documentElement);
    const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
    m.initialize({
      startOnLoad: false,
      securityLevel: "strict",
      theme: "base",
      maxTextSize: 2_000_000,
      maxEdges: 5000,
      htmlLabels: false,
      fontFamily: v("--sans", "system-ui, sans-serif"),
      flowchart: { htmlLabels: false, curve: "basis", padding: 12 },
      class: { htmlLabels: false },
      themeVariables: {
        darkMode: true,
        background: v("--bg", "#0f1216"),
        primaryColor: v("--panel-2", "#1d232c"),
        primaryTextColor: v("--text", "#eef1f4"),
        primaryBorderColor: "#5d6b7d",
        secondaryColor: v("--panel", "#161a20"),
        tertiaryColor: v("--panel", "#161a20"),
        lineColor: "#8b98a8",
        textColor: v("--text", "#eef1f4"),
        mainBkg: v("--panel-2", "#1d232c"),
        nodeBorder: "#5d6b7d",
        clusterBkg: "rgba(255,255,255,0.03)",
        clusterBorder: "#3a434f",
        titleColor: v("--text", "#eef1f4"),
        edgeLabelBackground: v("--panel", "#161a20"),
        noteBkgColor: "#2a2f37",
        noteTextColor: v("--text", "#eef1f4"),
        noteBorderColor: "#4b5563",
      },
    });
    return m;
  });
  return loading;
}

let renders = 0;


/** A UML diagram of the chosen scope, drawn by Mermaid and linked back to the code. */
export function UmlView() {
  const ui = useUi();
  const { index } = useStructure();
  const { kind, scope, availability, available, diagram } = useUmlDiagram();
  const [svg, setSvg] = useState<string>("");
  const [renderError, setRenderError] = useState<string | null>(null);
  /** Mermaid is drawing the current text (it runs on the main thread). */
  const [drawing, setDrawing] = useState(false);
  const [showText, setShowText] = useState(false);
  const [picked, setPicked] = useState<string | null>(null);
  const [view, setView] = useState({ x: 0, y: 0, scale: 1 });
  const box = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; y: number; vx: number; vy: number } | null>(null);
  /** The mouse moved while pressed: the release ends a pan, not a click. */
  const moved = useRef(false);
  /** A new drawing arrived: fit it once it is in the page. */
  const fitPending = useRef(false);
  /** The reader zoomed or panned: resizing the canvas keeps their view. */
  const adjusted = useRef(false);
  const data = diagram.data;

  // Draw the text; keep the previous drawing until the new one is ready.
  useEffect(() => {
    if (!data?.mermaid) return;
    let cancelled = false;
    setDrawing(true);
    // Let the overlay paint before Mermaid takes the main thread.
    void new Promise((r) => requestAnimationFrame(() => setTimeout(r, 0)))
      .then(() => mermaid())
      .then((m) => m.render(`uml-${++renders}`, data.mermaid))
      .then(({ svg }) => {
        if (cancelled) return;
        setSvg(svg);
        setRenderError(null);
        setPicked(null);
        fitPending.current = true;
      })
      .catch((e: unknown) => !cancelled && setRenderError(String(e)))
      .finally(() => !cancelled && setDrawing(false));
    return () => {
      cancelled = true;
    };
  }, [data?.mermaid]);

  const byId = useMemo(() => new Map((data?.elements ?? []).map((e) => [e.mermaid_id, e])), [data]);

  /** Scales the drawing to fill the canvas (never above 1.5×) and centers it. */
  const fit = () => {
    const canvas = box.current;
    const el = canvas?.querySelector<SVGSVGElement>(".uml-svg svg");
    if (!canvas || !el) return;
    const vb = el.viewBox.baseVal;
    const w = vb && vb.width ? vb.width : el.getBoundingClientRect().width;
    const h = vb && vb.height ? vb.height : el.getBoundingClientRect().height;
    // Draw at the natural size; the transform does the zooming.
    el.setAttribute("width", String(w));
    el.setAttribute("height", String(h));
    el.style.maxWidth = "none";
    const pad = 32;
    const scale = Math.max(0.15, Math.min(1.5, (canvas.clientWidth - pad) / w, (canvas.clientHeight - pad) / h));
    setView({ scale, x: (canvas.clientWidth - w * scale) / 2, y: Math.max(8, (canvas.clientHeight - h * scale) / 2) });
    adjusted.current = false;
  };
  const fitRef = useRef(fit);
  fitRef.current = fit;

  // Refit when the canvas changes size (the window, a panel), unless the reader moved the view.
  useEffect(() => {
    const canvas = box.current;
    if (!canvas) return;
    const ro = new ResizeObserver(() => {
      if (!adjusted.current) fitRef.current();
    });
    ro.observe(canvas);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    if (svg && fitPending.current) {
      fitPending.current = false;
      fit();
    }
  });

  // Link drawn nodes to their elements.
  useEffect(() => {
    const host = box.current?.querySelector(".uml-svg");
    if (!host || !data) return;
    const link = (n: SVGElement, id: string | null) => {
      if (!id || !byId.has(id)) return;
      n.dataset.element = id;
      n.classList.add("uml-node");
      n.classList.toggle("picked", id === picked);
    };
    for (const n of host.querySelectorAll<SVGElement>("g[id]")) link(n, elementIdOf(n.id));
    // Sequence diagrams name participant boxes (`<rect name="n2">`) instead of giving them ids.
    for (const n of host.querySelectorAll<SVGElement>("rect[name], g[name]")) {
      const target = n.tagName.toLowerCase() === "rect" ? (n.parentElement as unknown as SVGElement | null) : n;
      if (target) link(target, n.getAttribute("name"));
    }
  }, [svg, byId, data, picked]);

  const elementAt = (target: EventTarget | null): UmlElement | null => {
    const el = (target as Element | null)?.closest?.("[data-element]") as HTMLElement | null;
    return el ? (byId.get(el.dataset.element ?? "") ?? null) : null;
  };
  const symbolOf = (e: UmlElement) => (e.symbol && !e.symbol.startsWith("dir:") ? index?.byId.get(e.symbol) : undefined);

  const download = (name: string, type: string, content: Blob | string) => {
    saveFile(name, type, content)
      .then((saved) => saved && toast.info(`Saved ${saved}`))
      .catch((e: unknown) => toast.error(`Could not save ${name}: ${errorText(e)}`));
  };
  const base = `${kind.toLowerCase()}-${scopeLabel(scope).replace(/[^\w.-]+/g, "_")}`;
  const exportPng = () => {
    const el = box.current?.querySelector("svg");
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement("canvas");
      const ratio = 2;
      canvas.width = Math.max(1, rect.width / view.scale) * ratio;
      canvas.height = Math.max(1, rect.height / view.scale) * ratio;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue("--bg") || "#0f1216";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
      canvas.toBlob((b) => b && download(`${base}.png`, "image/png", b));
    };
    img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  };

  const pickedEl = picked ? byId.get(picked) : undefined;
  const pickedRelations = data && picked ? data.relations.filter((r) => r.from === picked || r.to === picked) : [];

  if (availability.isError) {
    return (
      <div className="empty">
        <div>
          <p>{String(availability.error).replace(/^Error: \d+ /, "")}</p>
          {ui.umlExplicitScope && <button onClick={() => ui.showUml()}>Use the project context</button>}
        </div>
      </div>
    );
  }
  if (available && !available.available) {
    return (
      <div className="empty uml-empty">
        <p>
          <b>{kindLabel(kind)}</b> for {scopeLabel(scope, (id) => index?.byId.get(id)?.name)}: {available.reason}.
        </p>
        {availability.data?.suggested && availability.data.suggested !== kind && (
          <button onClick={() => ui.showUml(undefined, availability.data!.suggested!)}>Show {kindLabel(availability.data.suggested)} instead</button>
        )}
      </div>
    );
  }

  return (
    <div className="uml">
      <div className="uml-controls">
        <UmlControls />
        <ConfidenceToggles />
      </div>
      <div className="uml-bar">
        <span className="uml-title" title={data?.title}>
          {kindLabel(kind)} · {data?.title ?? scopeLabel(scope)}
        </span>
        {data && (
          <span className="muted small">
            {data.stats.elements} elements · {data.stats.relations} relations
            {data.stats.hidden > 0 && ` · ${data.stats.hidden} hidden`}
          </span>
        )}
        {svg && (diagram.isFetching || availability.isFetching || drawing) && <LoadingBadge label="updating…" />}
        <span className="spacer" />
        <button onClick={() => ((adjusted.current = true), setView((v) => ({ ...v, scale: Math.min(4, v.scale * 1.25) })))} title="Zoom in">
          +
        </button>
        <button onClick={() => ((adjusted.current = true), setView((v) => ({ ...v, scale: Math.max(0.15, v.scale / 1.25) })))} title="Zoom out">
          −
        </button>
        <button onClick={fit} title="Fit to the view">
          Fit
        </button>
        <button className={showText ? "on" : ""} onClick={() => setShowText((t) => !t)} title="Show the Mermaid text">
          Text
        </button>
        <button disabled={!data} onClick={() => data && void navigator.clipboard.writeText(data.mermaid)} title="Copy the Mermaid text">
          Copy
        </button>
        <button disabled={!data} onClick={() => data && void navigator.clipboard.writeText("```mermaid\n" + data.mermaid + "```\n")} title="Copy as a Markdown block">
          Markdown
        </button>
        <button disabled={!data} onClick={() => data && download(`${base}.mmd`, "text/plain", data.mermaid)} title="Save the Mermaid file">
          .mmd
        </button>
        <button disabled={!svg} onClick={() => download(`${base}.svg`, "image/svg+xml", svg)} title="Save as SVG">
          .svg
        </button>
        <button disabled={!svg} onClick={exportPng} title="Save as PNG">
          .png
        </button>
      </div>
      {data && data.warnings.length > 0 && (
        <div className="uml-warnings">
          {data.warnings.map((w) => (
            <div key={w}>⚠ {w}</div>
          ))}
        </div>
      )}
      {diagram.isError && <div className="empty">Could not build the diagram: {String(diagram.error)}</div>}
      {renderError && (
        <div className="uml-error">
          <p>Mermaid could not draw this diagram: {renderError}</p>
          <button onClick={() => data && void navigator.clipboard.writeText(`${renderError}\n\n${data.mermaid}`)}>Copy error and text</button>
        </div>
      )}
      <div className="uml-body">
        <div
          ref={box}
          className="uml-canvas"
          onWheel={(e) => {
            adjusted.current = true;
            const rect = e.currentTarget.getBoundingClientRect();
            const factor = Math.exp(-Math.max(-600, Math.min(600, e.deltaY)) * 0.0015);
            setView((v) => {
              const scale = Math.max(0.15, Math.min(4, v.scale * factor));
              const px = e.clientX - rect.left;
              const py = e.clientY - rect.top;
              return { scale, x: px - ((px - v.x) * scale) / v.scale, y: py - ((py - v.y) * scale) / v.scale };
            });
          }}
          onMouseDown={(e) => {
            if (e.button === 0) drag.current = { x: e.clientX, y: e.clientY, vx: view.x, vy: view.y };
            moved.current = false;
          }}
          onMouseMove={(e) => {
            const d = drag.current;
            if (!d) return;
            if (Math.abs(e.clientX - d.x) + Math.abs(e.clientY - d.y) > 3) moved.current = adjusted.current = true;
            setView((v) => ({ ...v, x: d.vx + e.clientX - d.x, y: d.vy + e.clientY - d.y }));
          }}
          onMouseUp={() => (drag.current = null)}
          onMouseLeave={() => (drag.current = null)}
          onClick={(e) => {
            if (moved.current) return;
            const el = elementAt(e.target);
            setPicked(el?.mermaid_id ?? null);
            if (!el) return;
            const sym = symbolOf(el);
            if (sym) ui.select({ kind: "symbol", id: sym.id }, el.span ?? sym.origins[0] ?? null);
            else if (el.span) ui.focusCode(el.span);
          }}
          onDoubleClick={(e) => {
            const el = elementAt(e.target);
            if (!el?.symbol) return;
            if (el.symbol.startsWith("dir:")) {
              ui.showUml({ kind: "Symbol", id: el.symbol }, "Class");
              return;
            }
            const sym = symbolOf(el);
            if (sym?.has_flow) ui.openFlow(sym.id);
            else if (sym) ui.showUml({ kind: "Symbol", id: sym.id }, "Class");
          }}
          onContextMenu={(e) => {
            const el = elementAt(e.target);
            if (!el?.symbol) return;
            const sym = symbolOf(el);
            openMenu(e, [
              ...umlMenuItems({ kind: "Symbol", id: el.symbol }, `UML of ${sym?.name ?? el.label}`),
              ...(sym ? [{ label: "Show in Knowledge", run: () => ui.showKnowledge(sym.id) }] : []),
              ...(sym?.has_flow ? [{ label: "Open Blueprint", run: () => ui.openFlow(sym.id) }] : []),
            ]);
          }}
        >
          {svg ? (
            <div
              className="uml-svg"
              style={{ transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})` }}
              // Mermaid output, rendered with securityLevel "strict" (no scripts, sanitized labels).
              dangerouslySetInnerHTML={{ __html: svg }}
            />
          ) : (
            (diagram.isLoading || availability.isLoading || drawing) && (
              <Loading label={drawing ? "Drawing the diagram…" : "Building the diagram…"} detail={data ? `${data.stats.elements} elements · ${data.stats.relations} relations` : undefined} />
            )
          )}
          {svg && drawing && <LoadingOverlay label="Drawing the diagram…" detail={data ? `${data.stats.elements} elements` : undefined} />}
        </div>
        {showText && data && <pre className="uml-text">{data.mermaid}</pre>}
        {pickedEl && <Details element={pickedEl} diagram={data!} relations={pickedRelations} onClose={() => setPicked(null)} />}
      </div>
    </div>
  );
}

/** Why an element is drawn: what it is, and the evidence of its relations. */
function Details({ element, diagram, relations, onClose }: { element: UmlElement; diagram: UmlDiagram; relations: UmlDiagram["relations"]; onClose: () => void }) {
  const focusCode = useUi((s) => s.focusCode);
  const label = (id: string) => diagram.elements.find((e) => e.mermaid_id === id)?.label ?? id;
  return (
    <div className="uml-details">
      <div className="uml-details-head">
        <b>{element.label}</b>
        <span className="badge small">{element.kind}</span>
        {element.context && <span className="badge small">outside the scope</span>}
        <button className="chip-x" onClick={onClose} title="Close">
          ×
        </button>
      </div>
      {element.detail && <div className="muted small">{element.detail}</div>}
      {relations.length > 0 && <div className="section-title">Why these relations</div>}
      {relations.slice(0, 30).map((r, i) => (
        <div key={i} className="evidence">
          <div className="value-row">
            <span>{r.from === element.mermaid_id ? `${r.kind} →` : `← ${r.kind}`}</span>
            <span>{r.from === element.mermaid_id ? label(r.to) : label(r.from)}</span>
            {r.confidence && r.confidence !== "Exact" && <span className={`conf conf-${r.confidence.toLowerCase()}`}>{r.confidence.toLowerCase()}</span>}
            {r.count > 1 && <span className="muted">×{r.count}</span>}
          </div>
          {(r.evidence || r.site) && (
            <div className="evidence-detail">
              {r.evidence && <span>{r.evidence}</span>}
              {r.site && (
                <button className="link muted" onClick={() => focusCode(r.site!)}>
                  {r.site.file}:{r.site.start_line}
                </button>
              )}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
