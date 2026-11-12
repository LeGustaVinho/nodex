import type { ValueInfo } from "../gen/ValueInfo";
import type { TypeInfo } from "../gen/TypeInfo";
import { expressionSites, statementSites } from "./syntaxSites";
import type { Expr } from "../gen/Expr";
import type { Access } from "../gen/Access";
import type { FlowGraph } from "../gen/FlowGraph";
import type { FlowNode } from "../gen/FlowNode";
import type { Focus } from "../gen/Focus";
import type { Span } from "../gen/Span";
import type { StepInfo } from "../gen/StepInfo";
import type { Language } from "../gen/Language";
import type { ReadingSite } from "../gen/ReadingSite";
import type { OperationInfo } from "../gen/OperationInfo";
import type { StructureIndex } from "./structureGraph";
import { type BlueprintSource, type CodePreview, type CodeRange, codePreview, codePreviewHeight, sourceCode, uniqueCodeRange } from "./blueprintSource";
import type { GEdge, GGraph, GNode, GPort } from "./model";
import { argumentDetails, expressionDetailFlavors, expressionExplanations, isExplanationFlavor, rustChoicePresentation, sequenceArgumentRoles, sequenceCallbackRoles, sequencePreview, spreadOutput, statementExplanations, statementDetailFlavors, withoutExpressionFacts, type CallbackRoles, type ExpressionPinDetails, type SequencePreview } from "./blueprintExplanation";
import { compactReading, executionReading, readingHeight, readingsByNode } from "./blueprintReading";
import type { ContractItem } from "../gen/ContractItem";
import { displayedInputAccess, hasCaptureExpression, rustOwnershipHeight, rustOwnershipRows, type RustOwnershipRow } from "./rustOwnership";

export const BLUEPRINT_WIDTH = 300;

function flowEdgeId(edge: FlowGraph["edges"][number]) {
  return `${edge.from}:${edge.from_pin}->${edge.to}:${edge.to_pin}`;
}

/** Only a unique execution edge can explain an observed pair of mapped steps. */
export function recordedEdgeId(flow: FlowGraph | undefined, step: StepInfo | undefined, prefix = "") {
  if (!flow || !step?.previous_node || !step.node) return;
  const from = flow.nodes.find((node) => node.id === step.previous_node);
  const edges = flow.edges.filter((edge) => edge.kind === "Exec" && edge.from === from?.id && edge.to === step.node);
  if (edges.length !== 1 || from?.outputs.find((pin) => pin.id === edges[0].from_pin)?.label === "Then") return;
  return prefix + flowEdgeId(edges[0]);
}
export const EXPR_WIDTH = 190;
const READING_EXPR_WIDTH = 270;
export const PIN_ROW = 22;
const EXPR_PIN_ROW = 18;
const MAX_EXPR_NODES = 80;

/**
 * How much of each statement is drawn:
 * - `statements`: one node per statement; expression trees open on demand (ADR-007);
 * - `operations`: every statement's operations as Blueprint nodes, constants inside the pins and
 *   local variables wired from where they are set;
 * - `everything`: operations, plus the bodies of inline functions (lambdas) with their parameters.
 */
export type DetailLevel = "statements" | "operations" | "everything";

export const DETAIL_LEVELS: { id: DetailLevel; label: string; explain: string }[] = [
  { id: "statements", label: "Statements", explain: "One node per statement; open a statement's details with its + button." },
  { id: "operations", label: "Operations", explain: "Every operation (add, compare, upper case, maximum...) as its own node, with values in the pins." },
  { id: "everything", label: "Everything", explain: "Operations, plus the bodies of the small functions written inline (lambdas)." },
];

/** One input pin of an expression node (or of a statement in the compact levels). */
export interface ExprPin {
  id: string;
  /** Pin name (`A`, `Text`, `Separator`); null when the operation has no names for it. */
  name: string | null;
  ty: string | null;
  /** A constant or a name shown inside the pin, instead of a wire. */
  value: string | null;
  emphasis?: CodeRange;
  languageDetails?: ExpressionPinDetails;
  valueInfo?: ValueInfo;
  typeInfo?: TypeInfo;
  path?: string;
}

export interface BlueprintData {
  node: FlowNode;
  code?: CodePreview;
  sourceRevision?: string | null;
  flow: string;
  language: Language;
  expanded: boolean;
  explanations?: string[];
  sequence?: SequencePreview;
  reading?: ReadingSite[];
  readingExec?: Record<string, string>;
  contract?: ContractItem[];
  ownership?: RustOwnershipRow[];
  inputAccess?: Record<string, Access | undefined>;
  /** Compact levels: the statement's own call/assignment arguments, drawn as its input pins. */
  argPins?: ExprPin[];
  /** Inputs wired straight into the statement's expression tree instead (compact levels). */
  hiddenInputs?: string[];
  /** Whether the statement has details to open or close (its + / − button). */
  canExpand?: boolean;
  detailsLabel?: string;
  resultInfo?: ValueInfo;
}

export interface ExprData {
  expr: Expr;
  ownerId: string;
  path: string;
  flow: string;
  language: Language;
  code?: CodePreview;
  pins: ExprPin[];
  /** Everything level: the lambda's parameters, drawn as output pins. */
  params: string[];
  paramRoles?: string[];
  explanations?: string[];
  reading?: ReadingSite[];
  sourceRevision?: string;
  width?: number;
}

export interface FlowGraphOptions {
  level?: DetailLevel;
  structure?: StructureIndex;
  explanations?: boolean;
  source?: BlueprintSource;
  /** Catalog operations, for pin names. */
  operations?: Record<string, OperationInfo>;
}

export const expansionKey = (flow: string, nodeId: string) => `${flow}|${nodeId}`;

/**
 * Statements that *are* their expression (a call statement, an assignment): in the compact levels
 * the expression's inputs become the statement's own pins. A `for each` over `repo.all()` is not
 * the call, so its expression stays a tree feeding the details pin.
 */
function absorbs(node: FlowNode): boolean {
  const root = node.expr?.concept;
  if (!root) return false;
  if (root === node.concept) return ["Call", "Assign", "Construct", "Await", "Throw", "Emit"].includes(root);
  return (root === "Call" && ["Emit", "Spawn"].includes(node.concept)) || (root === "Assign" && node.concept === "SetMember");
}

/** Pin names for concepts that are not catalog operations. */
const CONCEPT_PINS: Record<string, string[]> = {
  Choose: ["If", "Then", "Else"],
  Index: ["Of", "At"],
  Fallback: ["Value", "Otherwise"],
  Assign: ["Target", "Value"],
  TypeCheck: ["Value"],
  Convert: ["Value"],
  Await: ["Task"],
  Throw: ["Error"],
};

function expressionSpan(expr: Expr): Span | undefined {
  if (expr.flavors.includes("captured")) return;
  const start = expr.focus?.primary_bytes?.[0];
  // Pipeline stage spans also include all earlier calls. Only this call belongs to the stage.
  if (expr.flavors.includes("stage") && start !== undefined && start >= expr.span.start_byte && start < expr.span.end_byte) {
    return { ...expr.span, start_byte: start };
  }
  if (!expr.flavors.includes("stage")) return expr.span;
}

function visibleCode(preview: CodePreview | undefined) {
  // A row made up of slots and their delimiters adds nothing to the connected pins.
  return preview && (!preview.text.includes("□") || /[^□\s()[\]{},;.]/u.test(preview.text)) ? preview : undefined;
}

function coversTarget(focus: Focus, file: string, covered: readonly Span[]) {
  const range = focus.primary_bytes;
  return !!range && covered.some((part) => part.file === file && part.start_byte <= range[0] && part.end_byte >= range[1]);
}

function focusPreview(focus: Focus, qualified: boolean): CodePreview | undefined {
  const prefix = qualified ? focus.prefix ?? "" : "";
  return codePreview(prefix + focus.primary, [prefix.length, prefix.length + focus.primary.length]);
}

function expressionCode(expr: Expr, source: Uint8Array | undefined, covered: readonly Span[] = []): CodePreview | undefined {
  const span = expressionSpan(expr);
  const written = span && sourceCode(source, span, expr.focus, covered);
  if (written) return visibleCode(written);
  const focus = expr.focus;
  if (focus) {
    const range = focus.primary_bytes;
    if (!coversTarget(focus, expr.span.file, covered) &&
        (covered.length === 0 || range || ["Call", "Construct", "GetMember", "Convert", "TypeCheck"].includes(expr.concept))) {
      return focusPreview(focus, covered.length === 0 && !expr.flavors.includes("stage"));
    }
  }
  if (covered.length === 0 || /^[^\p{ID_Continue}\s]+$/u.test(expr.label) ||
      (["Call", "Construct", "GetMember"].includes(expr.concept) && /^[\p{ID_Continue}.$:]+$/u.test(expr.label))) {
    return codePreview(expr.label);
  }
}

function statementCode(node: FlowNode, source: Uint8Array | undefined, expanded: boolean, covered: readonly Span[], absorb: boolean) {
  if (node.flavors.includes("post-check") && node.expr) {
    if (expanded) return;
    const condition = sourceCode(source, node.expr.span, node.focus);
    if (condition) {
      const prefix = "do □ while (";
      return codePreview(`${prefix}${condition.text})`, condition.emphasis &&
        [prefix.length + condition.emphasis[0], prefix.length + condition.emphasis[1]]);
    }
  }
  const written = sourceCode(source, node.span, node.focus, covered, node.code);
  if (!expanded) {
    const text = node.code || node.label;
    return written ?? codePreview(text, uniqueCodeRange(text, node.focus?.primary));
  }
  if (written) {
    if ((node.concept === "Branch" && /^if\s*(?:\(\s*□\s*\)|□)$/u.test(written.text)) ||
        (node.concept === "Return" && /^(?:(?:return\b|=>)\s*)?□\s*;?$/u.test(written.text))) return;
    return visibleCode(written);
  }
  if (absorb && node.expr) return expressionCode({ ...node.expr, focus: node.focus ?? node.expr.focus }, source, covered);
  // A synthetic or old header has no reliable byte offsets to remove the expression from.
  if (node.focus?.primary_bytes && !coversTarget(node.focus, node.span.file, covered)) {
    return focusPreview(node.focus, false);
  }
}

/** Converts a flow graph to the neutral model. Expanded statements show their expression trees. */
export function flowToGraph(flow: FlowGraph, expanded: Record<string, boolean>, showDataWires: boolean, opts: FlowGraphOptions = {}): GGraph {
  const level = opts.level ?? "statements";
  const compact = level !== "statements";
  const source = opts.source?.revision && opts.source.revision === flow.source_revision ? opts.source.bytes : undefined;
  const nodes: GNode[] = [];
  const edges: GEdge[] = [];
  /** `nodeId use:x` data edges replaced by wires into the node's expression tree. */
  const redirected = new Set<string>();
  const captures = new Set<string>();
  const accessByNode = new Map<string, Record<string, Access | undefined>>();
  for (const edge of flow.edges) {
    if (edge.kind !== "Data" || !edge.access) continue;
    let access = accessByNode.get(edge.to);
    if (!access) accessByNode.set(edge.to, access = {});
    access[edge.to_pin] = !Object.hasOwn(access, edge.to_pin) || access[edge.to_pin] === edge.access ? edge.access : undefined;
  }

  const stale = !!opts.source && !!flow.source_revision && !source;
  const readingNodes = readingsByNode(flow, opts.explanations !== false, stale);
  const ownershipNodes = rustOwnershipRows(flow, opts.explanations !== false && !stale);
  for (const original of flow.nodes) {
    const syntaxPins = (pins: FlowNode["inputs"]) => pins.map((pin) => ({
      ...pin, type_info: undefined, value_info: pin.value_info?.basis === "TypeAnalysis" ? undefined : pin.value_info,
    }));
    const node = stale ? {
      ...original, expr: original.expr && withoutExpressionFacts(original.expr),
      inputs: syntaxPins(original.inputs), outputs: syntaxPins(original.outputs),
    } : original;
    const hasCapture = flow.language === "rust" && hasCaptureExpression(node.expr);
    if (hasCapture) captures.add(node.id);
    const reading = readingNodes.get(node.id) ?? [];
    const contract = !stale && opts.explanations !== false && node.concept === "Entry"
      ? opts.structure?.byId.get(flow.symbol)?.rust_contract?.filter((item) => item.span.file === flow.span.file).slice(0, 5)
      : undefined;
    const ownership = ownershipNodes.get(node.id) ?? [];
    const routed = executionReading(node, reading);
    const cards = routed ? reading.filter((site) => site !== routed.site) : reading;
    const explanations = opts.explanations === false ? [] : statementExplanations(node, flow.language, reading.length > 0);
    const event = flow.language === "csharp" && node.concept === "Emit" ? node.expr?.symbol : undefined;
    if (opts.explanations !== false && event && explanations.length < 2) {
      const names = opts.structure?.eventSubscribers(event).map((subscriber) => subscriber.name);
      if (names?.length) explanations.push(`Subscriptions in code: ${[...new Set(names)].join(", ")}`);
    }
    // A statement whose whole expression is a constant or a variable has nothing to open.
    const trivial = compact && node.expr !== null && (node.expr.concept === "Literal" || node.expr.concept === "Var");
    const canExpand = (node.expr !== null && !trivial) || reading.some((site) => site.content.kind === "Pattern");
    const shownDetails = expanded[expansionKey(flow.symbol, node.id)];
    const isExpanded = canExpand && (compact ? shownDetails !== false : !!shownDetails);
    const onStatement = (site: ReadingSite) => {
      if (site.path === undefined) return true;
      // Match arms with an expression path belong to the embedded match.
      const pin = site.content.kind === "Pattern" ? site.content.route : undefined;
      return pin !== undefined && !pin.startsWith("case:")
        && node.outputs.some((output) => output.kind === "Exec" && output.id === pin);
    };
    const shownReading = isExpanded
      ? cards.filter(onStatement)
      : compactReading(cards);
    const sequence = opts.explanations === false || isExpanded || shownReading.some((site) => site.content.kind === "Paths")
      ? undefined : sequencePreview(node.expr);
    const stageText = sequence?.stages.flatMap((stage) => [stage.action, stage.timing]);
    const labels = explanations.filter((text) => !stageText?.includes(text));
    const ctx: TreeCtx = {
      flow,
      ownerId: node.id,
      level,
      source,
      stale,
      operations: opts.operations ?? {},
      locals: compact ? localsOf(flow, node.id, hasCapture) : new Map(),
      nodes,
      edges,
      redirected,
      count: 0,
      showDataWires,
      explanations: opts.explanations !== false,
      reading: cards.filter((site) => !onStatement(site)),
    };
    const absorb = compact && isExpanded && !reading.some((site) => site.path === "0") && absorbs(node);
    const argPins = absorb && node.expr ? pinsOf(node.expr, node.id, ctx, "x") : undefined;
    const tree =
      isExpanded && node.expr
        ? absorb
          ? addForest(argPins!, node.id, ctx)
          : addExprTree(node.expr, node.id, { id: node.id, handle: "expr" }, "0", ctx)
        : undefined;
    // Inputs now wired straight into the expression tree are not repeated on the statement.
    const hidden = node.inputs.filter((p) => redirected.has(`${node.id} ${p.id}`)).map((p) => p.id);
    const shown = node.inputs.filter((p) => !hidden.includes(p.id));
    const ports: GPort[] = [];
    for (const p of shown) ports.push({ id: p.id, side: "WEST" });
    if (isExpanded && node.expr && !absorb) ports.push({ id: "expr", side: "WEST" });
    for (const p of argPins?.pins ?? []) ports.push({ id: p.id, side: "WEST" });
    for (const p of node.outputs) ports.push({ id: p.id, side: "EAST" });
    const rootSpan = node.expr && expressionSpan(node.expr);
    const covered = argPins?.covered ?? (tree && rootSpan ? [rootSpan] : []);
    const code = statementCode(node, source, isExpanded, covered, absorb);
    const rows = Math.max(shown.length + (isExpanded && node.expr && !absorb ? 1 : 0) + (argPins?.pins.length ?? 0), node.outputs.length);
    const badges = (opts.explanations !== false && (statementDetailFlavors(node, flow.language, isExpanded, absorb).length > 0 || statementSites(node, isExpanded, absorb).length > 0))
      || node.flavors.some((flavor) => flavor !== "unreachable" && !isExplanationFlavor(flavor)) ? 24 : 0;
    const height = 38 + (code ? 8 + codePreviewHeight(code) : 0) + rows * PIN_ROW + badges
      + labels.length * 21 + (sequence ? 25 + sequence.stages.length * 34 : 0)
      + (sequence?.example ? 25 + sequence.example.length * 17 : 0) + readingHeight(shownReading)
      + rustOwnershipHeight(ownership) + (contract?.length ? 8 + 19 * contract.length : 0);
    const gnode: GNode = {
      id: node.id,
      type: "blueprint",
      data: { node, code, sourceRevision: flow.source_revision, flow: flow.symbol, language: flow.language, expanded: isExpanded,
        explanations: labels, sequence, reading: shownReading, readingExec: routed?.labels, contract, ownership, inputAccess: accessByNode.get(node.id),
        resultInfo: node.concept === "Return" && !stale ? opts.structure?.byId.get(flow.symbol)?.value_info : undefined,
        detailsLabel: node.concept === "Branch" || node.concept === "Loop.While" ? "Condition" : undefined,
        argPins: argPins?.pins, hiddenInputs: hidden, canExpand } satisfies BlueprintData,
      width: BLUEPRINT_WIDTH,
      height,
      ports,
    };
    nodes.push(gnode);
    if (!tree || tree.placed.length === 0) continue;
    // Reserve room below-left of the statement for its expression tree, so the tree feeds the
    // statement's pins from the left without crossing the exec wire on top.
    gnode.box = { width: tree.width + BLUEPRINT_WIDTH, height: height + TREE_GAP + tree.height, offsetX: tree.width };
    for (const n of tree.placed) {
      n.attach = { to: node.id, dx: n.attach!.dx - tree.width, dy: height + TREE_GAP + n.attach!.dy };
    }
  }

  for (const e of flow.edges) {
    const isExec = e.kind === "Exec";
    if (!isExec && !showDataWires) continue;
    if (!isExec && redirected.has(`${e.to} ${e.to_pin}`)) continue;
    const output = isExec ? undefined : flow.nodes.find((n) => n.id === e.from)?.outputs.find((p) => p.id === e.from_pin);
    const input = isExec ? undefined : flow.nodes.find((n) => n.id === e.to)?.inputs.find((p) => p.id === e.to_pin);
    const ty = output?.ty ?? null;
    const shape = stale ? undefined : (input?.type_info ?? output?.type_info)?.shape;
    const access = isExec ? null : (e.access ?? null);
    edges.push({
      id: flowEdgeId(e),
      source: e.from,
      sourceHandle: e.from_pin,
      target: e.to,
      targetHandle: e.to_pin,
      kind: isExec ? "exec" : "data",
      layout: isExec,
      data: { ty, shape, ...wireAccess(access, e.to_pin, captures.has(e.to)) },
    });
  }
  return { nodes, edges, direction: "RIGHT" };
}

const TREE_GAP = 28;
const TREE_COL = EXPR_WIDTH + 44;
const TREE_ROW = 78;
const TREE_ROW_GAP = 8;

function wireAccess(access: Access | null, pin: string, hasCapture: boolean) {
  return access && !displayedInputAccess(access, pin, hasCapture) ? { access: null, deferredBorrow: true } : { access };
}

type LocalWire = {
  from: string; pin: string; ty: string | null; ownership: ReturnType<typeof wireAccess>; valueInfo?: ValueInfo;
};

interface TreeCtx {
  flow: FlowGraph;
  ownerId: string;
  level: DetailLevel;
  source?: Uint8Array;
  stale: boolean;
  operations: Record<string, OperationInfo>;
  /** Local name → the pin that defines it (`var:x` of an earlier statement or the entry). */
  locals: Map<string, LocalWire>;
  nodes: GNode[];
  edges: GEdge[];
  redirected: Set<string>;
  count: number;
  showDataWires: boolean;
  explanations: boolean;
  reading: readonly ReadingSite[];
}

/** Locals a statement reads, from its `use:x` data edges. */
function localsOf(flow: FlowGraph, nodeId: string, hasCapture: boolean): Map<string, LocalWire> {
  const out = new Map<string, LocalWire>();
  for (const e of flow.edges) {
    if (e.kind !== "Data" || e.to !== nodeId || !e.to_pin.startsWith("use:")) continue;
    const pin = flow.nodes.find((n) => n.id === e.from)?.outputs.find((p) => p.id === e.from_pin);
    out.set(e.to_pin.slice(4), {
      from: e.from, pin: e.from_pin, ty: pin?.ty ?? null, ownership: wireAccess(e.access ?? null, e.to_pin, hasCapture), valueInfo: pin?.value_info,
    });
  }
  return out;
}

/** Catalog pin `Name:type` → its parts. */
function splitPin(pin: string | undefined): { name: string | null; ty: string | null } {
  if (!pin) return { name: null, ty: null };
  const i = pin.indexOf(":");
  return i < 0 ? { name: pin, ty: null } : { name: pin.slice(0, i), ty: pin.slice(i + 1) };
}

/** The operation's inputs, without a static receiver (`Math` in `Math.max(a, b)`). */
function operands(expr: Expr, op: OperationInfo | undefined): Expr[] {
  const first = expr.children[0];
  if (op && first?.flavors.includes("target") && op.inputs.length < expr.children.length) return expr.children.slice(1);
  return expr.children;
}

interface Subtree {
  expr: Expr;
  path: string;
  pin: string;
  callbackRoles?: CallbackRoles;
}

interface PinLayout {
  pins: ExprPin[];
  subtrees: Subtree[];
  covered: Span[];
}

function coverExpression(covered: Span[], expr: Expr) {
  const span = expressionSpan(expr);
  if (span) covered.push(span);
}

function inlineOperand(pin: ExprPin, child: Expr, layout: PinLayout, ctx: TreeCtx) {
  const preview = expressionCode(child, ctx.source);
  if (!preview) return;
  pin.value = preview.text;
  pin.emphasis = preview.emphasis;
  coverExpression(layout.covered, child);
}

function describePin(pin: ExprPin, expr: Expr, path: string, ctx: TreeCtx, ownsExpression: boolean) {
  if (!ctx.explanations) return;
  const details = argumentDetails(expr);
  const flavors = ownsExpression ? expressionDetailFlavors(expr, ctx.flow.language) : [];
  const sites = ownsExpression ? expressionSites(expr) : [];
  if (details.length || flavors.length || sites.length) pin.languageDetails = { path, details, flavors, sites };
}

function inlineSubtree(subtree: Subtree, layout: PinLayout, ctx: TreeCtx) {
  const pin = layout.pins.find((pin) => pin.id === subtree.pin)!;
  describePin(pin, subtree.expr, subtree.path, ctx, true);
  inlineOperand(pin, subtree.expr, layout, ctx);
}

/** Pins of `expr` in the compact levels: constants and names inline, locals wired, the rest as subtrees. */
function pinsOf(expr: Expr, id: string, ctx: TreeCtx, prefix = "c", path = "0"): PinLayout {
  const op = expr.op ? ctx.operations[expr.op] : undefined;
  const kids = operands(expr, op);
  // An assignment keeps Target/Value even when it applies an operation (`total += x`).
  const names = op && expr.concept !== "Assign" ? op.inputs : (CONCEPT_PINS[expr.concept] ?? []).map((n) => `${n}:`);
  const layout: PinLayout = { pins: [], subtrees: [], covered: [] };
  const roles = ctx.explanations ? sequenceArgumentRoles(expr) : [];
  const choice = ctx.explanations && ctx.flow.language === "rust" ? rustChoicePresentation(expr) : undefined;
  kids.forEach((child, i) => {
    const pinId = `${prefix}${i}`;
    const childIndex = expr.children.indexOf(child);
    const childPath = `${path}.${childIndex}`;
    const named = splitPin(names[Math.min(i, names.length - 1)]);
    let name = choice?.inputs[childIndex] ?? (named.name || null);
    if (roles[childIndex]) name = roles[childIndex];
    if (!name && ctx.explanations && spreadOutput(expr)) name = "Source";
    if (!name && child.flavors.includes("target")) name = "Target";
    const pin: ExprPin = { id: pinId, name, ty: named.ty || child.ty, value: null, valueInfo: child.value_info, typeInfo: child.type_info, path: childPath };
    // A leaf that becomes a pin has no expression node to show its details.
    const ownsExpression = ctx.level !== "statements" && (child.concept === "Var" || child.concept === "Literal");
    describePin(pin, child, childPath, ctx, ownsExpression);
    layout.pins.push(pin);
    if (ctx.level === "statements" && child.concept === "Var" && ctx.reading.length && ctx.showDataWires
      && !(expr.concept === "Assign" && childIndex === 0)) {
      const param = lambdaParam(ctx, child.label);
      if (param) {
        pin.name = name ?? child.label;
        ctx.edges.push({
          id: `${param}:p:${child.label}->${id}:${pinId}`,
          source: param, sourceHandle: `p:${child.label}`, target: id, targetHandle: pinId,
          kind: "data", layout: false, data: { ty: pin.ty, shape: pin.typeInfo?.shape },
        });
        coverExpression(layout.covered, child);
        return;
      }
    }
    if (ctx.level === "statements") {
      layout.subtrees.push({ expr: child, path: childPath, pin: pinId });
      return;
    }
    if (child.concept === "Literal") {
      inlineOperand(pin, child, layout, ctx);
      pin.ty = pin.ty ?? child.ty;
      return;
    }
    if (child.concept === "Var") {
      const writeTarget = expr.concept === "Assign" && childIndex === 0;
      const param = lambdaParam(ctx, child.label);
      const local = param ? undefined : ctx.locals.get(child.label);
      const scopes = lambdaScopes.get(ctx);
      if (ctx.explanations && (param || local) && scopes?.length && !scopes.at(-1)?.has(child.label)) {
        pin.name = ctx.showDataWires && !writeTarget
          ? `${name ? `${name} · ` : ""}${child.label} (outer scope)`
          : `${name ? `${name} · ` : ""}outer scope`;
      }
      if (param && ctx.showDataWires) {
        ctx.edges.push({
          id: `${param}:p:${child.label}->${id}:${pinId}`,
          source: param,
          sourceHandle: `p:${child.label}`,
          target: id,
          targetHandle: pinId,
          kind: "data",
          layout: false,
          data: { ty: pin.ty, shape: pin.typeInfo?.shape },
        });
        coverExpression(layout.covered, child);
        if (!writeTarget) return;
      }
      if (local) {
        pin.ty = pin.ty ?? local.ty;
        if (local.valueInfo && (!ctx.stale || local.valueInfo.basis === "Syntax") && !child.syntax_sites?.some((s) => ["deref", "try"].includes(s.key))) {
          if (!pin.valueInfo || (pin.valueInfo.kind === "Unknown" && pin.valueInfo.basis === "Syntax")) {
            pin.valueInfo = { ...local.valueInfo, passing: pin.valueInfo?.passing };
          }
        }
        ctx.redirected.add(`${ctx.ownerId} use:${child.label}`);
        if (ctx.showDataWires) {
          ctx.edges.push({
            id: `${local.from}:${local.pin}->${id}:${pinId}`,
            source: local.from,
            sourceHandle: local.pin,
            target: id,
            targetHandle: pinId,
            kind: "data",
            layout: false,
            data: { ty: local.ty, shape: pin.typeInfo?.shape, ...local.ownership },
          });
          coverExpression(layout.covered, child);
          if (!writeTarget) return;
        }
      }
      inlineOperand(pin, child, layout, ctx);
      return;
    }
    layout.subtrees.push({ expr: child, path: childPath, pin: pinId,
      callbackRoles: ctx.explanations && child.concept === "Lambda" ? sequenceCallbackRoles(expr, childIndex) : undefined });
  });
  return layout;
}

/** Lambda parameters in scope while a lambda body is visited (Everything level): name → lambda node id. */
const lambdaScopes = new WeakMap<TreeCtx, Map<string, string>[]>();

function lambdaParam(ctx: TreeCtx, name: string): string | undefined {
  const scopes = lambdaScopes.get(ctx) ?? [];
  for (let i = scopes.length - 1; i >= 0; i--) {
    const hit = scopes[i].get(name);
    if (hit) return hit;
  }
  return undefined;
}

/** Several subtrees feeding one node (the statement's argument pins), stacked vertically. */
function addForest(layout: PinLayout, ownerId: string, ctx: TreeCtx) {
  const placed: GNode[] = [];
  let width = 0;
  let height = 0;
  for (const s of layout.subtrees) {
    if (ctx.count >= MAX_EXPR_NODES) {
      inlineSubtree(s, layout, ctx);
      continue;
    }
    const tree = addExprTree(s.expr, ownerId, { id: ownerId, handle: s.pin }, s.path, ctx, s.callbackRoles);
    coverExpression(layout.covered, s.expr);
    for (const n of tree.placed) n.attach!.dy += height;
    placed.push(...tree.placed);
    width = Math.max(width, tree.width);
    height += tree.height;
  }
  // Right-align the trees: each root sits in the column next to the statement.
  for (const s of placed) {
    const own = (s as GNode & { treeWidth?: number }).treeWidth ?? width;
    s.attach!.dx += width - own;
  }
  return { width, height, placed };
}

/**
 * Adds an expression tree as pure nodes. Positions are relative: the root sits in the right-most
 * column (next to its statement) and children fan out to the left, like data inputs in a
 * Blueprint. Returns the tree's size and the placed nodes.
 */
function addExprTree(root: Expr, ownerId: string, target: { id: string; handle: string }, rootPath: string, ctx: TreeCtx, rootRoles?: CallbackRoles) {
  let leaves = 0;
  let maxDepth = 0;
  const nextRow = new Map<number, number>();
  const placed: GNode[] = [];
  let column = TREE_COL;
  const visit = (expr: Expr, path: string, depth: number, parent: { id: string; handle: string }, callbackRoles?: CallbackRoles): number => {
    ctx.count++;
    maxDepth = Math.max(maxDepth, depth);
    const id = `${ownerId}/e/${path}`;
    const isLambda = expr.concept === "Lambda";
    const reading = ctx.reading.filter((site) => site.path === path);
    const expandLambda = isLambda && expr.children.length > 0 && (ctx.level === "everything"
      || ctx.reading.some((site) => site.path === path || site.path?.startsWith(path + ".")));
    const result = callbackRoles?.result ?? (ctx.explanations ? "Result for the caller" : "Result");
    const layout: PinLayout =
      isLambda && !expandLambda
        ? { pins: [], subtrees: [], covered: [] }
        : expandLambda
          ? { pins: [{ id: "c0", name: result, ty: null, value: null, typeInfo: expr.children[0]?.type_info }], subtrees: expr.children.slice(0, 1).map((c) => ({ expr: c, path: `${path}.0`, pin: "c0" })), covered: [] }
          : pinsOf(expr, id, ctx, "c", path);
    const { pins, subtrees, covered } = layout;
    const patternBindings = reading.flatMap(({ content }) => content.kind === "Pattern"
      ? content.parts.flatMap((part) => part.binding ? [part.binding] : []) : []);
    const params = expandLambda ? [...new Set([
      ...(expr.param_pins?.map((pin) => pin.label) ?? expr.params).filter((name) => patternBindings.length === 0 || /^[\p{L}_][\p{L}\p{N}_]*$/u.test(name)),
      ...patternBindings,
    ])] : [];
    const ports: GPort[] = pins.map((p) => ({ id: p.id, side: "WEST" as const }));
    ports.push({ id: "out", side: "EAST" });
    for (const p of params) ports.push({ id: `p:${p}`, side: "EAST" });
    const rows = Math.max(pins.length, 1) + params.length;
    const explanations = ctx.explanations ? expressionExplanations(expr, ctx.flow.language, reading.length > 0).slice(0, 2) : [];
    const width = reading.some((site) => site.content.kind !== "Detail") ? READING_EXPR_WIDTH : EXPR_WIDTH;
    column = Math.max(column, width + (TREE_COL - EXPR_WIDTH));
    const data: ExprData = { expr, ownerId, path, flow: ctx.flow.symbol, language: ctx.flow.language, pins, params,
      explanations, paramRoles: callbackRoles?.params, reading, sourceRevision: ctx.flow.source_revision, width };
    const gnode: GNode<ExprData> & { treeWidth?: number } = {
      id,
      type: "expr",
      data,
      width,
      height: 34 + rows * EXPR_PIN_ROW + explanations.length * 21 + readingHeight(reading),
      ports,
      attach: { to: ownerId, dx: depth, dy: 0 },
    };
    ctx.nodes.push(gnode);
    placed.push(gnode);
    ctx.edges.push({
      id: `${id}->${parent.id}:${parent.handle}`,
      source: id,
      sourceHandle: "out",
      target: parent.id,
      targetHandle: parent.handle,
      kind: "expr",
      layout: false,
      data: { ty: expr.ty, shape: expr.type_info?.shape },
    });
    if (expandLambda) {
      const scopes = lambdaScopes.get(ctx) ?? [];
      scopes.push(new Map(params.map((p) => [p, id])));
      lambdaScopes.set(ctx, scopes);
    }
    const ys: number[] = [];
    for (const s of subtrees) {
      if (ctx.count < MAX_EXPR_NODES) {
        ys.push(visit(s.expr, s.path, depth + 1, { id, handle: s.pin }, s.callbackRoles));
        coverExpression(covered, s.expr);
      } else {
        inlineSubtree(s, layout, ctx);
      }
    }
    if (!expandLambda && expr.concept !== "Pipeline") {
      data.code = expressionCode(expr, ctx.source, covered);
      gnode.height += codePreviewHeight(data.code);
    }
    let y: number;
    if (ys.length === 0) {
      y = leaves * TREE_ROW;
      leaves++;
    } else {
      y = (ys[0] + ys[ys.length - 1]) / 2;
    }
    if (expandLambda) lambdaScopes.get(ctx)?.pop();
    y = Math.max(y, nextRow.get(depth) ?? 0);
    nextRow.set(depth, y + Math.max(TREE_ROW, gnode.height + TREE_ROW_GAP));
    gnode.attach!.dy = y;
    return y;
  };
  visit(root, rootPath, 0, target, rootRoles);
  const width = (maxDepth + 1) * column;
  for (const n of placed) {
    // depth → x: depth 0 is the right-most column.
    n.attach!.dx = width - (n.attach!.dx + 1) * column + (column - n.width - (TREE_COL - EXPR_WIDTH));
    (n as GNode & { treeWidth?: number }).treeWidth = width;
  }
  return { width, height: Math.max(leaves * TREE_ROW, ...nextRow.values()), placed };
}
