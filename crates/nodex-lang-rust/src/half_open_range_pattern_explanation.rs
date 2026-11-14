//! Explain a bounded half-open range used as a match pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Only handle a syntactic `start..end` pattern in a match arm. Expressions,
/// inclusive patterns and open-ended patterns have different meanings.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "range_pattern" || node.has_error() { return None; }
    let start = node.child_by_field_name("left")?;
    let end = node.child_by_field_name("right")?;
    let operator = children(node).into_iter().find(|part| part.kind() == ".." && !part.is_missing())?;
    let arm = std::iter::successors(node.parent(), |parent| parent.parent())
        .take_while(|parent| !matches!(parent.kind(), "function_item" | "closure_expression"))
        .find(|parent| parent.kind() == "match_arm")?;
    let owner = arm.parent()?.parent()?;
    if owner.kind() != "match_expression" { return None; }
    Some((owner, operator, ReadingContent::Detail {
        label: "Match a half-open range".into(),
        text: format!("In this pattern, {}..{} matches a value greater than or equal to {} and strictly less than {}: the lower bound is included, and the upper bound is excluded. For example, 1..10 matches 1 through 9, not 10. This tests the value against a pattern rather than creating a range value. Rust checks the validity of range-pattern bounds and types at compile time; runtime local variables cannot be used as pattern bounds. The arm is selected only after earlier arms fail to select a value and any guard on this arm passes.", src.norm(start), src.norm(end), src.norm(start), src.norm(end)),
        definition: None,
        borrow: false,
    }))
}
