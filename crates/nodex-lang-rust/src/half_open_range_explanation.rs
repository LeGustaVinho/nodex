//! Explain bounded `start..end` range expressions.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::{syntax::{children, Src}, syntax_sites::rust_assignee_rest};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "range_expression" || node.has_error() || rust_assignee_rest(node) { return None; }
    let parts = children(node);
    let operator = parts.iter().copied().find(|part| part.kind() == "..")?;
    let start = parts.iter().copied().find(|part| part.is_named() && !part.is_extra() && part.end_byte() <= operator.start_byte())?;
    let end = parts.iter().copied().find(|part| part.is_named() && !part.is_extra() && part.start_byte() >= operator.end_byte())?;
    if matches!(start.kind(), "return_expression" | "break_expression" | "continue_expression" | "yield_expression") { return None; }
    Some((operator, ReadingContent::Detail {
        label: "Create a range excluding its end".into(),
        text: format!("{}..{} creates a range value with a start bound that is included and an end bound that is excluded. For integer bounds, 0..10 covers 0 through 9. The bounds are evaluated when the range expression runs; creating the range does not create a list of all its values. An integer range can be used by for to produce values one at a time. A range with usize bounds can index a slice, subject to the slice's bounds. Other endpoint types can support different uses.", src.norm(start), src.norm(end)),
        definition: None,
        borrow: false,
    }))
}
