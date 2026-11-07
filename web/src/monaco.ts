// Bundles Monaco locally (no CDN) with only the languages Nodex supports.
import { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor/editor/editor.api";
import "monaco-editor/languages/definitions/csharp/register";
import "monaco-editor/languages/definitions/typescript/register";
import "monaco-editor/languages/definitions/javascript/register";
import "monaco-editor/languages/definitions/html/register";
import "monaco-editor/languages/definitions/rust/register";
import EditorWorker from "monaco-editor/editor/editor.worker?worker";

self.MonacoEnvironment = { getWorker: () => new EditorWorker() };
loader.config({ monaco });

monaco.editor.defineTheme("nodex-dark", {
  base: "vs-dark",
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#16181d",
    "editor.lineHighlightBackground": "#1f232b",
    "editorLineNumber.foreground": "#4b5262",
  },
});

export { monaco };
