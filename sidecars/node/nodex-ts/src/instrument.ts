// `nodex-ts instrument`: writes an instrumented JavaScript copy of a TypeScript/JavaScript
// project and the trace map Nodex uses to place events on the Blueprints.
//
// The functions that have a Blueprint in Nodex (named functions, methods, constructors,
// accessors, functions bound to constants, the module's top-level code) get probes:
//
//   function f(a) { S1; S2 }
//   →
//   function f(a) {
//     let __nx_m = 7, __nx_r = 0; __nx.enter(7); __nx.arg(__nx_m, 0, a);
//     try { __nx.step(__nx_m, 1); S1; __nx.step(__nx_m, 2); S2 }
//     catch (__nx_e) { __nx.thrown(__nx_e); throw __nx_e; }
//     finally { if (!__nx_r) __nx.exit(__nx_m); }
//   }
//
// `return v` → `return __nx.ret(__nx_m, v, __nx_r = 1)`. Like the .NET state machines, an
// `await`/`yield` ends the frame and the continuation runs as a "resume" method:
// `await x` → `__nx.rs(R, await __nx.aw(__nx_m, x), __nx_m = R)`.
// Each step's point is the UTF-8 byte span of the statement in the original file.

import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { type ByteOffset, byteOffsets } from "./positions.js";
import { ProjectHost } from "./project.js";

interface PointMap {
  file: string;
  start: number;
  end: number;
}

interface MethodMap {
  id: number;
  name: string;
  kind: "method" | "constructor" | "resume";
  points: PointMap[];
  locals: Record<string, string>;
  params: string[];
}

interface Method {
  map: MethodMap;
  /** Local name → slot. */
  slots: Map<string, number>;
  resume?: MethodMap;
}

const IGNORED_DIRS = new Set(["node_modules", ".git", "dist", "build", "out", "coverage", ".next", ".turbo", ".nodex", "bin", "obj", "target"]);
const COPY = /\.(js|mjs|cjs|json)$/;

export function instrumentCommand(args: string[]): number {
  const opt = (name: string) => {
    const i = args.indexOf(`--${name}`);
    return i >= 0 ? args[i + 1] : undefined;
  };
  const root = opt("root");
  const out = opt("out");
  if (!root || !out) {
    console.error("usage: nodex-ts instrument --root <project> --out <dir> [--runtime <runtime.cjs>]");
    return 2;
  }
  try {
    const result = instrument(path.resolve(root), path.resolve(out), opt("runtime"));
    process.stdout.write(`NODEX-RESULT ${JSON.stringify(result)}\n`);
    return 0;
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err));
    return 1;
  }
}

export interface InstrumentResult {
  /** The instrumented entry file to run with Node. */
  entry: string;
  /** The runtime to preload (`node --require`). */
  runtime: string;
  map: string;
  description: string;
}

export function instrument(root: string, out: string, runtimeArg?: string): InstrumentResult {
  const say = (line: string) => process.stdout.write(`${line}\n`);
  const project = new ProjectHost(root);
  project.open();
  say(`Loaded ${project.describe()}`);

  const jsOut = path.join(out, "js");
  fs.rmSync(jsOut, { recursive: true, force: true });
  fs.mkdirSync(jsOut, { recursive: true });

  const methods: MethodMap[] = [];
  const done = new Set<string>();
  const programs = project.programsWith((options, loose) => {
    const o: ts.CompilerOptions = {
      ...options,
      outDir: jsOut,
      rootDir: root,
      noEmit: false,
      emitDeclarationOnly: false,
      declaration: false,
      declarationMap: false,
      sourceMap: false,
      inlineSourceMap: false,
      inlineSources: false,
      composite: false,
      incremental: false,
      tsBuildInfoFile: undefined,
      noEmitOnError: false,
      outFile: undefined,
    };
    if (options.allowImportingTsExtensions) o.rewriteRelativeImportExtensions = true;
    if (loose) o.module = ts.ModuleKind.Preserve;
    return o;
  });
  let emitted = 0;
  for (const program of programs) {
    for (const sf of program.getSourceFiles()) {
      if (sf.isDeclarationFile || done.has(sf.fileName) || !project.isProjectFile(sf.fileName)) continue;
      done.add(sf.fileName);
      const rel = project.relative(sf.fileName)!;
      const result = program.emit(sf, writeFile, undefined, false, { before: [instrumenter(rel, methods)] });
      if (result.emitSkipped) say(`  skipped ${rel}`);
      else emitted++;
    }
  }
  say(`Instrumented ${emitted} files, ${methods.filter((m) => m.kind !== "resume").length} functions`);

  copyRuntimeFiles(root, jsOut);

  const runtimeSrc = runtimeArg ?? defaultRuntime();
  const runtime = path.join(out, "nodex-trace.cjs");
  fs.copyFileSync(runtimeSrc, runtime);
  const map = path.join(out, "nodex-trace-map.json");
  fs.writeFileSync(map, JSON.stringify({ version: 1, methods }));

  const source = findEntry(root, project);
  const entry = path.join(jsOut, emittedName(source));
  if (!fs.existsSync(entry)) throw new Error(`the entry ${source} was not emitted (${entry})`);
  return { entry, runtime, map, description: `${source} (Node.js, instrumented)` };
}

function writeFile(fileName: string, text: string): void {
  fs.mkdirSync(path.dirname(fileName), { recursive: true });
  fs.writeFileSync(fileName, text);
}

function defaultRuntime(): string {
  // dist/instrument.js → ../../nodex-trace/runtime.cjs (repository and installed layouts).
  const here = path.dirname(fileURLToPath(import.meta.url));
  return path.resolve(here, "..", "..", "nodex-trace", "runtime.cjs");
}

/** JavaScript and JSON files the compiler did not emit (plain .js helpers, data files). */
function copyRuntimeFiles(root: string, jsOut: string): void {
  const walk = (dir: string) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) {
        if (!IGNORED_DIRS.has(e.name) && !e.name.startsWith(".")) walk(full);
      } else if (COPY.test(e.name) && !/tsconfig.*\.json$/.test(e.name)) {
        const target = path.join(jsOut, path.relative(root, full));
        if (!fs.existsSync(target)) {
          fs.mkdirSync(path.dirname(target), { recursive: true });
          fs.copyFileSync(full, target);
        }
      }
    }
  };
  walk(root);
}

function emittedName(source: string): string {
  return source.replace(/\.tsx$/, ".js").replace(/\.ts$/, ".js").replace(/\.mts$/, ".mjs").replace(/\.cts$/, ".cjs");
}

/** The source file Node should run: package.json `nodex.entry`, `main`/`bin`, `scripts.start`, or a conventional name. */
function findEntry(root: string, project: ProjectHost): string {
  const sources = new Set(project.sourceFiles().map((s) => project.relative(s.file.fileName)!));
  const exists = (rel: string) => sources.has(rel) || fs.existsSync(path.join(root, rel));
  const toSource = (built: string): string | undefined => {
    const rel = path.normalize(built).replace(/\\/g, "/").replace(/^\.\//, "");
    if (/\.([cm]?ts|tsx)$/.test(rel) && exists(rel)) return rel;
    const candidates = [rel];
    for (const layout of project.layouts()) {
      if (!layout.outDir) continue;
      const outRel = path.relative(root, layout.outDir).replace(/\\/g, "/");
      if (rel === outRel || rel.startsWith(`${outRel}/`)) {
        const inner = rel.slice(outRel.length + 1);
        const srcRel = layout.rootDir ? path.relative(root, layout.rootDir).replace(/\\/g, "/") : "";
        candidates.push(srcRel ? `${srcRel}/${inner}` : inner);
      }
    }
    for (const c of candidates) {
      const stem = c.replace(/\.[cm]?js$/, "");
      for (const ext of [".ts", ".tsx", ".mts", ".cts", ".js", ".mjs", ".cjs"]) {
        if (sources.has(stem + ext)) return stem + ext;
      }
      if (exists(c) && COPY.test(c)) return c;
    }
    return undefined;
  };

  const pkgPath = path.join(root, "package.json");
  if (fs.existsSync(pkgPath)) {
    const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8")) as {
      nodex?: { entry?: string };
      main?: string;
      bin?: string | Record<string, string>;
      scripts?: Record<string, string>;
    };
    const declared = [
      pkg.nodex?.entry,
      pkg.main,
      typeof pkg.bin === "string" ? pkg.bin : pkg.bin ? Object.values(pkg.bin)[0] : undefined,
    ];
    for (const script of [pkg.scripts?.start, pkg.scripts?.dev]) {
      const m = script?.match(/(?:^|\s)(?:node|tsx|ts-node|bun)\s+(?:-\S+\s+)*([^\s&|;]+)/);
      if (m) declared.push(m[1]);
    }
    for (const d of declared) {
      const found = d ? toSource(d) : undefined;
      if (found) return found;
    }
  }
  for (const c of ["src/main.ts", "src/index.ts", "main.ts", "index.ts", "src/main.js", "src/index.js", "index.js", "main.js"]) {
    if (sources.has(c)) return c;
  }
  throw new Error('cannot tell which file to run; add "nodex": { "entry": "src/main.ts" } to package.json');
}

// ------------------------------------------------------------------------------------------
// The transformer
// ------------------------------------------------------------------------------------------

interface Ctx {
  /** The instrumented function being visited, if any. */
  method: Method | null;
  /** Inside a function without a Blueprint (a callback): nothing is instrumented. */
  opaque: boolean;
  /** Module or namespace level. */
  topLevel: boolean;
  /** Members of a class declared at module level. */
  classTop: boolean;
  /** Qualified name prefix for method names. */
  scope: string;
}

function instrumenter(rel: string, methods: MethodMap[]): ts.TransformerFactory<ts.SourceFile> {
  return (context) => (sf) => {
    const f = context.factory;
    const o: ByteOffset = byteOffsets(sf.text, sf.fileName);
    const nx = (name: string, ...args: ts.Expression[]) =>
      f.createCallExpression(f.createPropertyAccessExpression(f.createIdentifier("__nx"), name), undefined, args);
    const num = (n: number) => f.createNumericLiteral(n);
    const M = () => f.createIdentifier("__nx_m");

    const newMethod = (name: string, kind: MethodMap["kind"], anchor: ts.Node): Method => {
      const map: MethodMap = { id: methods.length + 1, name: `${rel}#${name}`, kind, points: [], locals: {}, params: [] };
      methods.push(map);
      const m: Method = { map, slots: new Map() };
      pointOf(m, anchor);
      return m;
    };
    const pointOf = (m: Method, node: ts.Node): number => {
      const original = ts.getOriginalNode(node);
      m.map.points.push({ file: rel, start: o(original.getStart(sf)), end: o(original.getEnd()) });
      return m.map.points.length - 1;
    };
    const resumeOf = (m: Method): number => {
      if (!m.resume) {
        m.resume = { ...m.map, id: methods.length + 1, kind: "resume" };
        // Shares the points, locals and parameters of the method (same arrays).
        methods.push(m.resume);
      }
      return m.resume.id;
    };
    const slot = (m: Method, name: string): number => {
      let s = m.slots.get(name);
      if (s === undefined) {
        s = m.slots.size;
        m.slots.set(name, s);
        m.map.locals[String(s)] = name;
      }
      return s;
    };
    const stmt = (e: ts.Expression) => f.createExpressionStatement(e);
    const step = (m: Method, anchor: ts.Node) => stmt(nx("step", M(), num(pointOf(m, anchor))));
    const local = (m: Method, name: string) => stmt(nx("local", M(), num(slot(m, name)), f.createIdentifier(name)));

    const bindingNames = (name: ts.BindingName, out: string[]) => {
      if (ts.isIdentifier(name)) out.push(name.text);
      else for (const el of name.elements) if (ts.isBindingElement(el)) bindingNames(el.name, out);
    };
    /** Names assigned by an expression statement (`x = …`, `x += …`, `x++`, `[a, b] = …`). */
    const assigned = (e: ts.Expression, out: string[]) => {
      if (ts.isParenthesizedExpression(e)) return assigned(e.expression, out);
      if (ts.isBinaryExpression(e) && e.operatorToken.kind >= ts.SyntaxKind.FirstAssignment && e.operatorToken.kind <= ts.SyntaxKind.LastAssignment) {
        const target = e.left;
        if (ts.isIdentifier(target)) out.push(target.text);
        else if (ts.isArrayLiteralExpression(target) || ts.isObjectLiteralExpression(target)) {
          const walk = (n: ts.Node) => {
            if (ts.isIdentifier(n) && !(ts.isPropertyAssignment(n.parent) && n.parent.name === n)) out.push(n.text);
            else if (ts.isShorthandPropertyAssignment(n)) out.push(n.name.text);
            else ts.forEachChild(n, walk);
          };
          walk(target);
        }
      } else if ((ts.isPrefixUnaryExpression(e) || ts.isPostfixUnaryExpression(e)) && ts.isIdentifier(e.operand)) {
        out.push(e.operand.text);
      }
    };
    const localsAfter = (m: Method, s: ts.Statement): ts.Statement[] => {
      const names: string[] = [];
      if (ts.isVariableStatement(s) && !s.modifiers?.some((x) => x.kind === ts.SyntaxKind.DeclareKeyword)) {
        for (const d of s.declarationList.declarations) if (d.initializer) bindingNames(d.name, names);
      } else if (ts.isExpressionStatement(s)) {
        assigned(s.expression, names);
      }
      return names.filter((n) => m.slots.has(n)).map((n) => local(m, n));
    };

    const skipStep = (s: ts.Statement) =>
      ts.isEmptyStatement(s) ||
      ts.isInterfaceDeclaration(s) ||
      ts.isTypeAliasDeclaration(s) ||
      ts.isImportDeclaration(s) ||
      ts.isImportEqualsDeclaration(s) ||
      ts.isExportDeclaration(s) ||
      ts.isModuleDeclaration(s) ||
      (ts.isVariableStatement(s) && !!s.modifiers?.some((x) => x.kind === ts.SyntaxKind.DeclareKeyword)) ||
      (ts.isExpressionStatement(s) && ts.isStringLiteral(s.expression));
    const anchorOf = (s: ts.Statement): ts.Statement => (ts.isLabeledStatement(s) ? anchorOf(s.statement) : s);

    const stepped = (stmts: readonly ts.Statement[], ctx: Ctx): ts.Statement[] => {
      const m = ctx.method!;
      const out: ts.Statement[] = [];
      for (const s of stmts) {
        if (skipStep(s)) {
          out.push(...visitStatements(s, ctx));
          continue;
        }
        out.push(step(m, anchorOf(s)));
        out.push(...visitStatements(s, ctx));
        out.push(...localsAfter(m, s));
      }
      return out;
    };
    const visitStatements = (s: ts.Statement, ctx: Ctx): ts.Statement[] => {
      const r = visit(s, ctx);
      if (!r) return [];
      return (Array.isArray(r) ? r : [r]) as ts.Statement[];
    };
    /** A statement used as a body (`if (c) x();`) becomes a block with its step. */
    const asBlock = (s: ts.Statement, ctx: Ctx, prefix: ts.Statement[] = []): ts.Statement => {
      if (ts.isBlock(s)) return f.updateBlock(s, [...prefix, ...stepped(s.statements, ctx)]);
      return f.createBlock([...prefix, ...stepped([s], ctx)], true);
    };

    /** Locals declared in a function body (not in nested functions), so assignments can be recorded. */
    const declareLocals = (m: Method, body: ts.Node) => {
      const walk = (n: ts.Node) => {
        if (ts.isVariableDeclaration(n)) {
          const names: string[] = [];
          bindingNames(n.name, names);
          for (const name of names) slot(m, name);
        }
        if (n !== body && ts.isFunctionLike(n)) return;
        if (ts.isClassLike(n)) return;
        ts.forEachChild(n, walk);
      };
      walk(body);
    };

    const isDerivedConstructor = (fn: ts.Node) =>
      ts.isConstructorDeclaration(fn) &&
      !!(fn.parent as ts.ClassLikeDeclaration).heritageClauses?.some((h) => h.token === ts.SyntaxKind.ExtendsKeyword);

    /** Wraps a function body with the probes. */
    const instrumentFunction = <T extends ts.FunctionLikeDeclaration | ts.ClassStaticBlockDeclaration>(fn: T, name: string, ctx: Ctx): T => {
      const body = fn.body!;
      const kind = ts.isConstructorDeclaration(fn) ? "constructor" : "method";
      const m = newMethod(name, kind, body);
      const inner: Ctx = { method: m, opaque: false, topLevel: false, classTop: false, scope: name };
      const params = ts.isClassStaticBlockDeclaration(fn) ? [] : fn.parameters;
      const argStatements: ts.Statement[] = [];
      params.forEach((p, i) => {
        if (ts.isIdentifier(p.name)) {
          m.map.params[i] = p.name.text;
          slot(m, p.name.text);
          argStatements.push(stmt(nx("arg", M(), num(i), f.createIdentifier(p.name.text))));
        } else {
          m.map.params[i] = p.name.getText(sf);
        }
      });
      declareLocals(m, body);

      let statements: ts.Statement[];
      let directives: ts.Statement[] = [];
      if (ts.isBlock(body)) {
        const all = [...body.statements];
        while (all.length && ts.isExpressionStatement(all[0]) && ts.isStringLiteral(all[0].expression)) directives.push(all.shift()!);
        statements = stepped(all, inner);
      } else {
        // `(x) => x * 2` → a block that steps and returns.
        const value = visit(body, inner) as ts.Expression;
        statements = [step(m, body), f.createReturnStatement(nx("ret", M(), value, f.createAssignment(f.createIdentifier("__nx_r"), num(1))))];
        directives = [];
      }
      const prologue = [
        f.createVariableStatement(
          undefined,
          f.createVariableDeclarationList(
            [f.createVariableDeclaration("__nx_m", undefined, undefined, num(m.map.id)), f.createVariableDeclaration("__nx_r", undefined, undefined, num(0))],
            ts.NodeFlags.Let,
          ),
        ),
        stmt(nx("enter", num(m.map.id))),
        ...argStatements,
      ];
      let wrapped: ts.Statement[];
      if (isDerivedConstructor(fn)) {
        // `super(...)` must stay a top-level statement of the constructor.
        wrapped = [...prologue, ...statements, stmt(nx("exit", M()))];
      } else {
        const e = f.createIdentifier("__nx_e");
        wrapped = [
          ...prologue,
          f.createTryStatement(
            f.createBlock(statements, true),
            f.createCatchClause(
              f.createVariableDeclaration(e),
              f.createBlock([stmt(nx("thrown", e)), f.createThrowStatement(e)], true),
            ),
            f.createBlock([f.createIfStatement(f.createPrefixUnaryExpression(ts.SyntaxKind.ExclamationToken, f.createIdentifier("__nx_r")), stmt(nx("exit", M())))], true),
          ),
        ];
      }
      const newBody = f.createBlock([...directives, ...wrapped], true);
      // Parameters may contain default values with functions: visit them without instrumenting.
      const visitParams = (ps: ts.NodeArray<ts.ParameterDeclaration>) =>
        ts.visitNodes(ps, (p) => visit(p, { ...ctx, method: null, opaque: true }) as ts.ParameterDeclaration) as ts.NodeArray<ts.ParameterDeclaration>;
      if (ts.isFunctionDeclaration(fn))
        return f.updateFunctionDeclaration(fn, fn.modifiers, fn.asteriskToken, fn.name, fn.typeParameters, visitParams(fn.parameters), fn.type, newBody) as unknown as T;
      if (ts.isMethodDeclaration(fn))
        return f.updateMethodDeclaration(fn, fn.modifiers, fn.asteriskToken, fn.name, fn.questionToken, fn.typeParameters, visitParams(fn.parameters), fn.type, newBody) as unknown as T;
      if (ts.isConstructorDeclaration(fn)) return f.updateConstructorDeclaration(fn, fn.modifiers, visitParams(fn.parameters), newBody) as unknown as T;
      if (ts.isGetAccessorDeclaration(fn))
        return f.updateGetAccessorDeclaration(fn, fn.modifiers, fn.name, visitParams(fn.parameters), fn.type, newBody) as unknown as T;
      if (ts.isSetAccessorDeclaration(fn)) return f.updateSetAccessorDeclaration(fn, fn.modifiers, fn.name, visitParams(fn.parameters), newBody) as unknown as T;
      if (ts.isClassStaticBlockDeclaration(fn)) return f.updateClassStaticBlockDeclaration(fn, newBody) as unknown as T;
      if (ts.isFunctionExpression(fn))
        return f.updateFunctionExpression(fn, fn.modifiers, fn.asteriskToken, fn.name, fn.typeParameters, visitParams(fn.parameters), fn.type, newBody) as unknown as T;
      if (ts.isArrowFunction(fn))
        return f.updateArrowFunction(fn, fn.modifiers, fn.typeParameters, visitParams(fn.parameters), fn.type, fn.equalsGreaterThanToken, newBody) as unknown as T;
      return fn;
    };

    /** The Blueprint name of a function value, or null when it has none in Nodex. */
    const blueprintName = (fn: ts.ArrowFunction | ts.FunctionExpression, ctx: Ctx): string | null => {
      const p = fn.parent;
      const blockBody = ts.isBlock(fn.body);
      if (ts.isVariableDeclaration(p) && ts.isIdentifier(p.name)) {
        if (ctx.topLevel || (ctx.method && blockBody)) return `${ctx.scope ? `${ctx.scope}.` : ""}${p.name.text}`;
        return null;
      }
      if (ts.isPropertyDeclaration(p) && ctx.classTop) return `${ctx.scope}.${p.name.getText(sf)}`;
      if (ts.isExportAssignment(p) && ctx.topLevel) return "default";
      if (
        ts.isBinaryExpression(p) &&
        p.operatorToken.kind === ts.SyntaxKind.EqualsToken &&
        ts.isPropertyAccessExpression(p.left) &&
        ts.isExpressionStatement(p.parent) &&
        ctx.topLevel &&
        blockBody
      ) {
        return p.left.getText(sf).replace(".prototype.", ".");
      }
      return null;
    };

    const visit = (node: ts.Node, ctx: Ctx): ts.VisitResult<ts.Node> => {
      const child = (c: Ctx) => (n: ts.Node) => visit(n, c);
      const opaque: Ctx = { ...ctx, method: null, opaque: true, topLevel: false, classTop: false };

      if (ctx.opaque) {
        return ts.visitEachChild(node, child(ctx), context);
      }

      // Functions: instrumented when they have a Blueprint, otherwise opaque.
      if (ts.isFunctionDeclaration(node)) {
        if (!node.body) return node;
        const name = `${ctx.scope ? `${ctx.scope}.` : ""}${node.name?.text ?? "default"}`;
        return instrumentFunction(node, name, ctx);
      }
      if (ts.isArrowFunction(node) || ts.isFunctionExpression(node)) {
        const name = blueprintName(node, ctx);
        return name ? instrumentFunction(node, name, ctx) : ts.visitEachChild(node, child(opaque), context);
      }
      if (ts.isClassDeclaration(node) || ts.isClassExpression(node)) {
        const top = ctx.topLevel && (ts.isClassDeclaration(node) || ts.isExportAssignment(node.parent));
        if (!top) return ts.visitEachChild(node, child(opaque), context);
        const scope = `${ctx.scope ? `${ctx.scope}.` : ""}${node.name?.text ?? "default"}`;
        return ts.visitEachChild(node, child({ method: null, opaque: false, topLevel: false, classTop: true, scope }), context);
      }
      if (ts.isMethodDeclaration(node) || ts.isConstructorDeclaration(node) || ts.isGetAccessorDeclaration(node) || ts.isSetAccessorDeclaration(node)) {
        if (!node.body || !ctx.classTop) return ts.visitEachChild(node, child(opaque), context);
        const base = ts.isConstructorDeclaration(node) ? "constructor" : node.name.getText(sf);
        const suffix = ts.isGetAccessorDeclaration(node) ? ".get" : ts.isSetAccessorDeclaration(node) ? ".set" : "";
        return instrumentFunction(node, `${ctx.scope}.${base}${suffix}`, ctx);
      }
      if (ts.isClassStaticBlockDeclaration(node)) {
        return ctx.classTop ? instrumentFunction(node, `${ctx.scope}.static`, ctx) : ts.visitEachChild(node, child(opaque), context);
      }
      if (ts.isPropertyDeclaration(node)) {
        // Field initializers run in the constructor; only function values are instrumented.
        return ts.visitEachChild(node, child(ctx), context);
      }
      if (ts.isObjectLiteralExpression(node) || ts.isFunctionLike(node)) {
        return ts.visitEachChild(node, child(opaque), context);
      }
      if (ts.isModuleDeclaration(node)) {
        const scope = `${ctx.scope ? `${ctx.scope}.` : ""}${node.name.getText(sf)}`;
        return ts.visitEachChild(node, child({ ...ctx, topLevel: true, scope }), context);
      }

      const m = ctx.method;
      if (m) {
        if (ts.isBlock(node)) return f.updateBlock(node, stepped(node.statements, ctx));
        if (ts.isCaseClause(node)) return f.updateCaseClause(node, visit(node.expression, ctx) as ts.Expression, stepped(node.statements, ctx));
        if (ts.isDefaultClause(node)) return f.updateDefaultClause(node, stepped(node.statements, ctx));
        if (ts.isIfStatement(node)) {
          return f.updateIfStatement(
            node,
            visit(node.expression, ctx) as ts.Expression,
            asBlock(node.thenStatement, ctx),
            node.elseStatement ? asBlock(node.elseStatement, ctx) : undefined,
          );
        }
        if (ts.isForOfStatement(node) || ts.isForInStatement(node)) {
          const names: string[] = [];
          if (ts.isVariableDeclarationList(node.initializer)) for (const d of node.initializer.declarations) bindingNames(d.name, names);
          const prefix = names.filter((n) => m.slots.has(n)).map((n) => local(m, n));
          const init = visit(node.initializer, ctx) as ts.ForInitializer;
          const expr = visit(node.expression, ctx) as ts.Expression;
          return ts.isForOfStatement(node)
            ? f.updateForOfStatement(node, node.awaitModifier, init, expr, asBlock(node.statement, ctx, prefix))
            : f.updateForInStatement(node, init, expr, asBlock(node.statement, ctx, prefix));
        }
        if (ts.isForStatement(node)) {
          return f.updateForStatement(
            node,
            node.initializer && (visit(node.initializer, ctx) as ts.ForInitializer),
            node.condition && (visit(node.condition, ctx) as ts.Expression),
            node.incrementor && (visit(node.incrementor, ctx) as ts.Expression),
            asBlock(node.statement, ctx),
          );
        }
        if (ts.isWhileStatement(node)) return f.updateWhileStatement(node, visit(node.expression, ctx) as ts.Expression, asBlock(node.statement, ctx));
        if (ts.isDoStatement(node)) return f.updateDoStatement(node, asBlock(node.statement, ctx), visit(node.expression, ctx) as ts.Expression);
        if (ts.isWithStatement(node)) return f.updateWithStatement(node, visit(node.expression, ctx) as ts.Expression, asBlock(node.statement, ctx));
        if (ts.isLabeledStatement(node) && !ts.isBlock(node.statement) && !isLoop(node.statement)) {
          return f.updateLabeledStatement(node, node.label, asBlock(node.statement, ctx));
        }
        if (ts.isCatchClause(node)) {
          const v = node.variableDeclaration;
          const prefix = v && ts.isIdentifier(v.name) ? [stmt(nx("thrown", f.createIdentifier(v.name.text)))] : [];
          return f.updateCatchClause(node, v, f.updateBlock(node.block, [...prefix, ...stepped(node.block.statements, ctx)]));
        }
        if (ts.isReturnStatement(node) && node.expression) {
          const value = visit(node.expression, ctx) as ts.Expression;
          return f.updateReturnStatement(node, nx("ret", M(), value, f.createAssignment(f.createIdentifier("__nx_r"), num(1))));
        }
        if (ts.isAwaitExpression(node)) {
          const r = resumeOf(m);
          const value = visit(node.expression, ctx) as ts.Expression;
          return nx("rs", num(r), f.createAwaitExpression(nx("aw", M(), value)), f.createAssignment(M(), num(r)));
        }
        if (ts.isYieldExpression(node)) {
          const r = resumeOf(m);
          const value = node.expression ? (visit(node.expression, ctx) as ts.Expression) : f.createVoidZero();
          const y = node.asteriskToken ? f.createYieldExpression(node.asteriskToken, nx("aw", M(), value)) : f.createYieldExpression(undefined, nx("aw", M(), value));
          return nx("rs", num(r), y, f.createAssignment(M(), num(r)));
        }
      }
      return ts.visitEachChild(node, child(ctx), context);
    };

    // Module top-level code: a method of its own when the module runs statements.
    const top = sf.statements;
    const runs = top.some(
      (s) =>
        !skipStep(s) &&
        !ts.isFunctionDeclaration(s) &&
        !ts.isClassDeclaration(s) &&
        !ts.isEnumDeclaration(s) &&
        !(ts.isVariableStatement(s) && s.declarationList.declarations.every((d) => d.initializer && (ts.isArrowFunction(d.initializer) || ts.isFunctionExpression(d.initializer)))),
    );
    const topCtx: Ctx = { method: null, opaque: false, topLevel: true, classTop: false, scope: "" };
    if (!runs) {
      return f.updateSourceFile(sf, top.flatMap((s) => visitStatements(s, topCtx)));
    }
    const first = top.find((s) => !skipStep(s)) ?? top[0];
    const m = newMethod("(module)", "method", first);
    for (const s of top) {
      if (ts.isVariableStatement(s)) for (const d of s.declarationList.declarations) {
        const names: string[] = [];
        bindingNames(d.name, names);
        for (const n of names) slot(m, n);
      }
    }
    const moduleCtx: Ctx = { ...topCtx, method: m };
    // Directives (`"use strict"`) must stay first.
    let directiveCount = 0;
    while (directiveCount < top.length && ts.isExpressionStatement(top[directiveCount]) && ts.isStringLiteral((top[directiveCount] as ts.ExpressionStatement).expression)) {
      directiveCount++;
    }
    const body: ts.Statement[] = [
      ...top.slice(0, directiveCount),
      f.createVariableStatement(undefined, f.createVariableDeclarationList([f.createVariableDeclaration("__nx_m", undefined, undefined, num(m.map.id))], ts.NodeFlags.Let)),
      stmt(nx("enter", num(m.map.id))),
    ];
    for (const s of top.slice(directiveCount)) {
      const declaration = ts.isFunctionDeclaration(s) || ts.isClassDeclaration(s) || skipStep(s);
      if (!declaration) body.push(step(m, anchorOf(s)));
      // Top-level statements belong to the module method, but nested functions and classes keep
      // their own rules (visit with the top-level context, not the method's).
      const visited = declaration || ts.isVariableStatement(s) ? visitStatements(s, topCtx) : visitStatements(s, moduleCtx);
      body.push(...visited);
      if (!declaration) body.push(...localsAfter(m, s));
    }
    body.push(stmt(nx("exit", M())));
    return f.updateSourceFile(sf, body);
  };
}

function isLoop(s: ts.Statement): boolean {
  return ts.isForStatement(s) || ts.isForOfStatement(s) || ts.isForInStatement(s) || ts.isWhileStatement(s) || ts.isDoStatement(s);
}
