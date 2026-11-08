// `nodex-ts instrument --target browser`: an instrumented *source* copy of a browser app
// (React, Vue, Angular, …) that the project's own dev server builds and serves.
//
// - The project is copied to `<out>/web/src` (without node_modules, build output and VCS);
//   `node_modules` is linked into the copy (a directory junction on Windows).
// - Each TS/TSX/JS/JSX file of the project, and the <script> blocks of `.vue` files, get the
//   same probes as Node runs, printed back as source (types and JSX kept) so Vite, webpack or
//   the Angular CLI compile the copy as usual.
// - Every instrumented file starts with `import "<relative>/__nodex_runtime.js"`: ES modules run
//   their imports first, so `globalThis.__nx` exists before any probe, whatever the entry is.
// - The trace map keeps the byte ranges of the original files.

import * as fs from "node:fs";
import * as path from "node:path";
import ts from "typescript";
import type { MethodMap } from "./instrument.js";

/** Never copied, at any depth. */
const SKIP = new Set(["node_modules", ".git", ".nodex"]);
/** Build output and tool caches: skipped at the project root only (a `src/build/` folder is source). */
const ROOT_SKIP = new Set(["dist", "build", "coverage", ".angular", ".svelte-kit", ".next", ".nuxt", ".turbo", ".vite", "target", "bin", "obj"]);

/** Whether a folder is left out of the copy; dot-folders only at the root (keeps `public/.well-known`). */
function skipped(name: string, atRoot: boolean): boolean {
  if (SKIP.has(name)) return true;
  return atRoot && (ROOT_SKIP.has(name) || (name.startsWith(".") && name !== ".vscode"));
}
const CODE = /\.(tsx?|jsx?|mts|cts|mjs|cjs)$/;
const FRAMEWORKS = ["react", "react-dom", "vue", "@angular/core", "svelte", "preact", "solid-js", "lit"];
const DEV_TOOLS = /\b(vite|ng\s+serve|webpack(-dev-server|\s+serve)|react-scripts\s+start|vue-cli-service\s+serve|parcel|rsbuild\s+dev|astro\s+dev)\b/;

interface PackageJson {
  nodex?: { entry?: string; target?: "browser" | "node" };
  main?: string;
  bin?: unknown;
  scripts?: Record<string, string>;
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
}

function readPackage(root: string): PackageJson | undefined {
  try {
    return JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")) as PackageJson;
  } catch {
    return undefined;
  }
}

/** The dev script that serves the app: `dev`, `start` or `serve`, if it runs a dev server. */
function devScript(pkg: PackageJson): string | undefined {
  for (const name of ["dev", "start", "serve"]) {
    const script = pkg.scripts?.[name];
    if (script && DEV_TOOLS.test(script)) return name;
  }
  return undefined;
}

/** `browser` when the project is a browser app with a dev server, otherwise `node`. */
export function detectTarget(root: string): "browser" | "node" {
  const pkg = readPackage(root);
  if (!pkg) return "node";
  if (pkg.nodex?.target) return pkg.nodex.target;
  if (pkg.nodex?.entry || pkg.bin) return "node";
  const deps = { ...pkg.dependencies, ...pkg.devDependencies };
  const framework = FRAMEWORKS.some((f) => f in deps);
  return framework && devScript(pkg) ? "browser" : "node";
}

function packageManager(root: string): string {
  if (fs.existsSync(path.join(root, "pnpm-lock.yaml"))) return "pnpm";
  if (fs.existsSync(path.join(root, "yarn.lock"))) return "yarn";
  if (fs.existsSync(path.join(root, "bun.lockb")) || fs.existsSync(path.join(root, "bun.lock"))) return "bun";
  return "npm";
}

function copyTree(from: string, to: string, atRoot = true): void {
  fs.mkdirSync(to, { recursive: true });
  for (const e of fs.readdirSync(from, { withFileTypes: true })) {
    if (e.isDirectory() && skipped(e.name, atRoot)) continue;
    const src = path.join(from, e.name);
    const dst = path.join(to, e.name);
    if (e.isDirectory()) copyTree(src, dst, false);
    else if (e.isFile()) fs.copyFileSync(src, dst);
  }
}

function sourceFiles(dir: string, out: string[], atRoot = true): void {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory()) {
      // Code in dot-folders (tool configuration) is copied but not instrumented.
      if (!skipped(e.name, atRoot) && !e.name.startsWith(".")) sourceFiles(path.join(dir, e.name), out, false);
    } else if ((CODE.test(e.name) && !e.name.endsWith(".d.ts") && !/\.config\.[cm]?[jt]s$/.test(e.name)) || e.name.endsWith(".vue")) {
      out.push(path.join(dir, e.name));
    }
  }
}

function scriptKind(file: string, lang?: string): ts.ScriptKind {
  const ext = lang ?? path.extname(file).slice(1);
  switch (ext) {
    case "tsx":
      return ts.ScriptKind.TSX;
    case "jsx":
      return ts.ScriptKind.JSX;
    case "js":
    case "mjs":
    case "cjs":
      return ts.ScriptKind.JS;
    default:
      return ts.ScriptKind.TS;
  }
}

type Instrumenter = (rel: string, methods: MethodMap[]) => ts.TransformerFactory<ts.SourceFile>;

const printer = ts.createPrinter({ newLine: ts.NewLineKind.LineFeed });

/** Instruments `text` (the file `file`, shown as `rel`) and prints it back as source. */
function instrumentText(file: string, rel: string, text: string, kind: ts.ScriptKind, methods: MethodMap[], instrumenter: Instrumenter): string | undefined {
  const before = methods.length;
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, kind);
  const result = ts.transform(sf, [instrumenter(rel, methods)]);
  try {
    if (methods.length === before) return undefined;
    return printer.printFile(result.transformed[0] as ts.SourceFile);
  } finally {
    result.dispose();
  }
}

/** The text with everything outside [start, end) blanked, keeping UTF-8 byte offsets. */
function blankOutside(text: string, start: number, end: number): string {
  let out = "";
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if ((i >= start && i < end) || ch === "\n" || ch === "\r") {
      out += ch;
      continue;
    }
    const code = text.charCodeAt(i);
    const bytes = code < 0x80 ? 1 : code < 0x800 ? 2 : code >= 0xd800 && code <= 0xdbff ? 4 : code >= 0xdc00 && code <= 0xdfff ? 0 : 3;
    out += " ".repeat(bytes);
  }
  return out;
}

export interface BrowserResult {
  mode: "browser";
  /** Program and arguments of the dev server. */
  command: string[];
  cwd: string;
  map: string;
  description: string;
}

export function instrumentBrowser(root: string, out: string, wsUrl: string, runtimeSrc: string, instrumenter: Instrumenter): BrowserResult {
  const say = (line: string) => process.stdout.write(`${line}\n`);
  const pkg = readPackage(root);
  const script = pkg ? devScript(pkg) : undefined;
  if (!pkg || !script) throw new Error('no dev server script ("dev", "start" or "serve" running vite, ng serve, webpack serve, …) in package.json');
  if (!fs.existsSync(path.join(root, "node_modules"))) throw new Error("node_modules is missing: install the project's dependencies first (npm install)");

  const copy = path.join(out, "web", "src");
  say("Copying the project …");
  fs.rmSync(copy, { recursive: true, force: true });
  copyTree(root, copy);
  fs.symlinkSync(path.join(root, "node_modules"), path.join(copy, "node_modules"), "junction");

  const runtime = path.join(copy, "__nodex_runtime.js");
  fs.writeFileSync(runtime, fs.readFileSync(runtimeSrc, "utf8").replace('const WS_URL = "__NODEX_WS_URL__";', `const WS_URL = ${JSON.stringify(wsUrl)};`));

  const methods: MethodMap[] = [];
  const files: string[] = [];
  sourceFiles(root, files);
  let changed = 0;
  for (const file of files) {
    const rel = path.relative(root, file).replace(/\\/g, "/");
    const target = path.join(copy, rel);
    const importPath = (() => {
      let r = path.relative(path.dirname(target), runtime).replace(/\\/g, "/");
      if (!r.startsWith(".")) r = `./${r}`;
      return r;
    })();
    const text = fs.readFileSync(file, "utf8");
    let output: string | undefined;
    if (file.endsWith(".vue")) {
      output = instrumentVue(file, rel, text, importPath, methods, instrumenter);
    } else {
      const printed = instrumentText(file, rel, text, scriptKind(file), methods, instrumenter);
      if (printed !== undefined) output = `import "${importPath}";\n${printed}`;
    }
    if (output !== undefined) {
      fs.writeFileSync(target, output);
      changed++;
    }
  }
  say(`Instrumented ${changed} files, ${methods.filter((m) => m.kind !== "resume").length} functions`);
  const map = path.join(out, "nodex-trace-map.json");
  fs.writeFileSync(map, JSON.stringify({ version: 1, methods }));

  const manager = packageManager(root);
  // npm, pnpm and yarn are `.cmd` shims on Windows; bun is a real executable.
  const program = process.platform === "win32" && manager !== "bun" ? `${manager}.cmd` : manager;
  return {
    mode: "browser",
    command: [program, "run", script],
    cwd: copy,
    map,
    description: `${manager} run ${script} (browser app, instrumented)`,
  };
}

/** Instruments the <script> blocks of a Vue single-file component. */
function instrumentVue(file: string, rel: string, text: string, importPath: string, methods: MethodMap[], instrumenter: Instrumenter): string | undefined {
  const re = /<script\b([^>]*)>([\s\S]*?)<\/script>/g;
  const blocks: { start: number; end: number; lang?: string }[] = [];
  for (let m = re.exec(text); m; m = re.exec(text)) {
    const attrs = m[1];
    const contentStart = m.index + m[0].indexOf(">") + 1;
    blocks.push({ start: contentStart, end: contentStart + m[2].length, lang: /lang=["']([a-z]+)["']/.exec(attrs)?.[1] });
  }
  if (blocks.length === 0) return undefined;
  let result = text;
  let instrumented = false;
  // Replace from the last block so earlier offsets stay valid.
  for (const b of [...blocks].reverse()) {
    const printed = instrumentText(file, rel, blankOutside(text, b.start, b.end), scriptKind(file, b.lang ?? "js"), methods, instrumenter);
    if (printed === undefined) continue;
    instrumented = true;
    result = `${result.slice(0, b.start)}\nimport "${importPath}";\n${printed}\n${result.slice(b.end)}`;
  }
  return instrumented ? result : undefined;
}

/** Instruments one changed file again into a running copy; returns the methods it added. */
export function reinstrumentFile(root: string, out: string, file: string, firstId: number, instrumenter: Instrumenter): MethodMap[] {
  const copy = path.join(out, "web", "src");
  const runtime = path.join(copy, "__nodex_runtime.js");
  const rel = path.relative(root, file).replace(/\\/g, "/");
  const target = path.join(copy, rel);
  if (!fs.existsSync(file)) {
    fs.rmSync(target, { force: true });
    return [];
  }
  // Ids are `methods.length + 1`: placeholders make the new ones continue after the map's.
  const methods: MethodMap[] = Array.from({ length: firstId - 1 }, () => ({}) as MethodMap);
  let importPath = path.relative(path.dirname(target), runtime).replace(/\\/g, "/");
  if (!importPath.startsWith(".")) importPath = `./${importPath}`;
  const text = fs.readFileSync(file, "utf8");
  let output: string | undefined;
  if (file.endsWith(".vue")) output = instrumentVue(file, rel, text, importPath, methods, instrumenter);
  else {
    const printed = instrumentText(file, rel, text, scriptKind(file), methods, instrumenter);
    if (printed !== undefined) output = `import "${importPath}";\n${printed}`;
  }
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, output ?? text);
  return methods.slice(firstId - 1);
}
