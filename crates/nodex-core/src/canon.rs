//! Canonical text forms of flow graphs.
//!
//! The canonical form keeps only concepts and the exec structure (no labels, flavors or
//! language details). Two implementations of the same algorithm in different languages should
//! produce the same canonical form — this is how the Rosetta tests enforce "same logic, same
//! visual".

use std::collections::HashSet;
use std::fmt::Write;

use nodex_ir::{Concept, Expr, FlowGraph};

/// Renders the exec structure with concepts only.
pub fn canonical(flow: &FlowGraph) -> String {
    render(flow, false)
}

/// Renders the exec structure with natural-language labels (handy for debugging and snapshots).
pub fn outline(flow: &FlowGraph) -> String {
    render(flow, true)
}

/// Renders the operations of each statement, in exec order: `Var add(multiply())`. Only
/// recognized operations appear (nested ones inside their parent), so two languages that write
/// the same logic produce the same text.
pub fn operations(flow: &FlowGraph) -> String {
    let mut out = String::new();
    let mut seen = HashSet::new();
    let mut order = Vec::new();
    for entry in flow.nodes.iter().filter(|n| matches!(n.concept, Concept::Entry | Concept::EventEntry)) {
        exec_order(flow, &entry.id, &mut seen, &mut order);
    }
    for id in order {
        let Some(node) = flow.node(&id) else { continue };
        let forms = node.expr.as_ref().map(op_forms).unwrap_or_default();
        if !forms.is_empty() {
            let _ = writeln!(out, "{} {}", node.concept.key(), forms.join(" "));
        }
    }
    out
}

fn op_forms(e: &Expr) -> Vec<String> {
    let inner: Vec<String> = e.children.iter().flat_map(op_forms).collect();
    match &e.op {
        Some(op) => vec![format!("{op}({})", inner.join(", "))],
        None => inner,
    }
}

fn exec_order(flow: &FlowGraph, start: &str, seen: &mut HashSet<String>, out: &mut Vec<String>) {
    let mut current = Some(start.to_owned());
    while let Some(id) = current.take() {
        if !seen.insert(id.clone()) {
            return;
        }
        out.push(id.clone());
        for edge in flow.exec_out(&id).filter(|e| e.from_pin != "out") {
            exec_order(flow, &edge.to, seen, out);
        }
        current = flow.exec_out(&id).find(|e| e.from_pin == "out").map(|e| e.to.clone());
    }
}

fn render(flow: &FlowGraph, labels: bool) -> String {
    let mut out = String::new();
    let mut seen = HashSet::new();
    for entry in flow.nodes.iter().filter(|n| matches!(n.concept, Concept::Entry | Concept::EventEntry)) {
        chain(flow, &entry.id, 0, labels, &mut seen, &mut out);
    }
    // Nodes no exec path reaches (unreachable code) are listed at the end.
    let orphans: Vec<_> = flow.nodes.iter().filter(|n| !seen.contains(&n.id)).collect();
    if !orphans.is_empty() {
        out.push_str("unreachable:\n");
        for n in orphans {
            line(&mut out, 1, n.concept, labels.then_some(n.label.as_str()));
        }
    }
    out
}

fn line(out: &mut String, depth: usize, concept: Concept, label: Option<&str>) {
    let _ = match label {
        Some(l) => writeln!(out, "{}{} — {}", "  ".repeat(depth), concept.key(), l),
        None => writeln!(out, "{}{}", "  ".repeat(depth), concept.key()),
    };
}

fn chain(flow: &FlowGraph, start: &str, depth: usize, labels: bool, seen: &mut HashSet<String>, out: &mut String) {
    let mut current = Some(start.to_owned());
    while let Some(id) = current.take() {
        if !seen.insert(id.clone()) {
            return;
        }
        let Some(node) = flow.node(&id) else { return };
        line(out, depth, node.concept, labels.then_some(node.label.as_str()));
        for pin in node.outputs.iter().filter(|p| p.kind == nodex_ir::PinKind::Exec && p.id != "out") {
            let role = pin.id.split(':').next().unwrap_or(&pin.id);
            let _ = writeln!(out, "{}{}:", "  ".repeat(depth + 1), role);
            if let Some(edge) = flow.exec_out(&id).find(|e| e.from_pin == pin.id) {
                chain(flow, &edge.to, depth + 2, labels, seen, out);
            }
        }
        current = flow.exec_out(&id).find(|e| e.from_pin == "out").map(|e| e.to.clone());
    }
}
