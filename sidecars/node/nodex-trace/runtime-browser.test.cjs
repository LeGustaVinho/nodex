const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const { test } = require("node:test");
const { runInNewContext } = require("node:vm");

test("browser trace carries native categories alongside value previews", () => {
  const frames = [];
  let socket;
  let flush;
  class WebSocket {
    readyState = 1;
    constructor() { socket = this; }
    send(frame) { frames.push(Buffer.from(frame)); }
  }
  const source = readFileSync(join(__dirname, "runtime-browser.js"), "utf8")
    .replace("__NODEX_WS_URL__", "ws://trace.test")
    .replace(/export \{\};\s*$/, "");
  const context = { WebSocket, TextEncoder, performance, addEventListener() {}, setInterval: (fn) => { flush = fn; } };
  runInNewContext(source, context);
  socket.onopen();
  runInNewContext(`
    __nx.enter(8);
    __nx.arg(8, 0, new Map([[1, true]]));
    __nx.step(8, 1);
    __nx.local(8, 2, [1, 2]);
    __nx.local(8, 3, { [Symbol.toStringTag]: "Map" });
    __nx.local(8, 4, new Set([1]));
  `, context);
  flush();
  assert.equal(frames.length, 1);
  const [frame] = frames;
  assert.equal(frame.readUInt32LE(0), frame.length - 4);
  const records = [];
  for (let at = 4; at < frame.length;) {
    const end = at + 23 + frame.readUInt16LE(at + 21);
    records.push([frame[at], frame.readInt32LE(at + 17), frame.toString("utf8", at + 23, end)]);
    at = end;
  }
  assert.deepEqual(records.slice(1), [
    [1, 0, ""],
    [6, 0, "Map(1)"],
    [12, 0, "Dictionary<?, ?>\nMap"],
    [2, 1, ""],
    [4, 2, "[1, 2]"],
    [11, 2, "Array<?>\nArray"],
    [4, 3, "{}"],
    [11, 3, "Object\nobject"],
    [4, 4, "Set(1)"],
    [11, 4, "Set<?>\nSet"],
  ]);
});
