import Editor, { type OnMount } from "@monaco-editor/react";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import { api } from "../api";
import type { FlowNode } from "../gen/FlowNode";
import type { Span } from "../gen/Span";
import { monaco } from "../monaco";
import { useUi } from "../store";
import { useStructure } from "../views/Views";

type Editor = Parameters<OnMount>[0];

const LANGUAGE_BY_EXT: Record<string, string> = {
  cs: "csharp",
  ts: "typescript",
  tsx: "typescript",
  mts: "typescript",
  cts: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  vue: "html",
  rs: "rust",
};

/** True if the 1-based (line, col) position lies inside the span. */
function contains(span: Span, line: number, col: number): boolean {
  const afterStart = line > span.start_line || (line === span.start_line && col >= span.start_col);
  const beforeEnd = line < span.end_line || (line === span.end_line && col <= span.end_col);
  return afterStart && beforeEnd;
}

const size = (s: Span) => s.end_byte - s.start_byte;

export function CodePanel() {
  const { codeFocus, flowSymbol, view } = useUi();
  const { index } = useStructure();
  const file = codeFocus?.file ?? null;
  const source = useQuery({ queryKey: ["source", file], queryFn: () => api.source(file!), enabled: !!file });
  const flow = useQuery({ queryKey: ["flow", flowSymbol], queryFn: () => api.flow(flowSymbol!), enabled: !!flowSymbol });
  const editorRef = useRef<Editor | null>(null);
  const decorations = useRef<monaco.editor.IEditorDecorationsCollection | null>(null);
  const suppressCursor = useRef(false);
  // The cursor listener is registered once; read the latest data through refs.
  const flowRef = useRef(flow.data);
  const indexRef = useRef(index);
  flowRef.current = flow.data;
  indexRef.current = index;

  // Highlight and reveal the focused span.
  useEffect(() => {
    const editor = editorRef.current;
    if (!editor || !codeFocus || !source.data) return;
    const range = new monaco.Range(codeFocus.start_line, codeFocus.start_col, codeFocus.end_line, codeFocus.end_col);
    // Large declarations (namespaces, classes) only mark their first line; statements are marked whole.
    const large = codeFocus.end_line - codeFocus.start_line > 6;
    const marked = large ? new monaco.Range(codeFocus.start_line, 1, codeFocus.start_line, 1) : range;
    decorations.current?.clear();
    decorations.current = editor.createDecorationsCollection([
      { range: marked, options: { className: "code-focus", isWholeLine: large, overviewRuler: { color: "#f5b041", position: 4 } } },
    ]);
    suppressCursor.current = true;
    editor.setSelection(new monaco.Selection(codeFocus.start_line, codeFocus.start_col, codeFocus.start_line, codeFocus.start_col));
    editor.revealRangeNearTopIfOutsideViewport(large ? marked : range);
    suppressCursor.current = false;
  }, [codeFocus, source.data]);

  const onMount: OnMount = (editor) => {
    editorRef.current = editor;
    // Clicking in the code selects the matching node (code → graph sync).
    editor.onDidChangeCursorPosition((e) => {
      if (suppressCursor.current || e.reason !== monaco.editor.CursorChangeReason.Explicit) return;
      const { lineNumber: line, column: col } = e.position;
      const state = useUi.getState();
      const currentFile = state.codeFocus?.file;
      const flowData = flowRef.current;
      const index = indexRef.current;
      if (!currentFile) return;
      if (state.view === "blueprint" && flowData && flowData.span.file === currentFile) {
        let best: FlowNode | null = null;
        for (const n of flowData.nodes) {
          if (contains(n.span, line, col) && (!best || size(n.span) < size(best.span))) best = n;
        }
        if (best) {
          state.select({ kind: "flow-node", flow: flowData.symbol, id: best.id });
          return;
        }
      }
      if (index) {
        let bestId: string | null = null;
        let bestSize = Infinity;
        for (const s of index.graph.symbols) {
          for (const o of s.origins) {
            if (o.file === currentFile && contains(o, line, col) && size(o) < bestSize) {
              bestSize = size(o);
              bestId = s.id;
            }
          }
        }
        if (bestId) state.select({ kind: "symbol", id: bestId });
      }
    });
  };

  if (!file) return <div className="code-empty muted">Select something to see its code.</div>;
  const ext = file.split(".").pop() ?? "";
  return (
    <div className="code-panel">
      <div className="code-title" title={file}>
        {file}
        {view === "blueprint" && <span className="muted"> · click code to find its node</span>}
      </div>
      <div className="code-editor">
      <Editor
        height="100%"
        theme="nodex-dark"
        path={file}
        language={LANGUAGE_BY_EXT[ext] ?? "plaintext"}
        value={source.data ?? ""}
        onMount={onMount}
        options={{
          readOnly: true,
          minimap: { enabled: false },
          fontSize: 12.5,
          scrollBeyondLastLine: false,
          renderLineHighlight: "line",
          domReadOnly: true,
          wordWrap: "off",
        }}
      />
      </div>
    </div>
  );
}
