import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api";
import { flavorInfo, useCatalog } from "../catalog";
import { HighlightedCode } from "../components/HighlightedCode";
import type { Language } from "../gen/Language";
import type { SyntaxSite } from "../gen/SyntaxSite";
import { sourceRange } from "../graph/blueprintSource";
import { codeLanguage } from "../graph/codeTokens";
import { syntaxId } from "../graph/syntaxSites";
import { useUi } from "../store";

export function SyntaxInspector({ sites, language, revision, active }: {
  sites: readonly SyntaxSite[];
  language: Language;
  revision: (file: string) => string | null | undefined;
  active?: string;
}) {
  const catalog = useCatalog();
  const [picked, pick] = useState<string>();
  const entries = [...sites].sort((a, b) => a.span.file.localeCompare(b.span.file) || a.span.start_byte - b.span.start_byte)
    .flatMap((site) => {
      const info = flavorInfo(catalog, site.key, language);
      return info ? [{ site, info, id: syntaxId(site) }] : [];
    });
  const entry = entries.find((e) => e.id === (picked ?? active)) ?? entries[0];
  const file = entry?.site.span.file;
  const expected = file ? revision(file) : undefined;
  const source = useQuery({
    queryKey: ["source", file],
    queryFn: ({ signal }) => api.sourceFile(file!, signal),
    enabled: !!file && !!expected,
  });
  if (!entry || !file) return null;
  const { site: { span }, info } = entry;
  const bytes = expected && source.data?.revision === expected && !source.isError ? source.data.bytes : undefined;
  const preview = syntaxPreview(bytes, span.start_byte, span.end_byte);
  return (
    <section className="insp-section blueprint-syntax">
      <div className="section-title">Syntax in this Blueprint</div>
      {entries.length > 1 && <select aria-label="Syntax occurrence" value={entry.id} onChange={(e) => pick(e.target.value)}>
        {entries.map(({ id, info: item, site }) => (
          <option key={id} value={id}>{item.label} · {site.span.file}:{site.span.start_line}:{site.span.start_col}</option>
        ))}
      </select>}
      <div className="value-row"><strong>{info.label}</strong>{info.phase && <span className="badge">{info.phase}</span>}</div>
      {preview ? <>
        <pre className="code-snippet"><HighlightedCode code={preview.text} language={codeLanguage(language, file)} emphasis={preview.emphasis} /></pre>
        <button type="button" onClick={() => useUi.getState().focusCode(span, expected ?? undefined)}>Show in source</button>
      </> : info.cue && <code>{info.cue}</code>}
      <p className="explain">{info.explain}</p>
      {info.effects?.length && <div className="syntax-example">
        <strong>Example</strong>
        <dl>{info.effects.map(([input, result]) => (
          <div className="syntax-effect" key={input}><dt>{input}</dt><dd><span aria-hidden="true">→ </span>{result}</dd></div>
        ))}</dl>
      </div>}
    </section>
  );
}

export function syntaxPreview(bytes: Uint8Array | undefined, start: number, end: number) {
  if (!bytes || start < 0 || end > bytes.length || start >= end) return;
  const boundary = (index: number) => (bytes[index] & 0xc0) !== 0x80;
  if (!boundary(start) || !boundary(end)) return;
  let left = start;
  let right = end;
  while (left > Math.max(0, start - 110) && ![10, 13].includes(bytes[left - 1])) left--;
  while (right < Math.min(bytes.length, end + 110) && ![10, 13].includes(bytes[right])) right++;
  while (!boundary(left)) left++;
  while (!boundary(right)) right--;
  const before = sourceRange(bytes, left, start)?.trimStart() ?? "";
  let clippedEnd = Math.min(start + 180, end);
  while (!boundary(clippedEnd)) clippedEnd--;
  const token = sourceRange(bytes, start, clippedEnd);
  if (!token) return;
  const after = clippedEnd < end ? "…" : sourceRange(bytes, end, right)?.trimEnd() ?? "";
  return { text: before + token + after, emphasis: [before.length, before.length + token.length] as const };
}
