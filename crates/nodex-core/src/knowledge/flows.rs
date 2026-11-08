//! Knowledge read from flow graphs. It is language-agnostic: the universal concepts already say
//! where decisions, throws and catch clauses are, in every language.

use std::collections::{HashMap, HashSet, VecDeque};

use nodex_ir::{Concept, Expr, FlowEdgeKind, FlowGraph, NodeId, PinKind};

/// Cyclomatic complexity of a function body: 1 + the decisions it makes.
///
/// A decision is an `if`, each extra `match`/`switch` case, a loop, a `catch` clause, an
/// error-propagation point (`?`), and each short-circuit operator (`&&`, `||`, `??`, `?:`).
pub fn complexity(flow: &FlowGraph) -> u32 {
    let mut c = 1u32;
    for node in &flow.nodes {
        let exec_outs = |prefix: &str| {
            node.outputs.iter().filter(|p| p.kind == PinKind::Exec && p.id.starts_with(prefix)).count() as u32
        };
        c += match node.concept {
            Concept::Branch => 1,
            Concept::Match => exec_outs("case:").saturating_sub(1),
            Concept::LoopWhile | Concept::LoopRange | Concept::LoopForEach | Concept::LoopInfinite => 1,
            Concept::Try => exec_outs("catch:"),
            Concept::PropagateError => 1,
            _ => 0,
        };
        if let Some(e) = &node.expr {
            c += expr_decisions(e);
        }
    }
    c
}

fn expr_decisions(e: &Expr) -> u32 {
    let own = match e.concept {
        Concept::Fallback | Concept::Choose | Concept::SafeAccess => 1,
        Concept::BinaryOp | Concept::Operation if matches!(e.op.as_deref(), Some("and" | "or")) => 1,
        // Lambdas are separate functions; their decisions are not this body's.
        Concept::Lambda => return 0,
        _ => 0,
    };
    own + e.children.iter().map(expr_decisions).sum::<u32>()
}

/// A `throw` statement of a flow and the exception type it creates, when the code names one.
#[derive(Clone, Debug, PartialEq)]
pub struct Throw {
    pub node: NodeId,
    /// Type as written (`InvalidOperationException`, `Domain.NotFound`); `None` for rethrows and
    /// thrown variables.
    pub ty: Option<String>,
}

/// Throw statements and throw expressions (`x ?? throw new ArgumentNullException(...)`).
pub fn throws(flow: &FlowGraph) -> Vec<Throw> {
    let mut out = Vec::new();
    for n in &flow.nodes {
        let before = out.len();
        let mut rest = n.code.as_str();
        while let Some(at) = rest.find("throw new ") {
            out.push(Throw { node: n.id.clone(), ty: thrown_type(&rest[at..]) });
            rest = &rest[at + "throw new ".len()..];
        }
        if out.len() == before && n.concept == Concept::Throw {
            out.push(Throw { node: n.id.clone(), ty: thrown_type(&n.code) });
        }
    }
    out
}

/// The type created by `throw new X(...)` / `throw new X { }`.
pub fn thrown_type(code: &str) -> Option<String> {
    let at = code.find("new ")?;
    let ty: String = code[at + 4..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | ':' | '$'))
        .collect();
    let ty = ty.trim_matches(['.', ':']).replace("::", ".");
    (!ty.is_empty()).then_some(ty)
}

/// For every node inside a `try` body, the exception types the enclosing `catch` clauses handle
/// (`*` handles everything). Read from the `catch:<i>` pin types.
pub fn catch_regions(flow: &FlowGraph) -> HashMap<NodeId, Vec<String>> {
    let mut out: HashMap<NodeId, Vec<String>> = HashMap::new();
    for node in flow.nodes.iter().filter(|n| n.concept == Concept::Try) {
        let types: Vec<String> = node
            .outputs
            .iter()
            .filter(|p| p.kind == PinKind::Exec && p.id.starts_with("catch:"))
            .map(|p| p.ty.clone().unwrap_or_else(|| "*".to_owned()))
            .collect();
        if types.is_empty() {
            continue;
        }
        for id in reachable(flow, &node.id, "try") {
            out.entry(id).or_default().extend(types.iter().cloned());
        }
    }
    out
}

/// Nodes reached from `pin` of `start` by exec wires. Blueprint bodies own their paths (a body
/// never wires back to its owner's continuation), so this is exactly the body.
fn reachable(flow: &FlowGraph, start: &str, pin: &str) -> HashSet<NodeId> {
    let mut seen = HashSet::new();
    let mut queue: VecDeque<&str> = flow
        .edges
        .iter()
        .filter(|e| e.kind == FlowEdgeKind::Exec && e.from == start && e.from_pin == pin)
        .map(|e| e.to.as_str())
        .collect();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.to_owned()) {
            continue;
        }
        for e in flow.edges.iter().filter(|e| e.kind == FlowEdgeKind::Exec && e.from == id) {
            queue.push_back(&e.to);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thrown_types_are_read_from_the_statement() {
        assert_eq!(thrown_type("throw new InvalidOperationException(\"x\");").as_deref(), Some("InvalidOperationException"));
        assert_eq!(thrown_type("throw new Domain.NotFound { Id = id };").as_deref(), Some("Domain.NotFound"));
        assert_eq!(thrown_type("throw new Error(`bad ${x}`)").as_deref(), Some("Error"));
        assert_eq!(thrown_type("throw;"), None);
        assert_eq!(thrown_type("throw err;"), None);
    }
}
