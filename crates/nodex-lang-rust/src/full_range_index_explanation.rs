//! Explain indexing with a full range without assuming the operand's type.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "index_expression" || node.has_error() { return None; }
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    let [base, range] = parts.as_slice() else { return None; };
    if range.kind() != "range_expression" { return None; }
    let range_parts = children(*range);
    if range_parts.iter().any(|part| part.is_named() && !part.is_extra()) { return None; }
    let dots = range_parts.into_iter().find(|part| part.kind() == "..")?;
    Some((dots, ReadingContent::Detail {
        label: "Index with the full range".into(),
        text: format!("{}[..] indexes {} with the full, unbounded range: neither a start nor an end is written. If the operand is an array or slice using Rust's standard indexing, this designates a slice covering all its elements; indexing alone does not copy all of them or itself create a reference. Writing &{}[..] borrows the whole slice for reading, while &mut can borrow it for mutation when permitted. Other operand types may implement indexing differently, so the exact behavior depends on the resolved type and implementation.", src.norm(*base), src.norm(*base), src.norm(*base)),
        definition: None,
        borrow: false,
    }))
}
