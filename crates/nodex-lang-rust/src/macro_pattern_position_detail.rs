//! Explain macro invocations syntactically appearing inside patterns.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Span, Node<'t>, ReadingContent)> {
    if node.kind() == "match_expression" {
        let body = node.child_by_field_name("body")?;
        for arm in named_children(body).into_iter().filter(|part| part.kind() == "match_arm") {
            let Some(pattern) = arm.child_by_field_name("pattern") else { continue; };
            let condition = pattern.child_by_field_name("condition");
            let mut pending = vec![pattern];
            while let Some(part) = pending.pop() {
                if Some(part) == condition { continue; }
                if part.kind() == "macro_invocation" { if let Some(found) = explain(src, part) { return Some(found); } continue; }
                pending.extend(named_children(part).into_iter().rev());
            }
        }
        return None;
    }
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    let mut child = node;
    let mut parent = node.parent();
    let anchor = loop {
        let current = parent?;
        if current.kind() == "match_pattern" {
            if current.child_by_field_name("condition").is_some_and(|condition| condition.start_byte() <= node.start_byte() && condition.end_byte() >= node.end_byte()) { return None; }
            break std::iter::successors(current.parent(), Node::parent).find(|p| p.kind() == "match_expression")?;
        }
        if current.kind() == "let_declaration" {
            if current.child_by_field_name("pattern") != Some(child) { return None; }
            break current;
        }
        if !matches!(current.kind(), "tuple_pattern" | "tuple_struct_pattern" | "struct_pattern" | "field_pattern" | "slice_pattern" | "or_pattern" | "reference_pattern" | "ref_pattern" | "mut_pattern" | "captured_pattern") { return None; }
        child = current;
        parent = current.parent();
    };
    let anchor = if anchor.kind() == "match_expression" { src.header_span(anchor, anchor.child_by_field_name("body")) } else { src.span(anchor) };
    Some((anchor, node, ReadingContent::Detail {
        label: "Macro in a pattern".into(),
        text: format!("`{}!` is a macro invocation written where Rust expects a pattern. During compilation the macro expands into syntax that must be valid for this pattern position; the resulting pattern is then used by the surrounding match or binding. The invocation's tokens do not themselves reveal which values will match, which names will be bound, or whether matching moves, copies, or borrows anything. Name resolution and macro expansion determine its meaning, and the compiler checks the resulting pattern's validity. In particular, a `let` binding normally requires an irrefutable pattern unless it has an `else` clause.", src.norm(name)),
        definition: None, borrow: false,
    }))
}
