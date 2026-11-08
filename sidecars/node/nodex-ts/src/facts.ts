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
  kind: "call" | "construct" | "base" | "compatible";
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
  navigation: NavigationFact[];
  implementation_contracts: ByteRange[];
  configuration_files: string[];
  messages: string[];
}

interface NavigationFact {
  site: ByteRange;
  target: SemanticTarget;
  implementation?: true;
  implementation_kind?: string;
}

/** Containers whose methods form pipelines (`xs.filter(…).map(…)`, `p.then(…)`). */
const PIPELINE_CONTAINERS = new Set([
  "Array", "ReadonlyArray", "Iterator", "IteratorObject", "AsyncIterator", "Promise", "PromiseLike",
  "Map", "ReadonlyMap", "Set", "ReadonlySet", "String",
]);

const MAX_TYPE = 80;

/** Facts of one file, reused while the project sources stay unchanged. */
export interface FileFacts {
  sf: ts.SourceFile;
  refs: SemanticRef[];
  locals: LocalTypeFact[];
  navigation: NavigationFact[];
  implementationContracts: ByteRange[];
  unresolved: number;
}

/**
 * Collects facts for the whole project. A file change can affect name binding in other files,
 * including names that were previously unresolved.
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

  const files = project.sourceFiles(true).map((file) => ({ ...file, key: JSON.stringify([file.context, file.file.fileName]) }));
  const seenPaths = new Set<string>();
  const sharedPaths = new Set<string>();
  for (const { file } of files) {
    if (seenPaths.has(file.fileName)) sharedPaths.add(file.fileName);
    seenPaths.add(file.fileName);
  }
  const shared = sharedPaths.size > 0;
  const live = new Set(files.map((file) => file.key));
  let changed = files.some(({ key, file }) => cache.get(key)?.sf !== file);
  for (const key of [...cache.keys()]) {
    if (!live.has(key)) {
      changed = true;
      cache.delete(key);
    }
  }

  for (const { file: sf, program, key } of files) {
    // References can change when another file adds an export or changes an overload.
    if (!changed && cache.has(key)) continue;
    const refs: SemanticRef[] = [];
    const locals: LocalTypeFact[] = [];
    const navigation: NavigationFact[] = [];
    const implementationContracts: ByteRange[] = [];
    const callNames = new Set<ts.Node>();
    const sharedFile = sharedPaths.has(sf.fileName);
    let unresolved = 0;
    const checker = program.getTypeChecker();
    const target = (decl: ts.Declaration | undefined, fallbackName: string): SemanticTarget | null => {
      if (!decl) return null;
      return describe(decl, fallbackName, checker, program, project, offsetsOf);
    };
    const usage = (node: ts.Node, decl: ts.Declaration | undefined, includeExternal = false) => {
      if (!decl) return;
      const variable = navigationVariableName(decl);
      if (!variable && !isNavigationDeclaration(decl)) return;
      const site = range(node, sf);
      const t = target(decl, node.getText(sf));
      if (!site || !t) return;
      const resolved = variable ? { ...t, kind: "variable", display: variable.text } : t;
      // A shared token can bind to a local member in one program and an external one in another.
      if (resolved.decl || ((sharedFile || includeExternal) && resolved.external &&
          ["field", "property", "event", "variable"].includes(resolved.kind))) {
        navigation.push({ site, target: resolved });
      }
    };
    const usagesOfSymbol = (node: ts.Node, symbol: ts.Symbol | undefined) => {
      if (!symbol) return;
      const declarations = symbol.declarations ?? [];
      const includeExternal = declarations.length > 1 && declarations.some((decl) => project.isProjectFile(decl.getSourceFile().fileName));
      for (const decl of declarations) {
        // Parameter properties have separate local and instance symbols.
        if (!isParameterProperty(decl) || (symbol.flags & ts.SymbolFlags.Property)) usage(node, decl, includeExternal);
      }
    };

    const visit = (node: ts.Node): void => {
      if (ts.isCallExpression(node) && node.expression.kind !== ts.SyntaxKind.ImportKeyword && !isRequire(node)) {
        const site = range(node, sf);
        const display = node.expression.getText(sf);
        const name = referenceName(node.expression);
        const usageSite = propertyReferenceSite(name, sf);
        const candidates = unionCallDeclarations(name, checker);
        if (candidates.length > 1) {
          callNames.add(name);
          for (const candidate of candidates) usage(usageSite, candidate);
          if (site) refs.push({ site, kind: "call", target: unresolvedCall(display) });
        } else {
          const decl = callDeclaration(node, checker);
          const t = target(decl, display);
          if (site) {
            refs.push({ site, kind: "call", target: t ?? unresolvedCall(display) });
            if (!t) unresolved++;
          }
          if (decl && !ts.isCallSignatureDeclaration(decl)) {
            if (isNavigationDeclaration(decl) || navigationVariableName(decl)) callNames.add(name);
            usage(usageSite, decl);
          }
        }
      } else if (ts.isNewExpression(node)) {
        const site = range(node, sf);
        const decl = constructDeclaration(node, checker);
        const t = target(decl, node.expression.getText(sf));
        if (site && t) refs.push({ site, kind: "construct", target: t });
        if (decl && ts.isConstructorDeclaration(decl)) usage(referenceName(node.expression), decl);
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
      if (ts.isTypeAliasDeclaration(node) && isObjectContract(checker.getTypeAtLocation(node))) {
        const site = range(node.name, sf);
        if (site) implementationContracts.push(site);
      }
      if (ts.isClassLike(node) || ts.isInterfaceDeclaration(node)) {
        const anchor = node.name ?? node.getChildren(sf).find((child) => child.kind === ts.SyntaxKind.ClassKeyword) ?? node;
        const owner = node.name ? checker.getSymbolAtLocation(node.name) : checker.getTypeAtLocation(node).getSymbol();
        const site = range(anchor, sf);
        if (owner && site) {
          const instance = checker.getDeclaredTypeOfSymbol(owner);
          const heritageTypes = [
            ...(node.heritageClauses ?? []).flatMap((clause) => clause.types),
            ...ts.getJSDocImplementsTags(node).map((tag) => tag.class),
          ];
          for (const heritage of heritageTypes) {
            const base = checker.getTypeAtLocation(heritage);
            const named = resolveAlias(checker.getSymbolAtLocation(heritage.expression), checker);
            const namedDecl = named?.declarations?.find(isTypeDeclaration);
            const baseDecl = base.getSymbol()?.declarations?.find(isTypeDeclaration);
            const aliasDecl = base.aliasSymbol?.declarations?.find(ts.isTypeAliasDeclaration);
            if (ts.isJSDocImplementsTag(heritage.parent)) usage(heritage.expression, namedDecl ?? aliasDecl ?? baseDecl);
            if (!isObjectContract(base) || !checker.isTypeAssignableTo(instance, base)) continue;
            for (const decl of new Set([namedDecl, aliasDecl, baseDecl])) {
              if (!decl) continue;
              const contract = target(decl, heritage.expression.getText(sf));
              if (!contract?.decl) continue;
              navigation.push({ site, target: contract, implementation: true });
              if (ts.isTypeAliasDeclaration(decl)) {
                const declarationName = range(decl.name);
                if (declarationName) implementationContracts.push(declarationName);
              }
            }
            collectMemberImplementations(instance, base, checker, target, navigation);
            if (ts.isClassLike(node) && ts.isHeritageClause(heritage.parent) &&
                heritage.parent.token === ts.SyntaxKind.ExtendsKeyword) {
              const derivedStatic = checker.getTypeOfSymbolAtLocation(owner, node);
              const baseStatic = checker.getTypeAtLocation(heritage.expression);
              collectMemberImplementations(derivedStatic, baseStatic, checker, target, navigation);
            }
          }
        }
      }
      if (ts.isJsxAttribute(node)) {
        const props = checker.getContextualType(node.parent);
        if (props) usagesOfSymbol(node.name, checker.getPropertyOfType(props, node.name.getText(sf)));
      } else if (ts.isIndexedAccessTypeNode(node)) {
        const key = literalPropertyKey(node.indexType, checker);
        if (key !== undefined) {
          usagesOfSymbol(node.indexType, checker.getPropertyOfType(checker.getTypeAtLocation(node.objectType), key));
        }
      } else if (ts.isBindingElement(node) && ts.isObjectBindingPattern(node.parent) &&
          !node.propertyName && !node.dotDotDotToken && ts.isIdentifier(node.name)) {
        // A shorthand binding declares a local and reads a property at the same token.
        const property = checker.getPropertyOfType(checker.getTypeAtLocation(node.parent), node.name.text);
        usagesOfSymbol(node.name, property);
      } else {
        const parent = node.parent;
        const computedKey = parent && ts.isElementAccessExpression(parent) && parent.argumentExpression === node && !isLiteralKey(node);
        if (computedKey && !callNames.has(node)) {
          const key = literalPropertyKey(node, checker);
          if (key !== undefined) {
            const receiver = checker.getNonNullableType(checker.getTypeAtLocation(parent.expression));
            usagesOfSymbol(propertyReferenceSite(node, sf), checker.getPropertyOfType(receiver, key));
          }
        }
        if (ts.isIdentifier(node) && (ts.isShorthandPropertyAssignment(parent) || ts.isPropertyAssignment(parent)) &&
            node === parent.name && isAssignmentPattern(parent.parent)) {
          usagesOfSymbol(node, checker.getPropertySymbolOfDestructuringAssignment(node));
        }
        if (isReferenceName(node) && (computedKey || !callNames.has(node))) {
          const reference = ts.isShorthandPropertyAssignment(parent)
            ? checker.getShorthandAssignmentValueSymbol(parent) : checker.getSymbolAtLocation(node);
          const property = !reference && isLiteralKey(node) && ts.isBindingElement(parent)
            ? checker.getPropertyOfType(checker.getTypeAtLocation(parent.parent), node.text) : reference;
          const symbol = resolveAlias(property, checker);
          usagesOfSymbol(node, symbol);
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(sf);
    cache.set(key, { sf, refs, locals, navigation, implementationContracts, unresolved });
  }

  const refs: SemanticRef[] = [];
  const locals: LocalTypeFact[] = [];
  const navigation: NavigationFact[] = [];
  const implementationContracts = new Map<string, ByteRange>();
  let unresolved = 0;
  for (const f of cache.values()) {
    refs.push(...f.refs);
    locals.push(...f.locals);
    for (const fact of f.navigation) navigation.push(fact);
    for (const site of f.implementationContracts) implementationContracts.set(JSON.stringify(site), site);
    unresolved += f.unresolved;
  }
  const messages = [...project.messages];
  if (unresolved > 0) messages.push(`${unresolved} calls could not be resolved`);
  for (const fact of compatibility(project, offsetsOf, messages, navigation)) refs.push(fact);
  return {
    source: project.describe(), refs: shared ? mergeReferences(refs) : refs, locals,
    navigation: shared ? [...new Map(navigation.map((fact) => [JSON.stringify(fact), fact])).values()] : navigation,
    implementation_contracts: [...implementationContracts.values()],
    configuration_files: [...project.configurationFiles].sort(), messages,
  };
}

function mergeReferences(refs: SemanticRef[]): SemanticRef[] {
  const merged = new Map<string, SemanticRef>();
  for (const ref of refs) {
    if (ref.kind !== "call" && ref.kind !== "construct") {
      merged.set(JSON.stringify(ref), ref);
      continue;
    }
    const key = JSON.stringify([ref.kind, ref.site]);
    const previous = merged.get(key);
    if (!previous) merged.set(key, ref);
    else {
      const identity = (target: SemanticTarget) => JSON.stringify(target.decl ?? [target.container, target.display, target.kind]);
      if (identity(previous.target) !== identity(ref.target)) {
        merged.set(key, { ...previous, target: unresolvedCall(previous.target.display) });
      }
    }
  }
  return [...merged.values()];
}

function unresolvedCall(display: string): SemanticTarget {
  return { decl: null, display, container: null, kind: "unresolved", linq: false, external: false };
}

function isAssignmentPattern(node: ts.Node): boolean {
  if (!ts.isObjectLiteralExpression(node) && !ts.isArrayLiteralExpression(node)) return false;
  const parent = node.parent;
  if (ts.isBinaryExpression(parent) && parent.left === node && parent.operatorToken.kind === ts.SyntaxKind.EqualsToken) return true;
  if (ts.isForOfStatement(parent) && parent.initializer === node) return true;
  return isAssignmentPattern(ts.isPropertyAssignment(parent) ? parent.parent : parent);
}

function referenceName(expr: ts.Expression): ts.Node {
  while (ts.isParenthesizedExpression(expr) || ts.isNonNullExpression(expr)) expr = expr.expression;
  if (ts.isPropertyAccessExpression(expr)) return expr.name;
  if (ts.isElementAccessExpression(expr)) return expr.argumentExpression;
  return expr;
}

function propertyReferenceSite(name: ts.Node, sf: ts.SourceFile): ts.Node {
  const access = name.parent;
  // Match the index expression in the Blueprint without giving the variable
  // used as the key the indexed property's symbol.
  if (ts.isElementAccessExpression(access) && access.argumentExpression === name && !isLiteralKey(name)) {
    return access.getChildren(sf).find((child) => child.kind === ts.SyntaxKind.OpenBracketToken) ?? name;
  }
  return name;
}

function literalPropertyKey(node: ts.Node, checker: ts.TypeChecker): string | undefined {
  const type = checker.getTypeAtLocation(node);
  return type.isStringLiteral() || type.isNumberLiteral() ? String(type.value) : undefined;
}

/** A synthetic union signature can pick one declaration even when different members are called. */
function unionCallDeclarations(name: ts.Node, checker: ts.TypeChecker): ts.Declaration[] {
  const access = name.parent;
  if (!ts.isPropertyAccessExpression(access) && !ts.isElementAccessExpression(access)) return [];
  const key = ts.isPropertyAccessExpression(access)
    ? (access.name === name ? access.name.text : undefined)
    : (access.argumentExpression === name ? literalPropertyKey(name, checker) : undefined);
  if (key === undefined) return [];
  const receiver = checker.getNonNullableType(checker.getTypeAtLocation(access.expression));
  if (!receiver.isUnion()) return [];
  const candidates = new Set<ts.Declaration>();
  for (const constituent of receiver.types) {
    const member = checker.getPropertyOfType(constituent, key);
    const declaration = member?.valueDeclaration ?? member?.declarations?.[0];
    if (declaration) candidates.add(implementationDeclaration(declaration, checker));
  }
  return [...candidates];
}

function isNavigationDeclaration(decl: ts.Declaration): boolean {
  return isTypeDeclaration(decl) || ts.isFunctionDeclaration(decl) || ts.isMethodDeclaration(decl) ||
    ts.isMethodSignature(decl) || ts.isConstructorDeclaration(decl) || ts.isPropertyDeclaration(decl) ||
    ts.isPropertySignature(decl) || ts.isGetAccessorDeclaration(decl) || ts.isSetAccessorDeclaration(decl) ||
    ts.isEnumMember(decl) || ts.isArrowFunction(decl) || ts.isFunctionExpression(decl) || isParameterProperty(decl) ||
    (ts.isVariableDeclaration(decl) && !!decl.initializer &&
      (ts.isArrowFunction(decl.initializer) || ts.isFunctionExpression(decl.initializer)));
}

/** The syntax graph represents module and namespace bindings as fields. */
function navigationVariableName(decl: ts.Declaration): ts.Identifier | undefined {
  if ((!ts.isVariableDeclaration(decl) && !ts.isBindingElement(decl)) || !ts.isIdentifier(decl.name)) return;
  let declaration: ts.Node = decl;
  while (ts.isBindingElement(declaration)) declaration = declaration.parent.parent;
  if (!ts.isVariableDeclaration(declaration)) return;
  const statement = declaration.parent.parent;
  if (!ts.isVariableStatement(statement) ||
      (!ts.isSourceFile(statement.parent) && !ts.isModuleBlock(statement.parent))) return;
  if (declaration.initializer && (ts.isArrowFunction(declaration.initializer) || ts.isFunctionExpression(declaration.initializer))) return;
  return decl.name;
}

function isParameterProperty(decl: ts.Declaration): decl is ts.ParameterPropertyDeclaration {
  return ts.isParameter(decl) && ts.isParameterPropertyDeclaration(decl, decl.parent);
}

function isReferenceName(node: ts.Node): boolean {
  if (isLiteralKey(node)) {
    const parent = node.parent;
    return (ts.isElementAccessExpression(parent) && parent.argumentExpression === node) ||
      (ts.isBindingElement(parent) && parent.propertyName === node);
  }
  if (!ts.isIdentifier(node) && !ts.isPrivateIdentifier(node)) return false;
  const parent = node.parent;
  if (ts.isShorthandPropertyAssignment(parent) || ts.isImportSpecifier(parent) || ts.isExportSpecifier(parent)) {
    return (parent as ts.NamedDeclaration).name === node;
  }
  return ts.getNameOfDeclaration(parent as ts.Declaration) !== node || ts.isPropertyAccessExpression(parent);
}

function isLiteralKey(node: ts.Node): node is ts.StringLiteralLike | ts.NumericLiteral {
  return ts.isStringLiteralLike(node) || ts.isNumericLiteral(node);
}

function isObjectContract(type: ts.Type): boolean {
  return !!(type.flags & ts.TypeFlags.Object) ||
    (type.isIntersection() && type.types.every(isObjectContract));
}

function collectMemberImplementations(
  instance: ts.Type,
  base: ts.Type,
  checker: ts.TypeChecker,
  target: (decl: ts.Declaration | undefined, name: string) => SemanticTarget | null,
  navigation: NavigationFact[],
) {
  for (const member of checker.getPropertiesOfType(base)) {
    const candidate = checker.getPropertyOfType(instance, member.name);
    if (!candidate || candidate === member) continue;
    const declaration = candidate.declarations?.find((decl) => ts.isMethodDeclaration(decl) && !!decl.body)
      ?? candidate.valueDeclaration ?? candidate.declarations?.[0];
    if (!declaration || (ts.canHaveModifiers(declaration) &&
      ts.getModifiers(declaration)?.some((m) => m.kind === ts.SyntaxKind.PrivateKeyword))) continue;
    if (!ts.isClassLike(declaration.parent) && !ts.isInterfaceDeclaration(declaration.parent) &&
        !isParameterProperty(declaration)) continue;
    const implementation = target(declaration, candidate.name);
    if (!implementation?.decl) continue;
    for (const decl of member.declarations ?? []) {
      const contract = target(decl, member.name);
      if (contract?.decl) navigation.push({
        site: implementation.decl, target: contract, implementation: true, implementation_kind: implementation.kind,
      });
    }
  }
}

/** Pairs of project classes and interfaces checked at most, so huge projects stay fast. */
const MAX_COMPATIBILITY_PAIRS = 200_000;

/**
 * Structural typing: a class that has every member of a project interface without declaring
 * `implements` still fits it. Reported for interfaces with at least two required members and
 * no type parameters (generic interfaces would need type arguments to compare).
 */
function compatibility(project: ProjectHost, offsetsOf: (sf: ts.SourceFile) => ByteOffset, messages: string[], navigation: NavigationFact[]): SemanticRef[] {
  const byProgram = new Map<ts.Program, { classes: ts.ClassDeclaration[]; interfaces: Map<ts.Symbol, ts.InterfaceDeclaration> }>();
  for (const { file: sf, program } of project.sourceFiles(true)) {
    let group = byProgram.get(program);
    if (!group) {
      group = { classes: [], interfaces: new Map() };
      byProgram.set(program, group);
    }
    const checker = program.getTypeChecker();
    const visit = (n: ts.Node) => {
      if (ts.isClassDeclaration(n) && n.name) group!.classes.push(n);
      if (ts.isInterfaceDeclaration(n) && !n.typeParameters?.length && hasRequiredMember(n)) {
        const sym = checker.getSymbolAtLocation(n.name);
        if (sym && !group!.interfaces.has(sym)) group!.interfaces.set(sym, n);
      }
      if (ts.isModuleDeclaration(n) || ts.isModuleBlock(n) || ts.isSourceFile(n)) ts.forEachChild(n, visit);
    };
    visit(sf);
  }

  const out: SemanticRef[] = [];
  for (const [program, { classes, interfaces }] of byProgram) {
    if (classes.length * interfaces.size > MAX_COMPATIBILITY_PAIRS) {
      messages.push(`structural compatibility skipped: ${classes.length} classes × ${interfaces.size} interfaces`);
      continue;
    }
    const checker = program.getTypeChecker();
    for (const cls of classes) {
      const sym = checker.getSymbolAtLocation(cls.name!);
      if (!sym) continue;
      const instance = checker.getDeclaredTypeOfSymbol(sym);
      const declared = declaredContracts(cls, checker);
      const sf = cls.getSourceFile();
      const file = project.relative(sf.fileName);
      if (!file) continue;
      const o = offsetsOf(sf);
      for (const [ifaceSym, iface] of interfaces) {
        if (declared.has(ifaceSym)) continue;
        let fits = false;
        try {
          fits = checker.isTypeAssignableTo(instance, checker.getDeclaredTypeOfSymbol(ifaceSym));
        } catch {
          fits = false;
        }
        if (!fits) continue;
        collectMemberImplementations(instance, checker.getDeclaredTypeOfSymbol(ifaceSym), checker, (decl, name) =>
          decl ? describe(decl, name, checker, program, project, offsetsOf) : null, navigation);
        out.push({
          site: { file, start: o(cls.name!.getStart(sf)), end: o(cls.name!.getEnd()) },
          kind: "compatible",
          target: { ...describe(iface, iface.name.text, checker, program, project, offsetsOf), kind: "type" },
        });
      }
    }
  }
  return out;
}

/** At least two required members: with fewer, unrelated classes fit by coincidence (`reset()`). */
function hasRequiredMember(iface: ts.InterfaceDeclaration): boolean {
  return iface.members.filter((m) => (ts.isPropertySignature(m) || ts.isMethodSignature(m)) && !m.questionToken).length >= 2;
}

/** Interfaces a class already declares, through `implements`, base classes and interface `extends`. */
function declaredContracts(cls: ts.ClassLikeDeclaration, checker: ts.TypeChecker): Set<ts.Symbol> {
  const out = new Set<ts.Symbol>();
  const seen = new Set<ts.Node>();
  const visit = (decl: ts.Declaration) => {
    if (seen.has(decl)) return;
    seen.add(decl);
    const clauses = (ts.isClassLike(decl) || ts.isInterfaceDeclaration(decl)) ? decl.heritageClauses ?? [] : [];
    const types = [...clauses.flatMap((clause) => clause.types), ...ts.getJSDocImplementsTags(decl).map((tag) => tag.class)];
    for (const t of types) {
      const target = checker.getTypeAtLocation(t).getSymbol() ?? resolveAlias(checker.getSymbolAtLocation(t.expression), checker);
      if (!target) continue;
      out.add(target);
      for (const d of target.declarations ?? []) visit(d);
    }
  };
  visit(cls);
  return out;
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
    let p = decl.parent;
    while (ts.isParenthesizedExpression(p)) p = p.parent;
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
  if (ts.isPropertyDeclaration(decl) && ts.isClassLike(decl.parent) && decl.initializer) {
    let value = decl.initializer;
    while (ts.isParenthesizedExpression(value)) value = value.expression;
    if (ts.isArrowFunction(value) || ts.isFunctionExpression(value)) return "method";
  }
  if (ts.isEnumMember(decl)) return "field";
  if (ts.isGetAccessorDeclaration(decl) || ts.isSetAccessorDeclaration(decl) || ts.isPropertyDeclaration(decl) ||
      ts.isPropertySignature(decl) || isParameterProperty(decl)) {
    return "property";
  }
  return "function";
}

/** The graph represents a TypeScript overload set by the declaration that has a body. */
function implementationDeclaration(decl: ts.Declaration, checker: ts.TypeChecker): ts.Declaration {
  if (ts.isConstructorDeclaration(decl) && !decl.body) {
    return decl.parent.members.find((member) => ts.isConstructorDeclaration(member) && member.body) ?? decl;
  }
  if ((ts.isFunctionDeclaration(decl) || ts.isMethodDeclaration(decl)) && !decl.body && decl.name) {
    const symbol = checker.getSymbolAtLocation(decl.name);
    return symbol?.declarations?.find((candidate) =>
      (ts.isFunctionDeclaration(candidate) || ts.isMethodDeclaration(candidate)) && candidate.body) ?? decl;
  }
  return decl;
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
  const location = named(implementationDeclaration(raw, checker));
  const sf = location.getSourceFile();
  const external = !project.isProjectFile(sf.fileName) || program.isSourceFileDefaultLibrary(sf);
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
    const anchor = ts.getNameOfDeclaration(location) ?? constructorKeyword(location) ?? location;
    if (file) {
      const o = offsetsOf(sf);
      declRange = { file, start: o(anchor.getStart(sf)), end: o(location.getEnd()) };
    }
  }
  const linq = external && program.isSourceFileDefaultLibrary(sf) && container !== null && PIPELINE_CONTAINERS.has(container);
  return { decl: declRange, display: display.length > 120 ? `${display.slice(0, 119)}…` : display, container, kind, linq, external };
}

function constructorKeyword(decl: ts.Declaration): ts.Node | undefined {
  if (!ts.isConstructorDeclaration(decl)) return undefined;
  return decl.getChildren().find((c) => c.kind === ts.SyntaxKind.ConstructorKeyword);
}
