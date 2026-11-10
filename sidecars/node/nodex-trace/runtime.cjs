// Nodex trace runtime for Node.js. Loaded with `node --require runtime.cjs` in front of code
// instrumented by `nodex-ts instrument`; it installs the probes as `globalThis.__nx`.
//
// Every probe is a no-op unless the process was started by Nodex (environment variables
// NODEX_TRACE = "host:port" and NODEX_TRACE_TOKEN). Wire format (little-endian), shared with
// sidecars/dotnet/Nodex.Trace: handshake line "NODEX1 <token>\n", then frames of
// `u32 length` + records; record = u8 kind, i32 thread, i64 micros, i32 method, i32 arg,
// u16 text length, UTF-8 text.
//
// Every frame starts with a Sequence record (kind 10: thread = pid, arg = frame number). Frames
// that may not have reached Nodex when the process ends abruptly (process.exit, crash) are
// written synchronously to NODEX_TRACE_TAIL/<pid>.bin; Nodex skips the ones it already has.
//
// Values are formatted here without running user code: no getters, no toString().
"use strict";

const KIND = { enter: 1, step: 2, exit: 3, local: 4, ret: 5, arg: 6, thrown: 7, dropped: 9, sequence: 10, localType: 11, argType: 12 };
const MAX_PENDING = 16 * 1024 * 1024;
const MAX_TEXT = 96;
const HEADER = 23;

function noop() {}
function identity(v) {
  return v;
}
function rsIdentity(_method, v) {
  return v;
}

const off = {
  enter: noop,
  step: noop,
  exit: noop,
  local: noop,
  arg: noop,
  ret: rsIdentity,
  aw: rsIdentity,
  rs: rsIdentity,
  thrown: noop,
  fmt: identity,
};

function start() {
  const endpoint = process.env.NODEX_TRACE;
  const token = process.env.NODEX_TRACE_TOKEN;
  if (!endpoint || !token) return off;

  const net = require("node:net");
  const fs = require("node:fs");
  const path = require("node:path");
  let threadId = 1;
  try {
    threadId = require("node:worker_threads").threadId + 1;
  } catch {}

  let connected = true;
  const colon = endpoint.lastIndexOf(":");
  const socket = net.connect({ host: endpoint.slice(0, colon), port: Number(endpoint.slice(colon + 1)) });
  socket.setNoDelay(true);
  // The trace connection must not keep the program alive; pending writes still complete.
  socket.unref();
  socket.on("error", () => {
    connected = false; // Nodex went away; keep the program running.
  });
  socket.write(`NODEX1 ${token}\n`);

  const startedAt = process.hrtime.bigint();
  let chunks = [];
  let pendingBytes = 0;
  let dropped = 0;
  let sequence = 0;
  /** Frames handed to the socket whose write has not completed yet: [seq, bytes]. */
  const unacknowledged = new Map();

  function record(kind, method, arg, text) {
    if (pendingBytes > MAX_PENDING) {
      dropped++;
      return;
    }
    const bytes = text === undefined || text === null ? null : Buffer.from(text, "utf8");
    const len = bytes ? Math.min(bytes.length, 0xffff) : 0;
    const b = Buffer.allocUnsafe(HEADER + len);
    b.writeUInt8(kind, 0);
    b.writeInt32LE(threadId, 1);
    b.writeBigInt64LE((process.hrtime.bigint() - startedAt) / 1000n, 5);
    b.writeInt32LE(method | 0, 13);
    b.writeInt32LE(arg | 0, 17);
    b.writeUInt16LE(len, 21);
    if (bytes) bytes.copy(b, HEADER, 0, len);
    chunks.push(b);
    pendingBytes += b.length;
  }

  function takeFrame() {
    if (dropped > 0) {
      record(KIND.dropped, 0, Math.min(dropped, 0x7fffffff));
      dropped = 0;
    }
    if (chunks.length === 0) return null;
    const seq = ++sequence;
    const head = Buffer.alloc(HEADER);
    head.writeUInt8(KIND.sequence, 0);
    head.writeInt32LE(process.pid, 1);
    head.writeInt32LE(seq, 17);
    const payload = Buffer.concat([head, ...chunks]);
    chunks = [];
    pendingBytes = 0;
    const frame = Buffer.allocUnsafe(4 + payload.length);
    frame.writeUInt32LE(payload.length, 0);
    payload.copy(frame, 4);
    return [seq, frame];
  }

  function flush() {
    const taken = takeFrame();
    if (!taken || !connected) return;
    const [seq, frame] = taken;
    unacknowledged.set(seq, frame);
    socket.write(frame, () => unacknowledged.delete(seq));
  }

  const timer = setInterval(flush, 15);
  timer.unref();

  // The event loop is empty: send the rest, close, and let the process end.
  let closing = false;
  process.on("beforeExit", () => {
    if (closing) return;
    closing = true;
    flush();
    if (connected) socket.end();
  });
  // Abrupt end (process.exit, uncaught error): what may not have arrived goes to the tail file.
  process.on("exit", () => {
    const rest = takeFrame();
    const frames = [...unacknowledged.values()];
    if (rest) frames.push(rest[1]);
    const dir = process.env.NODEX_TRACE_TAIL;
    if (!dir || frames.length === 0) return;
    try {
      fs.mkdirSync(dir, { recursive: true });
      fs.writeFileSync(path.join(dir, `${process.pid}.bin`), Buffer.concat(frames));
    } catch {}
  });

  // --- value formatting: bounded, never runs user code ---------------------------------------

  function truncate(s) {
    return s.length > MAX_TEXT ? `${s.slice(0, MAX_TEXT - 1)}…` : s;
  }

  function className(v) {
    try {
      const proto = Object.getPrototypeOf(v);
      if (proto === null) return "Object";
      const ctor = Object.getOwnPropertyDescriptor(proto, "constructor");
      const name = ctor && typeof ctor.value === "function" ? Object.getOwnPropertyDescriptor(ctor.value, "name") : undefined;
      return name && typeof name.value === "string" && name.value ? name.value : "Object";
    } catch {
      return "Object";
    }
  }

  function short(v, depth) {
    switch (typeof v) {
      case "undefined":
        return "undefined";
      case "string":
        return JSON.stringify(v.length > 48 ? `${v.slice(0, 47)}…` : v);
      case "number":
      case "boolean":
        return String(v);
      case "bigint":
        return `${v}n`;
      case "symbol":
        return "Symbol";
      case "function":
        return "ƒ";
    }
    if (v === null) return "null";
    try {
      if (Array.isArray(v)) {
        if (depth <= 0 || v.length > 6) return `Array(${v.length})`;
        return `[${v.map((x) => short(x, depth - 1)).join(", ")}]`;
      }
      if (v instanceof Map) return `Map(${v.size})`;
      if (v instanceof Set) return `Set(${v.size})`;
      if (v instanceof Promise) return "Promise";
      if (v instanceof Error) return `${className(v)}: ${Object.getOwnPropertyDescriptor(v, "message")?.value ?? ""}`;
      if (v instanceof Date) return `Date(${Date.prototype.toISOString.call(v)})`;
      const name = className(v);
      if (depth <= 0) return name;
      const fields = [];
      for (const key of Object.keys(v).slice(0, 4)) {
        const d = Object.getOwnPropertyDescriptor(v, key);
        if (d && "value" in d) fields.push(`${key}: ${short(d.value, depth - 1)}`);
      }
      return `${name === "Object" ? "" : `${name} `}{${fields.join(", ")}}`;
    } catch {
      return "?";
    }
  }


  const typeMapHas = Map.prototype.has;
  const typeSetHas = Set.prototype.has;
  const typeApply = Reflect.apply;
  const typeIsArray = Array.isArray;
  const typeKey = Symbol();

  function typeOf(v) {
    if (v === null) return "Null\nnull";
    switch (typeof v) {
      case "undefined": return "Undefined\nundefined";
      case "string": return "String\nstring";
      case "number": return "Number\nnumber";
      case "boolean": return "Bool\nboolean";
      case "bigint": return "BigInt\nbigint";
      case "symbol": return "Symbol\nsymbol";
      case "function": return "Function\nfunction";
    }
    try { if (typeIsArray(v)) return "Array<?>\nArray"; } catch {}
    // Intrinsic brand checks do not read properties, invoke getters, or inspect elements.
    try { typeApply(typeMapHas, v, [typeKey]); return "Dictionary<?, ?>\nMap"; } catch {}
    try { typeApply(typeSetHas, v, [typeKey]); return "Set<?>\nSet"; } catch {}
    return "Object\nobject";
  }

  function fmt(v) {
    return truncate(short(v, 1));
  }

  const seen = new WeakSet();

  return {
    enter: (m) => record(KIND.enter, m, 0),
    step: (m, p) => record(KIND.step, m, p),
    exit: (m) => record(KIND.exit, m, 0),
    local: (m, slot, v) => {
      record(KIND.local, m, slot, fmt(v));
      record(KIND.localType, m, slot, typeOf(v));
    },
    arg: (m, index, v) => {
      record(KIND.arg, m, index, fmt(v));
      record(KIND.argType, m, index, typeOf(v));
    },
    /** `return __nx.ret(m, value, done = 1)`: records the returned value. */
    ret: (m, v) => {
      record(KIND.ret, m, 0, fmt(v));
      return v;
    },
    /** Before `await`/`yield`: the function pauses (its frame ends). */
    aw: (m, v) => {
      record(KIND.exit, m, 0);
      return v;
    },
    /** After `await`/`yield`: the function resumes (`m` is its "resume" method). */
    rs: (m, v) => {
      record(KIND.enter, m, 0);
      return v;
    },
    /** An error passes through an instrumented function or reaches a `catch` (recorded once). */
    thrown: (e) => {
      if (e !== null && typeof e === "object") {
        if (seen.has(e)) return;
        seen.add(e);
      }
      record(KIND.thrown, 0, 0, fmt(e));
    },
    fmt,
  };
}

let probes = off;
try {
  probes = start();
} catch {
  probes = off; // Tracing must never break the program.
}
globalThis.__nx = probes;
