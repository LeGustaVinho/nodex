//! Explain bounded inclusive range patterns in `match` arms.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "range_pattern" || node.has_error() { return None; }
    let low = node.child_by_field_name("left")?;
    let high = node.child_by_field_name("right")?;
    let op = children(node).into_iter().find(|part| part.kind() == "..=")?;
    let mut current = node;
    let arm = loop {
        current = current.parent()?;
        if current.kind() == "match_arm" { break current; }
        // A range inside an expression in a guard or arm body is not the arm's pattern.
        if matches!(current.kind(), "block" | "match_expression") { return None; }
    };
    let pattern = arm.child_by_field_name("pattern")?;
    if node.start_byte() < pattern.start_byte() || node.end_byte() > pattern.end_byte() { return None; }
    let owner = arm.parent()?.parent().filter(|parent| parent.kind() == "match_expression")?;
    Some((owner, op, ReadingContent::Detail {
        label: "Match an inclusive range".into(),
        text: format!("The pattern {} checks whether the matched value is at least {} and at most {}. The `..=` includes both endpoints: a value equal to either endpoint matches. This is a pattern check for the arm, not a loop over the numbers in the range. A guard, if written on the arm, is checked after the pattern matches.", src.norm(node), src.norm(low), src.norm(high)),
        definition: None,
        borrow: false,
    }))
}
