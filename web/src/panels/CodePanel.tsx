import { toggleBreakpoint, useDebug } from "../debug";
import Editor, { type OnMount } from "@monaco-editor/react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { FlowNode } from "../gen/FlowNode";
import type { Span } from "../gen/Span";
import { isBlueprintContainer } from "../graph/structureGraph";
import { monaco } from "../monaco";
import { useUi } from "../store";
import { useStructure } from "../views/Views";
import { useClassFlows } from "../views/useClassFlows";
import { Loading, LoadingOverlay } from "../components/Loading";
import { CodeTabBar } from "./CodeTabBar";

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

/** UTF-8 byte offset of a Monaco position (spans count bytes; Monaco counts UTF-16 units). */
function byteOffset(model: monaco.editor.ITextModel, position: monaco.IPosition): number {
  const before = model.getValueInRange({ startLineNumber: 1, startColumn: 1, endLineNumber: position.lineNumber, endColumn: position.column });
  return new TextEncoder().encode(before).length;
}

/** A UML diagram of the selected code, or of the whole file when nothing is selected. */
function umlOfSelection(editor: Editor, file: string) {
  const model = editor.getModel();
  const sel = editor.getSelection();
  if (!model || !sel || sel.isEmpty()) {
    useUi.getState().showUml({ kind: "File", path: file }, "Class");
    return;
  }
  const start_byte = byteOffset(model, sel.getStartPosition());
  const end_byte = byteOffset(model, sel.getEndPosition());
  useUi.getState().showUml({ kind: "Range", file, start_byte, end_byte }, "Class");
}

export function CodePanel() {
  const { codeFocus, codeFocusRevision, codeTabs, activeCodeFile: file, projectSession, flowSymbol, view } = useUi();
  const queryClient = useQueryClient();
  const project = useQuery({ queryKey: ["project"], queryFn: api.project });
  const { index } = useStructure();
  const active = flowSymbol ? index?.byId.get(flowSymbol) : undefined;
  const classId = view === "blueprint" && active && isBlueprintContainer(active) ? active.id : null;
  const codeFlow = view === "blueprint" && !classId ? flowSymbol : null;
  const modelPath = (path: string) => monaco.Uri.from({ scheme: "nodex-source", path: `/${projectSession}/${path}` }).toString();
  const activeModelPath = file ? modelPath(file) : null;
  const source = useQuery({ queryKey: ["source", file], queryFn: ({ signal }) => api.sourceFile(file!, signal), enabled: !!file });
  const moved = !!codeFocusRevision && !!source.data && source.data.revision !== codeFocusRevision;
  const validating = !!codeFocusRevision && (source.isFetching || source.isError);
  const flow = useQuery({ queryKey: ["flow", codeFlow], queryFn: () => api.flow(codeFlow!), enabled: !!codeFlow });
  const { ids, query: classFlows } = useClassFlows(index, classId);
  const [editor, setEditor] = useState<Editor | null>(null);
  const [attachedPath, setAttachedPath] = useState<string | null>(null);
  const ownedModels = useRef(new Map<string, monaco.editor.ITextModel>());
  const positions = useRef(new Map<string, monaco.editor.ICodeEditorViewState>());
  const readyPath = useRef<string | null>(null);
  const appliedFocus = useRef<Span | null>(null);
  const decorations = useRef<monaco.editor.IEditorDecorationsCollection | null>(null);
  const debugDecorations = useRef<monaco.editor.IEditorDecorationsCollection | null>(null);
  const breakpoints = useDebug((s) => s.breakpoints);
  const stopped = useDebug((s) => s.stopped);
  const frameId = useDebug((s) => s.frame);
  const fileRef = useRef<string | null>(null);
  fileRef.current = file;
  const suppressCursor = useRef(false);
  // The cursor listener is registered once; read the latest data through refs.
  const flowsRef = useRef({ classId, ids, flows: classFlows.data ?? (flow.data ? [flow.data] : []) });
  const indexRef = useRef(index);
  flowsRef.current = { classId, ids, flows: classFlows.data ?? (flow.data ? [flow.data] : []) };
  indexRef.current = index;

  // Sync the attached model even when two consecutive tabs have identical source text.
  useEffect(() => {
    if (!editor || !activeModelPath || attachedPath !== activeModelPath || !source.data) return;
    const model = editor.getModel();
    if (model?.uri.toString() !== activeModelPath) return;
    const changed = model.getValue() !== source.data.text;
    const alreadyReady = readyPath.current === activeModelPath;
    if (alreadyReady && !changed) return;
    const saved = alreadyReady ? editor.saveViewState() : positions.current.get(activeModelPath);
    suppressCursor.current = true;
    try {
      if (changed) model.setValue(source.data.text);
      if (saved) editor.restoreViewState(saved);
      readyPath.current = activeModelPath;
    } finally {
      suppressCursor.current = false;
    }
  }, [editor, activeModelPath, attachedPath, source.data]);

  // Highlight and reveal the focused span. Source refreshes must not reset a user's cursor.
  useEffect(() => {
    decorations.current?.clear();
    if (!editor || !codeFocus || codeFocus.file !== file || attachedPath !== activeModelPath || !source.data || moved || validating) return;
    const range = new monaco.Range(codeFocus.start_line, codeFocus.start_col, codeFocus.end_line, codeFocus.end_col);
    // Large declarations (namespaces, classes) only mark their first line; statements are marked whole.
    const large = codeFocus.end_line - codeFocus.start_line > 6;
    const marked = large ? new monaco.Range(codeFocus.start_line, 1, codeFocus.start_line, 1) : range;
    decorations.current = editor.createDecorationsCollection([
      { range: marked, options: { className: "code-focus", isWholeLine: large, overviewRuler: { color: "#f5b041", position: 4 } } },
    ]);
    if (appliedFocus.current !== codeFocus) {
      appliedFocus.current = codeFocus;
      suppressCursor.current = true;
      editor.setSelection(new monaco.Selection(codeFocus.start_line, codeFocus.start_col, codeFocus.start_line, codeFocus.start_col));
      editor.revealRangeNearTopIfOutsideViewport(large ? marked : range);
      suppressCursor.current = false;
    }
  }, [codeFocus, file, activeModelPath, attachedPath, moved, validating, source.data, editor]);

  useEffect(() => {
    const open = new Set(codeTabs.map(modelPath));
    for (const [path, model] of ownedModels.current) {
      if (open.has(path) || path === attachedPath) continue;
      model.dispose();
      ownedModels.current.delete(path);
      positions.current.delete(path);
    }
  }, [codeTabs, attachedPath, projectSession]);

  useEffect(() => {
    const models = ownedModels.current;
    const states = positions.current;
    return () => {
      for (const model of models.values()) {
        if (!model.isDisposed()) model.dispose();
      }
      models.clear();
      states.clear();
    };
  }, []);

  useEffect(() => {
    if (moved && !source.isFetching) void queryClient.invalidateQueries({ queryKey: ["navigation"] });
  }, [moved, source.isFetching, queryClient]);

  // Breakpoints in the margin and the line the program is paused at.
  useEffect(() => {
    if (!editor || !file || attachedPath !== activeModelPath) return;
    const frame = stopped?.frames.find((f) => f.id === frameId) ?? stopped?.frames.find((f) => f.file);
    const list: monaco.editor.IModelDeltaDecoration[] = breakpoints
      .filter((b) => b.file === file)
      .map((b) => ({ range: new monaco.Range(b.line, 1, b.line, 1), options: { glyphMarginClassName: "glyph-breakpoint", glyphMarginHoverMessage: { value: "Breakpoint (click to remove)" } } }));
    if (frame?.file === file) {
      list.push({ range: new monaco.Range(frame.line, 1, frame.line, 1), options: { isWholeLine: true, className: "code-paused", glyphMarginClassName: "glyph-paused" } });
    }
    debugDecorations.current?.clear();
    debugDecorations.current = editor.createDecorationsCollection(list);
  }, [breakpoints, stopped, frameId, file, activeModelPath, attachedPath, source.data, editor]);

  const onMount: OnMount = (editor) => {
    setEditor(editor);
    const registerModel = () => {
      const model = editor.getModel();
      const path = model?.uri.toString() ?? null;
      if (model && path) ownedModels.current.set(path, model);
      setAttachedPath(path);
    };
    registerModel();
    editor.onWillChangeModel(() => {
      const path = editor.getModel()?.uri.toString();
      if (path && path === readyPath.current) {
        const saved = editor.saveViewState();
        if (saved) positions.current.set(path, saved);
      }
      readyPath.current = null;
    });
    editor.onDidChangeModel(registerModel);
    const currentFile = () => {
      const activeFile = fileRef.current;
      return activeFile && editor.getModel()?.uri.toString() === modelPath(activeFile) ? activeFile : null;
    };
    editor.addAction({
      id: "nodex.uml.selection",
      label: "UML diagram of the selection",
      contextMenuGroupId: "navigation",
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyU],
      run: (ed) => {
        const file = currentFile();
        if (file) umlOfSelection(ed as Editor, file);
      },
    });
    // A click in the margin toggles a breakpoint on that line.
    editor.onMouseDown((e) => {
      const t = e.target.type;
      if (t !== monaco.editor.MouseTargetType.GUTTER_GLYPH_MARGIN && t !== monaco.editor.MouseTargetType.GUTTER_LINE_NUMBERS) return;
      const line = e.target.position?.lineNumber;
      const file = currentFile();
      if (line && file) void toggleBreakpoint(file, line);
    });
    // Clicking in the code selects the matching node (code → graph sync).
    editor.onDidChangeCursorPosition((e) => {
      if (suppressCursor.current || e.reason !== monaco.editor.CursorChangeReason.Explicit) return;
      const { lineNumber: line, column: col } = e.position;
      const state = useUi.getState();
      const file = currentFile();
      const { classId, ids, flows } = flowsRef.current;
      const index = indexRef.current;
      if (!file) return;
      if (state.view === "blueprint") {
        let best: { flow: string; node: FlowNode } | undefined;
        for (const flow of flows) {
          if (flow.span.file !== file) continue;
          for (const node of flow.nodes) {
            if (node.span.file === file && contains(node.span, line, col) && (!best || size(node.span) < size(best.node.span))) {
              best = { flow: flow.symbol, node };
            }
          }
        }
        if (best) {
          if (classId && !(state.classOpen[best.flow] ?? ids.length <= 4)) state.setClassOpen([best.flow], true);
          state.select({ kind: "flow-node", flow: best.flow, id: best.node.id });
          return;
        }
      }
      if (index) {
        let bestId: string | null = null;
        let bestSize = Infinity;
        for (const s of index.graph.symbols) {
          for (const o of s.origins) {
            if (o.file === file && contains(o, line, col) && size(o) < bestSize) {
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
      <CodeTabBar project={project.data} index={index} />
      <div className="code-title" title={file}>
        {file}
        {view === "blueprint" && <span className="muted"> · click code to find its node</span>}
        {moved && !source.isFetching && <span role="status" className="muted"> · File changed. Choose a refreshed result or run the navigation action again.</span>}
        <button
          className="code-uml"
          title="UML diagram of the selected code, or of this file (Ctrl+Shift+U; also in the editor's right-click menu)"
          disabled={attachedPath !== activeModelPath}
          onClick={() => editor && umlOfSelection(editor, file)}
        >
          UML
        </button>
      </div>
      <div className="code-editor" id="source-code-panel" role="tabpanel" aria-label={file}>
      <Editor
        height="100%"
        theme="nodex-dark"
        path={activeModelPath!}
        saveViewState={false}
        language={LANGUAGE_BY_EXT[ext] ?? "plaintext"}
        onMount={onMount}
        loading={<Loading label="Loading the code editor…" />}
        options={{
          readOnly: true,
          // Follow the panel when a splitter or the window changes its size.
          automaticLayout: true,
          glyphMargin: true,
          minimap: { enabled: false },
          fontSize: 12.5,
          scrollBeyondLastLine: false,
          renderLineHighlight: "line",
          domReadOnly: true,
          wordWrap: "off",
        }}
      />
      {source.isFetching && source.data === undefined && <LoadingOverlay label="Loading the file…" detail={file} />}
      {source.isError && <div className="code-error">Could not read {file}: {String(source.error)}</div>}
      </div>
    </div>
  );
}
