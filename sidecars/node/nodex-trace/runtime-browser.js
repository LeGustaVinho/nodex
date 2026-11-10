// Nodex trace runtime for browser pages. `nodex-ts instrument --target browser` copies it into
// the instrumented app as `__nodex_runtime.js`, with the run's WebSocket address as WS_URL; every instrumented module imports it first, so `globalThis.__nx` exists
// before any probe runs.
//
// Same records as runtime.cjs (see sidecars/dotnet/Nodex.Trace for the format): each binary
// WebSocket message is one frame — `u32 length` + records, starting with a Sequence record
// (kind 10: thread = a random page id, arg = frame number). Frames are sent every 15 ms and when
// the page is hidden or left. Values are formatted without running user code.
/* eslint-disable */

const WS_URL = "__NODEX_WS_URL__";

const KIND = { enter: 1, step: 2, exit: 3, local: 4, ret: 5, arg: 6, thrown: 7, dropped: 9, sequence: 10, localType: 11, argType: 12 };
const MAX_PENDING = 16 * 1024 * 1024;
const MAX_TEXT = 96;
const HEADER = 23;

function noop() {}
function rsIdentity(_m, v) {
  return v;
}
const off = { enter: noop, step: noop, exit: noop, local: noop, arg: noop, ret: rsIdentity, aw: rsIdentity, rs: rsIdentity, thrown: noop, fmt: (v) => v };

function start() {
  if (typeof WebSocket === "undefined" || !WS_URL.startsWith("ws")) return off;
  const encoder = new TextEncoder();
  const page = (Math.random() * 0x7fffffff) | 0;
  const startedAt = performance.now();
  let chunks = [];
  let pendingBytes = 0;
  let dropped = 0;
  let sequence = 0;
  /** Frames waiting for the socket to open. */
  const queue = [];
  let socket = null;
  let open = false;

  function connect() {
    try {
      socket = new WebSocket(WS_URL);
      socket.binaryType = "arraybuffer";
      socket.onopen = () => {
        open = true;
        while (queue.length) socket.send(queue.shift());
      };
      socket.onclose = () => {
        open = false;
      };
      socket.onerror = () => {};
    } catch {
      socket = null;
    }
  }
  connect();

  function record(kind, method, arg, text) {
    if (pendingBytes > MAX_PENDING) {
      dropped++;
      return;
    }
    const bytes = text === undefined || text === null ? null : encoder.encode(text);
    const len = bytes ? Math.min(bytes.length, 0xffff) : 0;
    const b = new Uint8Array(HEADER + len);
    const v = new DataView(b.buffer);
    v.setUint8(0, kind);
    v.setInt32(1, 1, true);
    v.setBigInt64(5, BigInt(Math.round((performance.now() - startedAt) * 1000)), true);
    v.setInt32(13, method | 0, true);
    v.setInt32(17, arg | 0, true);
    v.setUint16(21, len, true);
    if (bytes) b.set(bytes.subarray(0, len), HEADER);
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
    const total = HEADER + pendingBytes;
    const frame = new Uint8Array(4 + total);
    const v = new DataView(frame.buffer);
    v.setUint32(0, total, true);
    v.setUint8(4, KIND.sequence);
    v.setInt32(5, page, true);
    v.setInt32(4 + 17, seq, true);
    let at = 4 + HEADER;
    for (const c of chunks) {
      frame.set(c, at);
      at += c.length;
    }
    chunks = [];
    pendingBytes = 0;
    return frame.buffer;
  }

  function flush() {
    const frame = takeFrame();
    if (!frame) return;
    if (open && socket && socket.readyState === 1) socket.send(frame);
    else if (queue.length < 256) queue.push(frame);
  }
  setInterval(flush, 15);
  addEventListener("pagehide", flush);
  addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") flush();
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
      if (typeof Event !== "undefined" && v instanceof Event) return `${className(v)}(${Object.getOwnPropertyDescriptor(Event.prototype, "type")?.get?.call(v) ?? ""})`;
      if (typeof Node !== "undefined" && v instanceof Node) return `<${String(Object.getOwnPropertyDescriptor(Node.prototype, "nodeName")?.get?.call(v) ?? "node").toLowerCase()}>`;
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
    ret: (m, v) => {
      record(KIND.ret, m, 0, fmt(v));
      return v;
    },
    aw: (m, v) => {
      record(KIND.exit, m, 0);
      return v;
    },
    rs: (m, v) => {
      record(KIND.enter, m, 0);
      return v;
    },
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

if (!globalThis.__nx) {
  let probes = off;
  try {
    probes = start();
  } catch {
    probes = off; // Tracing must never break the page.
  }
  globalThis.__nx = probes;
}
export {};
