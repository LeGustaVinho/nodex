import { BlueprintReading } from "../components/BlueprintReading";
import { HighlightedCode } from "../components/HighlightedCode";
import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import type { ReadingSite } from "../gen/ReadingSite";
import { routeName } from "../graph/blueprintReading";
import { useUi } from "../store";
import { BorrowInspector } from "./BorrowInspector";
import { syntaxPreview } from "./SyntaxInspector";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api";
import type { Span } from "../gen/Span";

const MACRO_LABELS = new Set(["Macro invocation", "Build a list", "Format text", "Check a condition", "Wait for one eligible branch"]);

export function RustMacroExpansion({ span, revision }: { span: Span; revision: string }) {
  const [open, setOpen] = useState(false);
  const session = useUi((state) => state.projectSession);
  const result = useQuery({
    queryKey: ["rust-macro-expansion", session, span.file, span.start_byte, revision],
    queryFn: ({ signal }) => api.rustMacroExpansion(span.file, span.start_byte, revision, signal),
    enabled: open,
    retry: false,
  });
  return (
    <div className="insp-section">
      <button type="button" aria-expanded={open} onClick={() => setOpen(!open)}>{open ? "Hide generated Rust" : "Show generated Rust"}</button>
      {open && (result.isPending ? <p role="status">Finding the macro expansion…</p>
        : result.data ? <>
          <div className="section-title">Generated Rust · {result.data.name}</div>
          <button type="button" className="link" onClick={() => useUi.getState().focusCode(span, revision)}>Back to invocation</button>
          <pre className="code-snippet">{result.data.expansion}</pre>
        </> : <p role="status">Expansion unavailable for this occurrence.</p>)}
    </div>
  );
}

export function readingOrigin(all: readonly ReadingSite[], shown: readonly ReadingSite[]) {
  for (const site of shown) {
    const { content } = site;
    if (content.kind === "Detail" && content.borrow && site.path) return site.path;
    if (content.kind === "Pattern" && content.source) {
      const source = content.source;
      const origin = all.find((entry) => entry.content.kind === "Detail" && entry.content.borrow
        && entry.span.file === source.file && entry.span.start_byte === source.start_byte && entry.span.end_byte === source.end_byte);
      if (origin?.path) return origin.path;
    }
  }
}

export function ReadingInspector({ flow, node, sites, origin, revision }: {
  flow: FlowGraph;
  node: FlowNode;
  sites: readonly ReadingSite[];
  origin?: string;
  revision?: (file: string) => string | undefined;
}) {
  const file = flow.span.file;
  const source = useQuery({
    queryKey: ["source", file],
    queryFn: ({ signal }) => api.sourceFile(file, signal),
    enabled: sites.some((site) => site.span.file === file) && !!flow.source_revision,
  });
  if (sites.length === 0) return null;
  const bytes = flow.source_revision && source.data?.revision === flow.source_revision && !source.isError
    ? source.data.bytes : undefined;
  return (
    <section className="insp-section reading-inspector" aria-label="Read this Rust">
      <div className="section-title">Read this Rust</div>
      <BlueprintReading full showDefinition sites={sites} route={(pin) => routeName(node, pin)} preview={(span) => {
        if (span.file !== file) return;
        const snippet = syntaxPreview(bytes, span.start_byte, span.end_byte);
        if (snippet) return (
          <pre className="code-snippet reading-source"><code>
            <HighlightedCode code={snippet.text} language="rust" emphasis={snippet.emphasis} />
          </code></pre>
        );
      }} inspect={(_, span) => {
        const expected = span.file === file ? flow.source_revision : revision?.(span.file);
        if (expected) useUi.getState().focusCode(span, expected);
      }} />
      {origin && <BorrowInspector key={node.id + ":" + origin} flow={flow.symbol} node={node.id} path={origin} />}
      {flow.source_revision && sites.filter(({ content }) => content.kind === "Detail" && MACRO_LABELS.has(content.label))
        .map(({ span }) => <RustMacroExpansion key={span.start_byte} span={span} revision={flow.source_revision!} />)}
    </section>
  );
}
