import ts from "typescript";

type TypeShape =
  | { kind: "Unknown" }
  | { kind: "Scalar"; name: string }
  | { kind: "Named"; name: string; arguments: TypeShape[] }
  | { kind: "Collection"; family: "Array" | "Dictionary" | "Set"; arguments: TypeShape[] }
  | { kind: "Tuple" | "Union"; items: TypeShape[] };

export function blueprintShape(type: ts.Type, checker: ts.TypeChecker, program: ts.Program, depth = 0): TypeShape {
  if (depth >= 8 || type.flags & (ts.TypeFlags.Any | ts.TypeFlags.Unknown)) return { kind: "Unknown" };
  const flags = type.flags;
  const symbol = type.getSymbol();
  const enumMember = symbol?.declarations?.find(ts.isEnumMember);
  if (enumMember) return { kind: "Named", name: enumMember.parent.name.text, arguments: [] };
  if (symbol && symbol.flags & ts.SymbolFlags.Enum) {
    return { kind: "Named", name: symbol.getName(), arguments: [] };
  }
  if (flags & ts.TypeFlags.BooleanLike) return { kind: "Scalar", name: "Bool" };
  if (flags & ts.TypeFlags.StringLike) return { kind: "Scalar", name: "String" };
  if (flags & ts.TypeFlags.NumberLike) return { kind: "Scalar", name: "Number" };
  if (flags & ts.TypeFlags.BigIntLike) return { kind: "Scalar", name: "BigInt" };
  if (flags & ts.TypeFlags.ESSymbolLike) return { kind: "Scalar", name: "Symbol" };
  if (flags & ts.TypeFlags.Null) return { kind: "Scalar", name: "Null" };
  if (flags & ts.TypeFlags.Undefined) return { kind: "Scalar", name: "Undefined" };
  if (flags & ts.TypeFlags.Never) return { kind: "Scalar", name: "Never" };
  if (flags & ts.TypeFlags.Void) return { kind: "Scalar", name: "Unit" };
  const inner = (arg: ts.Type) => blueprintShape(arg, checker, program, depth + 1);
  if (type.isUnion()) {
    if (type.types.length > 16) return { kind: "Unknown" };
    const items = [...new Map(type.types.map(inner).map((item) => [JSON.stringify(item), item])).values()];
    if (items.length > 8 || items.some((item) => item.kind === "Unknown")) return { kind: "Unknown" };
    return items.length === 1 ? items[0] : { kind: "Union", items };
  }
  const reference = (flags & ts.TypeFlags.Object) && ((type as ts.ObjectType).objectFlags & ts.ObjectFlags.Reference)
    ? type as ts.TypeReference : undefined;
  const args = reference ? checker.getTypeArguments(reference).slice(0, 16).map(inner) : [];
  if (checker.isTupleType(type)) return { kind: "Tuple", items: args };
  if (checker.isArrayType(type)) return { kind: "Collection", family: "Array", arguments: args };
  const name = symbol?.getName();
  const declarations = symbol?.declarations;
  if (declarations?.length && declarations.every((decl) => program.isSourceFileDefaultLibrary(decl.getSourceFile()))) {
    if (name === "Map" || name === "ReadonlyMap") return { kind: "Collection", family: "Dictionary", arguments: args };
    if (name === "Set" || name === "ReadonlySet") return { kind: "Collection", family: "Set", arguments: args };
  }
  if (type.aliasSymbol) return { kind: "Named", name: type.aliasSymbol.getName(), arguments: (type.aliasTypeArguments ?? []).slice(0, 16).map(inner) };
  if (name && !name.startsWith("__")) return { kind: "Named", name, arguments: args };
  if (checker.getSignaturesOfType(type, ts.SignatureKind.Call).length) return { kind: "Scalar", name: "Function" };
  if (flags & (ts.TypeFlags.Object | ts.TypeFlags.NonPrimitive)) return { kind: "Scalar", name: "Object" };
  return { kind: "Unknown" };
}

export function writtenType(node: ts.Node): boolean {
  if (ts.getJSDocType(node)) return true;
  if (ts.isBindingElement(node)) {
    const parent = node.parent.parent;
    return ts.isVariableDeclaration(parent) || ts.isParameter(parent) || ts.isBindingElement(parent) ? writtenType(parent) : false;
  }
  return (
    ts.isVariableDeclaration(node) || ts.isParameter(node) || ts.isPropertyDeclaration(node) ||
    ts.isPropertySignature(node) || ts.isFunctionDeclaration(node) || ts.isMethodDeclaration(node) || ts.isGetAccessorDeclaration(node)
  ) && !!node.type;
}
