import type { Expr } from "../gen/Expr";
import type { FlowNode } from "../gen/FlowNode";
import type { Language } from "../gen/Language";
import type { SyntaxSite } from "../gen/SyntaxSite";
import type { Symbol } from "../gen/Symbol";
import type { LanguageDetail } from "./bindingDetails";

const spread: Readonly<Record<string, string>> = {
  "spread-arguments": "Spread into separate arguments",
  "spread-items": "Insert items into the new list",
  "spread-properties": "Copy properties into the new object",
};

const rest: Readonly<Record<string, string>> = {
  "rest-arguments": "Collect remaining arguments",
  "rest-items": "Collect remaining items",
  "rest-properties": "Collect remaining properties",
};
const argumentMode: Readonly<Record<string, string>> = {
  "argument:in": "Pass by read-only reference",
  "argument:ref": "The call can change this variable",
  "argument:out": "The call assigns this variable on normal return",
};

export function argumentDetails(expr: Expr): LanguageDetail[] {
  return expr.flavors.flatMap((flavor) => Object.hasOwn(argumentMode, flavor) ? [{
    label: flavor.slice("argument:".length),
    explain: argumentMode[flavor],
  }] : []);
}

export interface ExpressionPinDetails {
  sites?: SyntaxSite[];
  path: string;
  details: LanguageDetail[];
  flavors: string[];
}

export function resolvePinDetails(pin: ExpressionPinDetails, resolveFlavor: (flavor: string) => LanguageDetail | undefined): LanguageDetail[] {
  return [...pin.details, ...pin.flavors.flatMap((flavor) => resolveFlavor(flavor) ?? [])];
}

const sequenceSteps: Readonly<Record<string, string>> = {
  filter: "Keep items that pass the test",
  Where: "Keep items that pass the test",
  map: "Transform each item",
  Select: "Transform each item",
  flatMap: "Combine the items produced by each input",
  flat_map: "Combine the items produced by each input",
  SelectMany: "Combine the items produced by each input",
  filter_map: "Keep the new value from Some and skip None",
  reduce: "Update the accumulator for each item",
  reduceRight: "Update the accumulator from right to left",
  Aggregate: "Update the accumulator for each item",
  fold: "Update the accumulator for each item",
  find: "Stop at the first match",
  First: "Stop at the first match",
  FirstOrDefault: "Stop at the first match or use the default",
  some: "Stop with yes at the first match",
  any: "Stop with yes at the first match",
  Any: "Stop with yes at the first match",
  every: "Stop with no at the first failure",
  all: "Stop with no at the first failure",
  All: "Stop with no at the first failure",
  collect: "Consume the sequence and collect its results",
  ToList: "Consume the sequence and build a list",
  ToArray: "Consume the sequence and build an array",
  sum: "Consume the sequence and add its values",
  Sum: "Consume the sequence and add its values",
};
const deferred = new Set(["filter", "map", "Where", "Select", "flatMap", "flat_map", "SelectMany", "filter_map"]);
const promiseActions: Readonly<Record<string, string>> = {
  then: "Continue when the previous promise settles",
  catch: "Handle a rejection from the previous step",
  finally: "Run cleanup after success or failure",
};

interface SequenceOperation {
  family: string;
  method: string;
  action: string;
  timing: string;
}

function receiverOffset(expr: Expr): number {
  return expr.children[0]?.flavors.includes("target") ? 1 : 0;
}

function sequenceOperation(expr: Expr): SequenceOperation | undefined {
  const method = expr.label.match(/^\w+/)?.[0];
  if (expr.symbol || !method) return;
  const iteratorSource = rustIteratorExplanation(expr);
  if (iteratorSource) {
    return { family: "sequence:iterator", method, action: iteratorSource, timing: "Prepares an iterator" };
  }
  const family = expr.flavors.find((f) => f.startsWith("sequence:"));
  if (!family) return;
  if (family === "sequence:promise") {
    if (!Object.hasOwn(promiseActions, method)) return;
    const action = method === "then" && expr.children.length - receiverOffset(expr) < 2
      ? "Continue when the previous promise succeeds" : promiseActions[method];
    return { family, method, action, timing: "Runs after the previous promise settles" };
  }
  if (!Object.hasOwn(sequenceSteps, method)) return;
  const firstArgument = expr.children[receiverOffset(expr)];
  if (family === "sequence:array" && method === "map" && firstArgument?.concept === "Lambda" &&
      firstArgument.flavors.includes("async")) {
    return { family, method, action: "Produce a promise for each item", timing: "Starts each callback when called" };
  }
  const timing = family === "sequence:array" ? "Runs when called"
    : deferred.has(method) ? "Runs when the sequence is consumed" : "Consumes the sequence";
  return { family, method, action: sequenceSteps[method], timing };
}

export interface SequencePreview {
  source: string;
  stages: { action: string; timing?: string }[];
  example?: string[];
}

function sequenceExample(stages: readonly Expr[]): string[] | undefined {
  const operations = (stages[0] && rustIteratorExplanation(stages[0]) ? stages.slice(1) : stages).map(sequenceOperation);
  if (operations.length !== 3) return;
  const [filter, map, total] = operations;
  if (!filter || !map || !total || !["filter", "Where"].includes(filter.method) || !["map", "Select"].includes(map.method) ||
      !["sum", "Sum", "reduce", "Aggregate", "fold"].includes(total.method)) return;
  const eager = filter.family === "sequence:array" && map.family === filter.family;
  if (eager) return ["Keep evens: [1, 2, 3, 4] → [2, 4]", "Square: [2, 4] → [4, 16]", "Add: 0 → 4 → 20"];
  if (filter.family !== map.family || filter.family !== total.family || !["sequence:linq", "sequence:iterator"].includes(filter.family)) return;
  return ["Keep evens and square one item at a time", "1 and 3 are skipped · 2 → 4 · 4 → 16", "Running total: 0 → 4 → 20"];
}

function rustIteratorExplanation(expr: Expr): string | undefined {
  if (expr.flavors.includes("rust-iter:borrow")) return "Iterate over shared references";
  if (expr.flavors.includes("rust-iter:borrow-mut")) return "Iterate over references that allow changes";
  if (expr.flavors.includes("rust-iter:by-value")) return "Take the iterator source by value";
}

function asyncArrayPreview(all: Expr): SequencePreview | undefined {
  if (!all.flavors.includes("promise:all")) return;
  const map = all.children[receiverOffset(all)];
  if (!map || map.children[0]?.flavors.includes("target") !== true) return;
  const mapping = sequenceOperation(map);
  if (mapping?.family !== "sequence:array" || mapping.method !== "map" ||
      !map.children[1]?.flavors.includes("async")) return;
  return {
    source: map.children[0].label,
    stages: [
      { action: mapping.action, timing: mapping.timing },
      { action: "Wait for all results", timing: "Rejects when an input rejects" },
    ],
  };
}

export function sequencePreview(root: Expr | null): SequencePreview | undefined {
  const pending = root ? [root] : [];
  while (pending.length) {
    const expr = pending.pop()!;
    const source = expr.concept === "Pipeline" ? expr.children[0] : expr;
    const asyncArray = source && asyncArrayPreview(source);
    if (asyncArray) {
      for (const stage of expr.concept === "Pipeline" ? expr.children.slice(1) : []) {
        const operation = sequenceOperation(stage);
        asyncArray.stages.push(operation ? { action: operation.action, timing: operation.timing }
          : { action: stage.label, timing: "Timing not determined" });
      }
      return asyncArray;
    }
    if (expr.concept === "Pipeline" && expr.children.slice(1).some(sequenceOperation)) {
      return {
        source: expr.children[0].label,
        stages: expr.children.slice(1).map((stage) => {
          const operation = sequenceOperation(stage);
          return operation ? { action: operation.action, timing: operation.timing }
            : { action: stage.label, timing: "Timing not determined" };
        }),
        example: sequenceExample(expr.children.slice(1)),
      };
    }
    if (expr.concept !== "Lambda") {
      for (let i = expr.children.length - 1; i >= 0; i--) pending.push(expr.children[i]);
    }
  }
}

export function sequenceArgumentRoles(expr: Expr): string[] {
  const operation = sequenceOperation(expr);
  if (!operation || expr.children.some((child) => child.flavors.includes("spread"))) return [];
  const { method } = operation;
  const offset = receiverOffset(expr);
  const prefix = offset ? ["Source"] : [];
  if (method === "fold" || (method === "Aggregate" && expr.children.length - offset > 1)) {
    return [...prefix, "Initial value", "Update total", "Select result"];
  }
  if (method === "reduce" || method === "reduceRight" || method === "Aggregate") return [...prefix, "Update total", "Initial value"];
  if (deferred.has(method) || ["find", "some", "any", "every", "all", "All"].includes(method)) {
    return [...prefix, "Function for each item"];
  }
  if (["First", "FirstOrDefault", "Any"].includes(method) && expr.children[offset]?.concept === "Lambda") {
    return [...prefix, "Function for each item"];
  }
  return [];
}

export interface CallbackRoles {
  params: string[];
  result?: string;
}

export function sequenceCallbackRoles(stage: Expr, argument: number): CallbackRoles | undefined {
  const role = sequenceArgumentRoles(stage)[argument];
  const operation = sequenceOperation(stage);
  const array = operation?.family === "sequence:array";
  if (role === "Update total") return {
    params: array ? ["Current total", "Item", "Index", "Source"] : ["Current total", "Item"],
    result: "Updated total for the next item",
  };
  if (role === "Function for each item") {
    return { params: array ? ["Item", "Index", "Source"] : ["Item"] };
  }
}

function sequenceExplanation(expr: Expr): string[] {
  const operation = sequenceOperation(expr);
  if (!operation) return [];
  const { family, method, action, timing } = operation;
  const roles = sequenceArgumentRoles(expr);
  if (roles.includes("Update total")) {
    const seed = roles.indexOf("Initial value");
    if (expr.children[seed]) {
      return [action, roles.includes("Select result") && expr.children.length - receiverOffset(expr) > 2
        ? "Empty sequence: select the result from the initial value" : "Empty sequence: keep the initial value"];
    }
    if (method === "reduce" && family === "sequence:iterator") return [action, "Start at the first item · empty returns None"];
    if (expr.children.length - receiverOffset(expr) === 1 && (family === "sequence:array" || method === "Aggregate")) {
      return [action, "Start at the first item · empty raises an error"];
    }
  }
  if (method === "find") return [action, family === "sequence:array" ? "No match: undefined" : "No match: None"];
  if (method === "First") return [action, "No match: raise an error"];
  if (method === "FirstOrDefault") return [action, "No match: use the default value"];
  if (["some", "any", "Any"].includes(method)) return [action, "No match: return false"];
  if (["every", "all", "All"].includes(method)) return [action, "No failures: return true"];
  return [action, timing];
}

export function isExplanationFlavor(flavor: string): boolean {
  return flavor.startsWith("sequence:") || flavor.startsWith("match-binding:") || flavor.startsWith("fallthrough-out:") ||
    /^(binding-default|parameter-default|catch-filter|match-guard):/.test(flavor) ||
    flavor.startsWith("option:") || flavor.startsWith("rust-iter:") || flavor === "promise:all" ||
    Object.hasOwn(spread, flavor) || Object.hasOwn(argumentMode, flavor) ||
    Object.hasOwn(rest, flavor.split(":", 1)[0]);
}

const detailFlavors = new Set([
  "const", "readonly", "required", "init", "accessor:init", "unsafe", "lifetime", "move",
  "private", "runtime-private", "type-only", "type-import", "type-assertion", "as-const", "satisfies", "non-null",
  "runtime-freeze", "falsy-fallback", "typeof", "instanceof", "in",
  "checked", "unchecked", "yield-item", "dynamic", "boxing", "value-copy", "reference-copy", "copy-with",
  "safe-access", "fallback", "dispose", "dispose-at-end", "try", "let-else", "if-let", "while-let", "implicit-return",
  ...Object.keys(spread),
]);

/** A file edit can invalidate compiler facts before the refreshed graph arrives. */
export function withoutExpressionFacts(expr: Expr): Expr {
  const flavors = expr.flavors.filter((flavor) => !flavor.startsWith("narrowed:") &&
    !flavor.startsWith("sequence:") && !flavor.startsWith("rust-iter:") && !flavor.startsWith("option:") &&
    !["boxing", "value-copy", "reference-copy", "dynamic", "runtime-freeze", "promise:all"].includes(flavor));
  const children = expr.children.map(withoutExpressionFacts);
  return {
    ...expr,
    value_info: expr.value_info?.basis === "TypeAnalysis" ? undefined : expr.value_info,
    type_info: undefined,
    param_pins: expr.param_pins?.map((pin) => ({ ...pin, type_info: undefined })),
    syntax_sites: expr.syntax_sites?.filter((site) => !site.key.startsWith("narrowed:") &&
      !["boxing", "value-copy", "reference-copy", "dynamic", "runtime-freeze"].includes(site.key)),
    flavors,
    children,
    ty: expr.flavors.some((flavor) => flavor.startsWith("narrowed:")) ? null : expr.ty,
  };
}

export function languageDetailFlavors(flavors: readonly string[]): string[] {
  return flavors.filter((flavor) => (
    detailFlavors.has(flavor) || flavor.startsWith("#cfg(") || flavor.startsWith("narrowed:")
  ) && !(flavor === "private" && flavors.includes("runtime-private")))
    .map((flavor) => flavor === "accessor:init" ? "init" : flavor)
    .filter((flavor, index, values) => values.indexOf(flavor) === index);
}

export function declarationDetailFlavors(symbol: Symbol): string[] {
  const hasReadonlySite = symbol.language === "csharp" && symbol.syntax_sites?.some((site) => site.key.startsWith("readonly-"));
  return languageDetailFlavors(symbol.flavors).filter((flavor) => flavor !== "readonly" || !hasReadonlySite);
}

export function expressionDetailFlavors(expr: Expr, language: Language): string[] {
  const syntax = language === "typescript"
    ? expr.concept === "Fallback" ? ["fallback"] : expr.concept === "SafeAccess" ? ["safe-access"] : []
    : [];
  return languageDetailFlavors([...expr.flavors, ...syntax]);
}

/** Collapsed nodes own the explanations of the expressions still displayed on them. */
export function statementDetailFlavors(node: FlowNode, language: Language, expanded: boolean, inlineRoot = false): string[] {
  const declarations = node.outputs.some((pin) => pin.binding);
  const result = new Set(languageDetailFlavors(node.flavors).filter((f) => !(declarations && f === "const")));
  if (node.expr && inlineRoot) {
    for (const flavor of expressionDetailFlavors(node.expr, language)) result.add(flavor);
  } else if (!expanded && node.expr) {
    const pending = [node.expr];
    while (pending.length) {
      const expr = pending.pop()!;
      for (const flavor of expressionDetailFlavors(expr, language)) result.add(flavor);
      if (expr.concept !== "Lambda") pending.push(...[...expr.children].reverse());
    }
  }
  return [...result];
}

export function pinSyntaxDetails(node: FlowNode, id: string): LanguageDetail[] {
  const details: LanguageDetail[] = [];
  const rest = restPinExplanation(node, id);
  if (rest) details.push({ label: "Gather rest", explain: rest });
  if (id.startsWith("var:") && node.flavors.some((f) => f === `binding-default:${id.slice(4)}` || f === `parameter-default:${id.slice(4)}`)) {
    details.push({ label: "Default if undefined", explain: "The default expression runs if the incoming value is undefined, including when no argument or property was supplied. A null value passes through." });
  }
  if (id.startsWith("catch:") && node.flavors.includes(`catch-filter:${id.slice(6)}`)) {
    details.push({ label: "Exception filter", explain: "This handler runs only if the exception matches and the when condition is true. Otherwise the search continues." });
  }
  if (id.startsWith("case:") && node.flavors.includes(`match-guard:${id.slice(5)}`)) {
    details.push({ label: "Match guard", explain: "Check the guard after the pattern matches. If the guard fails, continue trying the following arms." });
  }
  return details;
}

export function executionPinLabel(node: FlowNode, id: string, original: string): string {
  if (node.flavors.includes(`fallthrough-out:${id}`)) return "Continue into the next case";
  if (node.flavors.includes("let-else")) {
    if (id === "out") return "Pattern matched · continue with the bindings";
    if (id === "else") return "Pattern did not match · run the else block";
  }
  if (id === "true" && node.flavors.includes("if-let")) return "Matched · bindings available";
  if (id === "false" && node.flavors.includes("if-let")) return "Pattern did not match";
  if (id === "body" && node.flavors.includes("while-let")) return "Matched · bindings available";
  if (id === "out" && !original && node.concept === "Await") return "Resume after the wait";
  if (id === "out" && !original && node.flavors.includes("try") && node.outputs.some((p) => p.id === "error")) {
    return "Continue with the value";
  }
  if (!id.startsWith("case:")) return original;
  const prefix = `match-binding:${id.slice(5)}:`;
  const bindings = node.flavors.filter((f) => f.startsWith(prefix)).map((f) => f.slice(prefix.length));
  return bindings.length ? `${original} · Extract ${bindings.join(", ")}` : original;
}

export function restPinExplanation(node: FlowNode, id: string): string | undefined {
  if (!id.startsWith("var:")) return undefined;
  const name = id.slice(4);
  const kind = Object.keys(rest).find((kind) => node.flavors.includes(`${kind}:${name}`));
  return kind ? rest[kind] : undefined;
}

export function spreadOutput(expr: Expr): string | undefined {
  if (expr.flavors.includes("spread-arguments")) return "Separate arguments";
  if (expr.flavors.includes("spread-items")) return "Items for the new list";
  if (expr.flavors.includes("spread-properties")) return "Properties for the new object";
}

export function expressionExplanations(expr: Expr, language: Language): string[] {
  const labels = sequenceExplanation(expr);
  if (language === "csharp") {
    for (const f of expr.flavors) if (Object.hasOwn(argumentMode, f)) labels.push(argumentMode[f]);
  }
  if (language === "rust" && (expr.flavors.includes("option:map") || expr.flavors.includes("option:and_then"))) {
    labels.push(expr.flavors.includes("option:map") ? "Has value: transform it and wrap in Some" : "Has value: use the Option returned by the function");
    labels.push("Absent: keep None");
  }
  if (language === "rust" && expr.concept === "Lambda" && expr.flavors.includes("move")) {
    labels.push("Capture values from the enclosing scope by value");
  }
  if (language !== "typescript") return labels;
  if (expr.flavors.includes("promise:all")) labels.push("Wait for all results", "Reject when an input rejects");
  return labels;
}

export function statementExplanations(node: FlowNode, language: Language): string[] {
  const labels = new Set<string>();
  if (node.concept === "EventEntry" && node.label && node.label !== node.code) labels.add(node.label);
  if (node.flavors.some((f) => f === "dispose" || f === "dispose-at-end")) {
    labels.add(node.flavors.includes("async") ? "Await cleanup when leaving scope" : "Release the resource when leaving scope");
    labels.add("Cleanup also runs on early return");
  }
  if (node.concept === "Await") labels.add("Continue here after the wait completes");
  if (language === "csharp" && node.concept === "CriticalSection") {
    labels.add("Acquire the lock before entering");
    labels.add("Release the lock when leaving");
  }
  if (node.concept === "Subscribe") labels.add(node.flavors.includes("unsubscribe")
    ? "Stop calling this handler for the event" : "Register the handler to run when the event occurs");
  if (node.concept === "Emit") labels.add("Notify the handlers registered for this event");
  if (language === "rust" && node.flavors.includes("implicit-return")) labels.add("The last expression is the returned value");
  if (language === "rust" && node.flavors.includes("let-else")) labels.add("Continue with the bindings or take the else path");
  if (language === "rust" && node.outputs.some((p) => p.id === "error") && node.flavors.includes("try")) {
    labels.add("Continue with the value or return early");
  }
  if (language === "rust" && node.flavors.includes("implicit-drop")) labels.add("Automatic cleanup at scope exit");
  if (node.concept === "Call" && node.expr?.concept === "Call" && !node.expr.target && !node.expr.symbol &&
      !node.expr.op && !node.calls.some((call) => call.symbol)) labels.add("Call destination not resolved");
  const pending = node.expr ? [node.expr] : [];
  while (pending.length && labels.size < 2) {
    const expr = pending.pop()!;
    if (expr.concept !== "Pipeline") {
      for (const label of expressionExplanations(expr, language)) labels.add(label);
    } else {
      // The ordered sequence preview owns the stage descriptions on the statement.
      continue;
    }
    // The body of a function value runs when called. Its effects belong to that body.
    if (expr.concept !== "Lambda") {
      for (let i = expr.children.length - 1; i >= 0; i--) pending.push(expr.children[i]);
    }
  }
  return [...labels].slice(0, 2);
}
