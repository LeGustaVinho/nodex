import { memo, useEffect, useState, type ReactNode } from "react";
import { type CodeLanguage, codeTokens } from "../graph/codeTokens";
import type { CodePreview, CodeRange } from "../graph/blueprintSource";

type Tokens = Awaited<ReturnType<typeof codeTokens>>;

export const HighlightedCode = memo(function HighlightedCode({ code, language, emphasis }: {
  code: string; language: CodeLanguage; emphasis?: CodeRange;
}) {
  const [result, setResult] = useState<{ code: string; language: CodeLanguage; tokens: Tokens }>();
  useEffect(() => {
    let current = true;
    void codeTokens(language, code)?.then((tokens) => {
      if (current && tokens?.some((token) => token.color)) setResult({ code, language, tokens });
    });
    return () => { current = false; };
  }, [code, language]);

  const tokens = result?.code === code && result.language === language ? result.tokens : null;
  if (!emphasis && !tokens) return code;
  const [start, end] = emphasis ?? [code.length, code.length];
  const before: ReactNode[] = [];
  const marked: ReactNode[] = [];
  const after: ReactNode[] = [];
  let offset = 0;
  for (const { text, color } of tokens ?? [{ text: code, color: null }]) {
    const add = (out: ReactNode[], from: number, to: number) => {
      const part = text.slice(from, to);
      if (part) out.push(color ? <span className={`code-token-${color}`} key={offset}>{part}</span> : part);
    };
    const a = Math.min(text.length, Math.max(0, start - offset));
    const b = Math.min(text.length, Math.max(0, end - offset));
    add(before, 0, a);
    add(marked, a, b);
    add(after, b, text.length);
    offset += text.length;
  }
  return <>{before}{marked.length > 0 && <strong className="blueprint-code-target">{marked}</strong>}{after}</>;
});

export function BlueprintCode({ preview, language, target, className }: {
  preview: CodePreview; language: CodeLanguage; target?: string | null; className?: string;
}) {
  return (
    <code className={className ? `blueprint-code ${className}` : "blueprint-code"} title={target ?? preview.text}>
      <HighlightedCode code={preview.text} language={language} emphasis={preview.emphasis} />
    </code>
  );
}
