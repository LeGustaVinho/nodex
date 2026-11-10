import type { Focus } from "../gen/Focus";
import type { Span } from "../gen/Span";

export type BlueprintSource = { bytes: Uint8Array; revision: string | null };
export type BlueprintSources = Readonly<Record<string, BlueprintSource>>;

const decoder = new TextDecoder("utf-8", { ignoreBOM: true });
const encoder = new TextEncoder();
const PREVIEW_BYTES = 2048;
const PREVIEW_UNITS = 512;
export const CODE_ROW_HEIGHT = 16;
export type CodeRange = readonly [start: number, end: number];
export type CodePreview = { text: string; emphasis?: CodeRange };

export function sourceRange(bytes: Uint8Array | undefined, start: number, end: number): string | undefined {
  if (bytes && start >= 0 && start < end && end <= bytes.length) {
    return decoder.decode(bytes.subarray(start, end));
  }
}

const normalize = (text: string) => text.trim().replace(/\p{White_Space}+/gu, " ");

function writtenRange(bytes: Uint8Array | undefined, span: Span) {
  if (!bytes || span.end_byte > bytes.length) return;
  let end = Math.min(span.end_byte, span.start_byte + PREVIEW_BYTES);
  while (end > span.start_byte && (bytes[end] & 0xc0) === 0x80) end--;
  const raw = sourceRange(bytes, span.start_byte, end);
  const text = raw?.trim();
  if (!raw || !text) return;
  const start = span.start_byte + encoder.encode(raw.slice(0, raw.indexOf(text))).length;
  return { text, start, end: start + encoder.encode(text).length, clipped: end < span.end_byte };
}

/** A legacy or declaration target can be emphasized when there is exactly one whole match. */
export function uniqueCodeRange(text: string, value: string | undefined): CodeRange | undefined {
  if (!value) return;
  const word = /[\p{ID_Continue}$]/u;
  const leftWord = word.test(String.fromCodePoint(value.codePointAt(0)!));
  const rightWord = word.test(Array.from(value).at(-1)!);
  let match: CodeRange | undefined;
  for (let start = text.indexOf(value); start >= 0; start = text.indexOf(value, start + 1)) {
    const end = start + value.length;
    if ((leftWord && /[\p{ID_Continue}$]$/u.test(text.slice(0, start))) ||
        (rightWord && /^[\p{ID_Continue}$]/u.test(text.slice(end)))) continue;
    if (match) return;
    match = [start, end];
  }
  return match;
}

export function codePreview(text: string, emphasis?: CodeRange): CodePreview | undefined {
  if (!text) return;
  if (text.length <= PREVIEW_UNITS) return { text, emphasis };
  let end = PREVIEW_UNITS - 1;
  if (/[\uDC00-\uDFFF]/.test(text[end])) end--;
  return { text: `${text.slice(0, end)}…`, emphasis: emphasis && emphasis[1] <= end ? emphasis : undefined };
}

export function codePreviewHeight(code: CodePreview | undefined): number {
  return code ? Math.min(2, code.text.split(/\r\n|\r|\n/).length) * CODE_ROW_HEIGHT : 0;
}

/** Replace operands displayed by other nodes or pins with slots, keeping the surrounding syntax. */
export function sourceCode(
  bytes: Uint8Array | undefined, span: Span, focus?: Focus | null, covered: readonly Span[] = [], header?: string,
): CodePreview | undefined {
  let written = writtenRange(bytes, span);
  if (!written || !bytes) return;
  if (header !== undefined) {
    const actual = normalize(written.text);
    const expected = normalize(header);
    if (!expected) return;
    if (actual !== expected && !(written.clipped && expected.startsWith(actual))) {
      const preview = expected.endsWith("…");
      const anchor = preview ? expected.slice(0, -1).trimEnd() : expected;
      if (!anchor || !actual.startsWith(anchor) || (!preview && !/^[ {;]/u.test(actual.slice(anchor.length)))) return;
      // A statement span can extend past its displayed header (for example, Rust let-else).
      // Match the header against the original whitespace so the body stays on its own nodes.
      const pattern = anchor.split(" ").map((word) => word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("\\p{White_Space}+");
      const text = written.text.match(new RegExp(`^${pattern}`, "u"))?.[0];
      if (!text) return;
      written = { ...written, text, end: written.start + encoder.encode(text).length, clipped: preview };
    }
  }
  const offset = (byte: number) => decoder.decode(bytes.subarray(written.start, byte)).length;
  const target = focus?.primary_bytes;
  let emphasis = target && target[0] >= written.start && target[1] <= written.end &&
      sourceRange(bytes, ...target) === focus?.primary
    ? [offset(target[0]), offset(target[1])] as CodeRange
    : target ? undefined : uniqueCodeRange(written.text, focus?.primary);
  const slots = covered.filter((part) => part.file === span.file &&
      part.start_byte >= span.start_byte && part.end_byte <= span.end_byte &&
      part.start_byte < written.end && part.end_byte > written.start && part.start_byte < part.end_byte)
    .map((part) => [offset(Math.max(part.start_byte, written.start)), offset(Math.min(part.end_byte, written.end))] as CodeRange)
    .sort((a, b) => a[0] - b[0]);
  const parts: CodeRange[] = [];
  for (const slot of slots) {
    const last = parts.at(-1);
    if (last && slot[0] <= last[1]) parts[parts.length - 1] = [last[0], Math.max(last[1], slot[1])];
    else parts.push(slot);
  }
  let text = written.text;
  // Iterate backwards so the remaining ranges still index the original text.
  for (const [start, end] of parts.reverse()) {
    text = `${text.slice(0, start)}□${text.slice(end)}`;
    if (emphasis && start < emphasis[1]) {
      emphasis = end <= emphasis[0] ? [emphasis[0] - (end - start - 1), emphasis[1] - (end - start - 1)] : undefined;
    }
  }
  return codePreview(written.clipped ? `${text}…` : text, emphasis);
}
