//! Explain the optional initial `|` in a match-arm pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(_src: Src<'_>, arm: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if arm.kind() != "match_arm" || arm.has_error() { return None; }
    let owner = arm.parent()?.parent().filter(|node| node.kind() == "match_expression")?;
    let wrapper = arm.child_by_field_name("pattern")?;
    let mut pattern = named_children(wrapper).into_iter().next()?;
    if pattern.kind() != "or_pattern" { return None; }
    // Tree-sitter nests the first alternative on the left; the optional initial
    // separator is the first child of the leftmost or-pattern.
    while let Some(left) = named_children(pattern).into_iter().next().filter(|child| child.kind() == "or_pattern") { pattern = left; }
    let first = children(pattern).into_iter().next()?;
    if first.kind() != "|" || first.is_missing() { return None; }
    Some((owner, first, ReadingContent::Detail {
        label: "Initial separator in a match pattern".into(),
        text: "The first `|` before a match-arm pattern is optional. It is a leading separator for formatting and does not add an empty alternative or change which values match. Each following `|` separates alternative patterns for this same arm; if any alternative matches and its guard, if present, succeeds, the arm is selected. Alternatives must introduce the same names with the same types and binding modes.".into(),
        borrow: false, definition: None,
    }))
}

