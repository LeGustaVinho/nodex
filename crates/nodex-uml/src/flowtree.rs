//! Flow graphs read as nested blocks, and the calls each statement makes.
//!
//! Frontends build structured flows: a compound statement (if, switch, loop, try, using, lock)
//! has one exec pin per nested body (`true`/`false`, `case:<i>`, `body`, `try`/`catch:<i>`/
//! `finally`, `variant:<i>`, `else`) and an `out` pin for what runs after it. A body is the chain
//! of `out` edges from the first statement. Activity and sequence diagrams walk this tree.

use std::collections::HashSet;

use nodex_ir::{Concept, Expr, FlowEdgeKind, FlowGraph, FlowNode, PinKind, Span};

/// A chain of statements.
pub struct Block<'f> {
    pub steps: Vec<Step<'f>>,
}

impl Block<'_> {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

/// One statement and its nested bodies.
pub struct Step<'f> {
    pub node: &'f FlowNode,
    pub arms: Vec<Arm<'f>>,
    /// Control continues after the statement.
    pub falls: bool,
}

/// A nested body: a branch, a case, a loop body, a catch clause.
pub struct Arm<'f> {
    pub pin: String,
    pub label: String,
    /// Exception type of a `catch:<i>` arm (`*`: any).
    pub ty: Option<String>,
    pub block: Block<'f>,
}

impl Step<'_> {
    pub fn arm(&self, pin: &str) -> Option<&Arm<'_>> {
        self.arms.iter().find(|a| a.pin == pin)
    }
}

/// The entry node of a flow (its trigger: "when X is called", "Unity calls Update").
pub fn entry(flow: &FlowGraph) -> Option<&FlowNode> {
    flow.nodes.iter().find(|n| matches!(n.concept, Concept::Entry | Concept::EventEntry))
}

/// The body of a flow, after its entry node.
pub fn tree(flow: &FlowGraph) -> Block<'_> {
    let mut seen = HashSet::new();
    let start = entry(flow).and_then(|e| target(flow, &e.id, "out"));
    block(flow, start, &mut seen)
}

fn target<'f>(flow: &'f FlowGraph, node: &str, pin: &str) -> Option<&'f str> {
    flow.edges.iter().find(|e| e.kind == FlowEdgeKind::Exec && e.from == node && e.from_pin == pin).map(|e| e.to.as_str())
}

fn block<'f>(flow: &'f FlowGraph, mut cur: Option<&'f str>, seen: &mut HashSet<&'f str>) -> Block<'f> {
    let mut steps = Vec::new();
    while let Some(id) = cur {
        // Flows are trees; a repeated node would mean a malformed graph.
        if !seen.insert(id) {
            break;
        }
        let Some(node) = flow.node(id) else { break };
        let mut arms = Vec::new();
        for pin in node.outputs.iter().filter(|p| p.kind == PinKind::Exec && p.id != "out") {
            let body = block(flow, target(flow, id, &pin.id), seen);
            arms.push(Arm { pin: pin.id.clone(), label: pin.label.clone(), ty: pin.ty.clone(), block: body });
        }
        let has_out = node.outputs.iter().any(|p| p.kind == PinKind::Exec && p.id == "out");
        steps.push(Step { node, arms, falls: has_out && !node.concept.is_terminal() });
        cur = target(flow, id, "out");
    }
    Block { steps }
}

/// A call or object creation inside a statement.
pub struct CallSite<'f> {
    /// Called name (`CancelOrderAsync`, `post`) or created type (`Order`).
    pub name: String,
    pub receiver: Option<&'f Expr>,
    pub expr: &'f Expr,
    pub construct: bool,
    /// Awaited where it is made.
    pub awaited: bool,
    /// Inside a concurrency combinator (`Task.WhenAll`, `Promise.all`, `join!`): its index and
    /// the argument it is in.
    pub parallel: Option<(usize, usize)>,
    /// Arguments as short text.
    pub args: Vec<String>,
}

const PARALLEL_CALLS: &[&str] = &["WhenAll", "WhenAny", "all", "allSettled", "race", "any", "join_all", "try_join_all", "join", "try_join", "select"];

/// Calls and creations of a statement in evaluation order (arguments before the call). Lambda
/// bodies are left out: they run later, when something calls them.
pub fn call_sites(node: &FlowNode) -> Vec<CallSite<'_>> {
    let mut out = Vec::new();
    let mut groups = 0;
    if let Some(e) = &node.expr {
        walk(e, false, None, &mut groups, &mut out);
    }
    out
}

fn walk<'f>(e: &'f Expr, awaited: bool, parallel: Option<(usize, usize)>, groups: &mut usize, out: &mut Vec<CallSite<'f>>) {
    match e.concept {
        Concept::Lambda => return,
        Concept::Await => {
            for c in &e.children {
                walk(c, true, parallel, groups, out);
            }
            return;
        }
        _ => {}
    }
    let receiver = e.children.iter().find(|c| c.flavors.iter().any(|f| f == "target"));
    let is_parallel = matches!(e.concept, Concept::Call | Concept::Macro)
        && PARALLEL_CALLS.contains(&e.label.trim_end_matches('!'))
        && (receiver.is_none_or(|r| matches!(text(r).as_str(), "Task" | "Promise" | "futures" | "future" | "tokio")));
    if is_parallel {
        let group = *groups;
        *groups += 1;
        let mut branch = 0;
        for c in &e.children {
            if receiver.is_some_and(|r| std::ptr::eq(r, c)) {
                continue;
            }
            walk(c, awaited, Some((group, branch)), groups, out);
            branch += 1;
        }
        return;
    }
    for c in &e.children {
        // A call awaited as a whole is awaited; its arguments are not.
        walk(c, false, parallel, groups, out);
    }
    let is_stage = e.flavors.iter().any(|f| f == "stage") && !e.flavors.iter().any(|f| f == "custom-stage");
    if (e.concept == Concept::Call && !is_stage) || e.concept == Concept::Construct {
        let args = e.children.iter().filter(|c| !receiver.is_some_and(|r| std::ptr::eq(r, *c))).map(|c| crate::mermaid::shorten(&text(c), 24)).collect();
        out.push(CallSite {
            name: e.label.trim_start_matches("new ").trim().to_owned(),
            receiver,
            expr: e,
            construct: e.concept == Concept::Construct,
            awaited,
            parallel,
            args,
        });
    }
}

/// Short source-like text of an expression (`_db.Orders`, `this.repo`, `"x"`).
pub fn text(e: &Expr) -> String {
    match e.concept {
        Concept::Var | Concept::Literal => e.label.clone(),
        Concept::GetMember => {
            let recv = e.children.first().map(text).unwrap_or_default();
            format!("{recv}{}", e.label)
        }
        Concept::SafeAccess => e.children.iter().map(text).collect::<Vec<_>>().join("?"),
        Concept::Call => {
            let recv = e.children.iter().find(|c| c.flavors.iter().any(|f| f == "target")).map(text);
            match recv {
                Some(r) => format!("{r}.{}()", e.label),
                None => format!("{}()", e.label),
            }
        }
        Concept::Await => format!("await {}", e.children.first().map(text).unwrap_or_default()),
        Concept::Operation | Concept::BinaryOp if e.children.len() == 2 => format!("{} {} {}", text(&e.children[0]), e.label, text(&e.children[1])),
        Concept::UnaryOp | Concept::Operation if e.children.len() == 1 => format!("{}{}", e.label, text(&e.children[0])),
        Concept::Construct => format!("new {}", e.label.trim_start_matches("new ")),
        Concept::Lambda => "λ".into(),
        Concept::Compose if e.label == "text with values" => "`…`".into(),
        _ => e.label.clone(),
    }
}

/// The type a `Throw` statement raises (`OrderNotFoundException`), read from `throw new X(...)`.
pub fn thrown_type(node: &FlowNode) -> Option<String> {
    fn find(e: &Expr) -> Option<String> {
        if e.concept == Concept::Construct {
            return Some(e.label.trim_start_matches("new ").trim().to_owned());
        }
        e.children.iter().find_map(find)
    }
    node.expr.as_ref().and_then(find)
}

/// True when `inner` lies inside `outer` (same file).
pub fn within(inner: &Span, outer: &Span) -> bool {
    inner.file == outer.file && outer.start_byte <= inner.start_byte && inner.end_byte <= outer.end_byte
}
