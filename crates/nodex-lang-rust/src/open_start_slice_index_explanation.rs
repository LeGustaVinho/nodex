//! Explain an open-start exclusive range used in indexing.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Describe `value[..end]` syntactically without assuming the operand's type.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "index_expression" || node.has_error() { return None; }
    let items = named_children(node);
    if items.len() != 2 { return None; }
    let value = items[0];
    let range = items[1];
    if range.kind() != "range_expression" || range.has_error() { return None; }
    let parts = children(range);
    let operator = parts.iter().find(|part| part.kind() == ".." && !part.is_missing())?;
    if parts.iter().any(|part| part.is_named() && !part.is_extra() && part.end_byte() <= operator.start_byte()) { return None; }
    let end = parts.iter().find(|part| part.is_named() && !part.is_extra() && part.start_byte() >= operator.end_byte())?;
    let bracket = children(node).into_iter().find(|part| part.kind() == "[" && !part.is_missing())?;
    Some((bracket, ReadingContent::Detail {
        label: "Index from the beginning up to a bound".into(),
        text: format!("{}[..{}] passes an open-start, exclusive-end range to indexing: .. omits a start and the written end {} is excluded. If this resolves to standard array or slice indexing, it selects the slice from index 0 up to, but not including, {}; for ..4 that means indexes 0, 1, 2, and 3. The end may equal the length (including 0); an end beyond the length panics. The selected slice is a place, not an automatic owned copy: for example, &{}[..{}] borrows it, with the applicable borrowing rules. Other operand types can implement indexing differently, so the spelling alone cannot establish the operand type, indexing implementation, or result.", src.norm(value), src.norm(*end), src.norm(*end), src.norm(*end), src.norm(value), src.norm(*end)),
        definition: None,
        borrow: false,
    }))
}
