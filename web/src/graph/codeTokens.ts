import type { Language } from "../gen/Language";

export type CodeLanguage = Language | "javascript";

export function codeLanguage(language: Language, file: string): CodeLanguage {
  return language === "typescript" && /\.(?:[cm]?js|jsx)$/i.test(file) ? "javascript" : language;
}

type LexicalToken = { offset: number; type: string };
type Tokenizer = (code: string) => LexicalToken[][];
type Color = "keyword" | "type" | "number" | "string" | "comment" | "operator";
type Segment = { text: string; color: Color | null };

const grammars = new Map<CodeLanguage, Promise<Tokenizer>>();
const snippets = new Map<string, Promise<Segment[] | null>>();
const MAX_SNIPPETS = 2048;
const MAX_UNITS = 512;

function loadGrammar(language: CodeLanguage): Promise<Tokenizer> {
  let grammar = grammars.get(language);
  if (!grammar) {
    grammar = import("../monaco").then(async ({ monaco }) => {
      // Loads the registered Monarch grammar without creating an editor, model or worker.
      await monaco.editor.colorize("", language, {});
      return (code) => monaco.editor.tokenize(code, language);
    });
    grammars.set(language, grammar);
  }
  return grammar;
}

function colorOf(type: string): Color | null {
  if (/^(?:type\b|keyword\.type\b)/.test(type)) return "type";
  switch (type.split(".", 1)[0]) {
    case "keyword":
    case "constant":
      return "keyword";
    case "string":
    case "regexp":
      return "string";
    case "number":
      return "number";
    case "comment":
      return "comment";
    case "operator":
    case "delimiter":
      return "operator";
    default:
      return null;
  }
}

/** Keep the original text, including line endings. Monaco's offsets count UTF-16 units. */
export function codeSegments(code: string, tokens: LexicalToken[][]): Segment[] {
  const segments: Segment[] = [];
  const append = (text: string, color: Color | null = null) => {
    if (!text) return;
    const last = segments.at(-1);
    if (last && last.color === color) last.text += text;
    else segments.push({ text, color });
  };
  const parts = code.split(/(\r\n|\r|\n)/);
  for (let i = 0; i < parts.length; i += 2) {
    const line = parts[i];
    const row = tokens[i / 2] ?? [];
    append(line.slice(0, row[0]?.offset ?? line.length));
    row.forEach((token, j) => append(line.slice(token.offset, row[j + 1]?.offset), colorOf(token.type)));
    append(parts[i + 1] ?? "");
  }
  return segments;
}

/** Share work for identical snippets and evict the least recently used entries. */
export function codeTokens(language: CodeLanguage, code: string): Promise<Segment[] | null> | null {
  if (!code || code.length > MAX_UNITS) return null;
  const key = `${language}\0${code}`;
  let entry = snippets.get(key);
  if (entry) snippets.delete(key);
  else entry = loadGrammar(language).then((tokenize) => codeSegments(code, tokenize(code))).catch(() => null);
  snippets.set(key, entry);
  if (snippets.size > MAX_SNIPPETS) snippets.delete(snippets.keys().next().value!);
  return entry;
}
