//! Explain an exclusive range-to pattern in a match arm, distinct from range expressions.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "range_pattern" || node.has_error() || node.child_by_field_name("left").is_some() { return None; }
    let upper = node.child_by_field_name("right")?;
    let operator = children(node).into_iter().find(|child| child.kind() == "..")?;
    // This fact is attached to the match node so it is visible even though an
    // arm's pattern is not itself a separate flow node.
    let mut current = node.parent();
    let mut in_arm = false;
    let match_node = loop {
        let ancestor = current?;
        if ancestor.kind() == "match_arm" { in_arm = true; }
        if ancestor.kind() == "match_expression" { break in_arm.then_some(ancestor)?; }
        if matches!(ancestor.kind(), "function_item" | "closure_expression" | "async_block") { return None; }
        current = ancestor.parent();
    };
    let end = src.norm(upper);
    Some((match_node, operator, ReadingContent::Detail {
        label: "Pattern below an exclusive limit".into(),
        text: format!("In a pattern, ..{end} matches values strictly less than {end}; the upper endpoint {end} itself is excluded. No lower endpoint is written, so matching can start at the lowest value representable by the matched type; it does not imply an unlimited numeric domain. The endpoint must be valid as a constant bound for the pattern, and the matched type determines which values can be compared. In a match, this is a test for selecting an arm, not the construction of a range value."),
        definition: None,
        borrow: false,
    }))
}
