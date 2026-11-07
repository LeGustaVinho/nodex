// Semantic facts for Nodex (JSON contract: crates/nodex-ir/src/semantic.rs, snake_case).
//
// - call / construct: what a call or `new` resolves to, following imports and re-exports;
// - base: the class or interface named in `extends` / `implements`;
// - locals: inferred types of variables and parameters written without a type.
// Sites and declarations are UTF-8 byte ranges of the files on disk.

import ts from "typescript";
import { type ByteOffset, byteOffsets } from "./positions.js";
import type { ProjectHost } from "./project.js";

export interface ByteRange {
  file: string;
  start: number;
  end: number;
}

export interface SemanticTarget {
  decl: ByteRange | null;
  display: string;
  container: string | null;
  kind: string;
  /** True for standard collection operations (the array/iterator/promise methods of the default library). */
  linq: boolean;
  external: boolean;
}

export interface SemanticRef {
  site: ByteRange;
  kind: "call" | "construct" | "base";
  target: SemanticTarget;
}

export interface LocalTypeFact {
  site: ByteRange;
  name: string;
  ty: string;
}

export interface SemanticFacts {
  source: string;
  refs: SemanticRef[];
  locals: LocalTypeFact[];
  messages: string[];
}

/** Containers whose methods form pipelines (`xs.filter(…).map(…)`, `p.then(…)`). */
const PIPELINE_CONTAINERS = new Set([
  "Array", "ReadonlyArray", "Iterator", "IteratorObject", "AsyncIterator", "Promise", "PromiseLike",
  "Map", "ReadonlyMap", "Set", "ReadonlySet", "String",
]);

const MAX_TYPE = 80;

/** Facts of one file, reused while neither the file nor what it refers to changes. */
export interface FileFacts {
  sf: ts.SourceFile;
  rel: string;
  refs: SemanticRef[];
  locals: LocalTypeFact[];
  unresolved: number;
}

/**
 * Collects facts for the whole project. With a `cache` from the previous run, a file is analyzed
 * again only when it changed (its `SourceFile` was re-parsed) or when one of its references
 * points into a changed file (declaration positions may have moved).
 */
export type FactsCache = Map<string, FileFacts>;

export function collectFacts(project: ProjectHost, cache: FactsCache = new Map()): SemanticFacts {
  const offsets = new Map<string, ByteOffset>();
  const offsetsOf = (sf: ts.SourceFile): ByteOffset => {
    let o = offsets.get(sf.fileName);
    if (!o) {
      o = byteOffsets(sf.text, sf.fileName);
      offsets.set(sf.fileName, o);
    }
    return o;
  };
  const range = (node: ts.Node, sf = node.getSourceFile()): ByteRange | null => {
    const file = project.relative(sf.fileName);
    if (!file) return null;
    const o = offsetsOf(sf);
    return { file, start: o(node.getStart(sf)), end: o(node.getEnd()) };
  };

  const files = project.sourceFiles();
  const changed = new Set<string>();
  for (const { file: sf } of files) {
    const cached = cache.get(sf.fileName);
    if (!cached || cached.sf !== sf) changed.add(project.relative(sf.fileName) ?? sf.fileName);
  }
  const live = new Set(files.map((f) => f.file.fileName));
  for (const key of [...cache.keys()]) {
    if (!live.has(key)) {
      changed.add(cache.get(key)!.rel);
      cache.delete(key);
    }
  }

  for (const { file: sf, program } of files) {
    const cached = cache.get(sf.fileName);
    if (cached && cached.sf === sf && !cached.refs.some((r) => r.target.decl && changed.has(r.target.decl.file))) continue;
    const refs: SemanticRef[] = [];
    const locals: LocalTypeFact[] = [];
    let unresolved = 0;
    const checker = program.getTypeChecker();
    const target = (decl: ts.Declaration | undefined, fallbackName: string): SemanticTarget | null => {
      if (!decl) return null;
      return describe(decl, fallbackName, checker, program, project, offsetsOf);
    };

    const visit = (node: ts.Node): void => {
      if (ts.isCallExpression(node) && node.expression.kind !== ts.SyntaxKind.ImportKeyword && !isRequire(node)) {
        const site = range(node, sf);
        const decl = callDeclaration(node, checker);
        const t = target(decl, node.expression.getText(sf));
        if (site && t) refs.push({ site, kind: "call", target: t });
        else if (site && !decl) unresolved++;
      } else if (ts.isNewExpression(node)) {
        const site = range(node, sf);
        const decl = constructDeclaration(node, checker);
        const t = target(decl, node.expression.getText(sf));
        if (site && t) refs.push({ site, kind: "construct", target: t });
      } else if (ts.isHeritageClause(node)) {
        for (const ty of node.types) {
          const site = range(ty, sf);
          const symbol = resolveAlias(checker.getSymbolAtLocation(ty.expression), checker);
          const decl = symbol?.declarations?.find(isTypeDeclaration) ?? symbol?.declarations?.[0];
          const t = decl
            ? describe(decl, ty.expression.getText(sf), checker, program, project, offsetsOf)
            : { decl: null, display: ty.expression.getText(sf), container: null, kind: "type", linq: false, external: true };
          if (site && t) refs.push({ site, kind: "base", target: { ...t, kind: "type" } });
        }
      } else if (ts.isVariableDeclaration(node) && !node.type) {
        bindingFacts(node.name, checker, sf, range, locals);
      } else if (ts.isParameter(node) && !node.type && ts.isIdentifier(node.name) && isBlueprintFunction(node.parent)) {
        bindingFacts(node.name, checker, sf, range, locals);
      }
      ts.forEachChild(node, visit);
    };
    visit(sf);
    cache.set(sf.fileName, { sf, rel: project.relative(sf.fileName) ?? sf.fileName, refs, locals, unresolved });
  }

  const refs: SemanticRef[] = [];
  const locals: LocalTypeFact[] = [];
  let unresolved = 0;
  for (const f of cache.values()) {
    refs.push(...f.refs);
    locals.push(...f.locals);
    unresolved += f.unresolved;
  }
  const messages = [...project.messages];
  if (unresolved > 0) messages.push(`${unresolved} calls could not be resolved`);
  return { source: project.describe(), refs, locals, messages };
}

function isRequire(node: ts.CallExpression): boolean {
  return ts.isIdentifier(node.expression) && node.expression.text === "require";
}

/** Functions that get their own Blueprint in Nodex: their parameters are pins of the Start node. */
function isBlueprintFunction(node: ts.Node): boolean {
  return (
    ts.isFunctionDeclaration(node) ||
    ts.isMethodDeclaration(node) ||
    ts.isConstructorDeclaration(node) ||
    ts.isSetAccessorDeclaration(node) ||
    ((ts.isArrowFunction(node) || ts.isFunctionExpression(node)) &&
      (ts.isVariableDeclaration(node.parent) || ts.isPropertyDeclaration(node.parent) || ts.isBinaryExpression(node.parent)))
  );
}

function bindingFacts(
  name: ts.BindingName,
  checker: ts.TypeChecker,
  sf: ts.SourceFile,
  range: (n: ts.Node, sf?: ts.SourceFile) => ByteRange | null,
  out: LocalTypeFact[],
): void {
  if (ts.isIdentifier(name)) {
    const site = range(name, sf);
    if (!site) return;
    const ty = checker.typeToString(checker.getTypeAtLocation(name), undefined, ts.TypeFormatFlags.UseAliasDefinedOutsideCurrentScope);
    if (ty && ty !== "any") out.push({ site, name: name.text, ty: ty.length > MAX_TYPE ? `${ty.slice(0, MAX_TYPE - 1)}…` : ty });
    return;
  }
  for (const element of name.elements) {
    if (ts.isBindingElement(element)) bindingFacts(element.name, checker, sf, range, out);
  }
}

function resolveAlias(symbol: ts.Symbol | undefined, checker: ts.TypeChecker): ts.Symbol | undefined {
  if (symbol && symbol.flags & ts.SymbolFlags.Alias) {
    try {
      return checker.getAliasedSymbol(symbol);
    } catch {
      return symbol;
    }
  }
  return symbol;
}

function callDeclaration(node: ts.CallExpression, checker: ts.TypeChecker): ts.Declaration | undefined {
  const signature = checker.getResolvedSignature(node);
  const decl = signature?.getDeclaration();
  if (decl && !ts.isJSDocSignature(decl)) return decl as ts.Declaration;
  const callee = ts.isPropertyAccessExpression(node.expression) ? node.expression.name : node.expression;
  return resolveAlias(checker.getSymbolAtLocation(callee), checker)?.declarations?.[0];
}

function constructDeclaration(node: ts.NewExpression, checker: ts.TypeChecker): ts.Declaration | undefined {
  const signature = checker.getResolvedSignature(node);
  const decl = signature?.getDeclaration();
  if (decl && ts.isConstructorDeclaration(decl)) return decl;
  // No explicit constructor: the class itself.
  const symbol = resolveAlias(checker.getSymbolAtLocation(node.expression), checker);
  return symbol?.declarations?.find(isTypeDeclaration) ?? symbol?.declarations?.[0] ?? (decl as ts.Declaration | undefined);
}

function isTypeDeclaration(d: ts.Declaration): boolean {
  return ts.isClassLike(d) || ts.isInterfaceDeclaration(d) || ts.isTypeAliasDeclaration(d) || ts.isEnumDeclaration(d);
}

/** The node whose name identifies a function value: `const f = () => …` → the declaration. */
function named(decl: ts.Declaration): ts.Declaration {
  if ((ts.isArrowFunction(decl) || ts.isFunctionExpression(decl)) && !(decl as ts.FunctionExpression).name) {
    const p = decl.parent;
    if (ts.isVariableDeclaration(p) || ts.isPropertyDeclaration(p) || ts.isPropertyAssignment(p)) return p;
  }
  return decl;
}

function nameOf(decl: ts.Declaration, fallback: string): string {
  const name = ts.getNameOfDeclaration(decl);
  if (name) return ts.isIdentifier(name) || ts.isPrivateIdentifier(name) ? name.text : name.getText();
  if (ts.isConstructorDeclaration(decl)) return "constructor";
  return fallback;
}

function containerOf(decl: ts.Declaration): string | null {
  let cur: ts.Node | undefined = decl.parent;
  while (cur) {
    if (ts.isClassLike(cur) || ts.isInterfaceDeclaration(cur)) {
      return cur.name ? cur.name.text : "(class)";
    }
    if (ts.isModuleDeclaration(cur)) return cur.name.getText();
    if (ts.isSourceFile(cur)) {
      // Globals of the standard library and of type packages (`setTimeout`) have no container.
      if (cur.isDeclarationFile) return null;
      const base = cur.fileName.replace(/\\/g, "/").split("/").pop() ?? "";
      return base.replace(/\.(d\.)?[cm]?[jt]sx?$/, "");
    }
    cur = cur.parent;
  }
  return null;
}

function kindOf(decl: ts.Declaration): string {
  if (isTypeDeclaration(decl)) return "type";
  if (ts.isConstructorDeclaration(decl)) return "constructor";
  if (ts.isMethodDeclaration(decl) || ts.isMethodSignature(decl)) return "method";
  if (ts.isGetAccessorDeclaration(decl) || ts.isSetAccessorDeclaration(decl) || ts.isPropertyDeclaration(decl) || ts.isPropertySignature(decl)) {
    return "property";
  }
  return "function";
}

function describe(
  raw: ts.Declaration,
  fallbackName: string,
  checker: ts.TypeChecker,
  program: ts.Program,
  project: ProjectHost,
  offsetsOf: (sf: ts.SourceFile) => ByteOffset,
): SemanticTarget {
  const decl = named(raw);
  const sf = decl.getSourceFile();
  const external = sf.isDeclarationFile || !project.isProjectFile(sf.fileName);
  const name = nameOf(decl, fallbackName.split(".").pop() ?? fallbackName);
  const container = containerOf(decl);
  const kind = kindOf(decl);
  let params = "";
  const fn = ts.isFunctionLike(raw) ? raw : undefined;
  if (fn && kind !== "type") {
    params = `(${fn.parameters
      .map((p) => {
        const ty = p.type ? p.type.getText(p.getSourceFile()) : checker.typeToString(checker.getTypeAtLocation(p));
        return p.dotDotDotToken ? `...${ty}` : ty;
      })
      .join(", ")})`;
  }
  const display = `${container && kind !== "type" ? `${container}.` : ""}${name}${params}`;
  let declRange: ByteRange | null = null;
  if (!external) {
    const file = project.relative(sf.fileName);
    const anchor = ts.getNameOfDeclaration(decl) ?? constructorKeyword(decl) ?? decl;
    if (file) {
      const o = offsetsOf(sf);
      declRange = { file, start: o(anchor.getStart(sf)), end: o(decl.getEnd()) };
    }
  }
  const linq = external && program.isSourceFileDefaultLibrary(sf) && container !== null && PIPELINE_CONTAINERS.has(container);
  return { decl: declRange, display: display.length > 120 ? `${display.slice(0, 119)}…` : display, container, kind, linq, external };
}

function constructorKeyword(decl: ts.Declaration): ts.Node | undefined {
  if (!ts.isConstructorDeclaration(decl)) return undefined;
  return decl.getChildren().find((c) => c.kind === ts.SyntaxKind.ConstructorKeyword);
}
