// TypeScript positions are UTF-16 code-unit offsets into the text it read (without a byte
// order mark). Nodex joins facts by UTF-8 byte offsets of the file on disk, so every position
// goes through a per-file converter.

import * as fs from "node:fs";

export type ByteOffset = (pos: number) => number;

const ASCII = /^[\x00-\x7f]*$/;

/** A converter from UTF-16 offsets in `text` to UTF-8 byte offsets in the file `path`. */
export function byteOffsets(text: string, path: string): ByteOffset {
  const bom = hasBom(path) ? 3 : 0;
  if (ASCII.test(text)) {
    return (pos) => pos + bom;
  }
  const prefix = new Uint32Array(text.length + 1);
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);
    let bytes: number;
    if (code < 0x80) bytes = 1;
    else if (code < 0x800) bytes = 2;
    else if (code >= 0xd800 && code <= 0xdbff) bytes = 4; // high surrogate: the pair is 4 bytes
    else if (code >= 0xdc00 && code <= 0xdfff) bytes = 0; // low surrogate: counted with its pair
    else bytes = 3;
    prefix[i + 1] = prefix[i] + bytes;
  }
  return (pos) => prefix[Math.max(0, Math.min(pos, text.length))] + bom;
}

function hasBom(path: string): boolean {
  let fd: number | undefined;
  try {
    fd = fs.openSync(path, "r");
    const buf = Buffer.alloc(3);
    const read = fs.readSync(fd, buf, 0, 3, 0);
    return read === 3 && buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf;
  } catch {
    return false;
  } finally {
    if (fd !== undefined) fs.closeSync(fd);
  }
}
