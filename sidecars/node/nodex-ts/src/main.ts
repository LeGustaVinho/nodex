#!/usr/bin/env node
// nodex-ts — TypeScript sidecar for Nodex.
//
// Without arguments: semantic server. One JSON object per line on stdin, one per line on stdout.
//   request : {"id": 1, "method": "open", "params": {"root": "D:/project"}}
//   response: {"id": 1, "result": {...}}  or  {"id": 1, "error": "message"}
// Methods: open {root} · analyze · update {files: [absolute paths]} · shutdown
//
// `nodex-ts instrument --root <dir> --out <dir> --map <file> --runtime <file>`: writes an
// instrumented JavaScript copy of the project for a live trace (see instrument.ts).

import * as readline from "node:readline";
import { type FactsCache, collectFacts } from "./facts.js";
import { ProjectHost } from "./project.js";

async function serve(): Promise<void> {
  // Anything printed by libraries must not corrupt the protocol stream.
  const out = process.stdout;
  console.log = console.error;
  console.info = console.error;
  console.warn = console.error;
  const write = (message: unknown) => out.write(`${JSON.stringify(message)}\n`);

  let host: ProjectHost | undefined;
  // Per-file facts, so an update re-analyzes only what changed.
  let cache: FactsCache = new Map();
  const require = (): ProjectHost => {
    if (!host) throw new Error("call 'open' first");
    return host;
  };
  const lines = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of lines) {
    if (!line.trim()) continue;
    let id: unknown = null;
    try {
      const request = JSON.parse(line) as { id?: unknown; method?: string; params?: Record<string, unknown> };
      id = request.id ?? null;
      let result: unknown;
      switch (request.method) {
        case "open": {
          const root = request.params?.root;
          if (typeof root !== "string") throw new Error("root is required");
          host = new ProjectHost(root);
          host.open();
          cache = new Map();
          result = { source: host.describe(), messages: host.messages, documents: host.sourceFiles().length };
          break;
        }
        case "analyze":
          result = collectFacts(require(), cache);
          break;
        case "update": {
          const files = Array.isArray(request.params?.files) ? (request.params.files as unknown[]).filter((f): f is string => typeof f === "string") : [];
          require().update(files);
          result = collectFacts(require(), cache);
          break;
        }
        case "shutdown":
          write({ id, result: true });
          process.exit(0);
          break;
        default:
          throw new Error(`unknown method '${request.method}'`);
      }
      write({ id, result });
    } catch (err) {
      write({ id, error: err instanceof Error ? err.message : String(err) });
    }
  }
}

const [command] = process.argv.slice(2);
if (command === "instrument") {
  const { instrumentCommand } = await import("./instrument.js");
  process.exitCode = instrumentCommand(process.argv.slice(3));
} else {
  await serve();
}
