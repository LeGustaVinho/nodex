//! Explain half-open range indexing without assuming the receiver's type.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "index_expression" || node.has_error() { return None; }
    let operands = named_children(node);
    let [receiver, index] = operands.as_slice() else { return None; };
    let range = crate::syntax::unparen(*index);
    if range.kind() != "range_expression" { return None; }
    let parts = children(range);
    let operator = parts.iter().find(|part| part.kind() == "..")?;
    let start = parts.iter().find(|part| part.is_named() && part.end_byte() <= operator.start_byte())?;
    let end = parts.iter().find(|part| part.is_named() && part.start_byte() >= operator.end_byte())?;
    let bracket = children(node).into_iter().find(|part| part.kind() == "[")?;
    Some((bracket, ReadingContent::Detail {
        label: "Index with a half-open range".into(),
        text: format!("{} uses the range {} as an index into {}. The range starts at {} and stops before {}; the end is excluded. If the receiver is a standard array or slice and the bounds are valid, this selects the contiguous slice from the start index up to but not including the end index; invalid bounds panic. A slice is often accessed through `&` or `&mut`, subject to Rust's borrowing rules. Other types can define indexing behavior for this range, so the spelling alone does not prove the receiver is an array or what custom indexing returns.", src.norm(node), src.norm(range), src.norm(*receiver), src.norm(*start), src.norm(*end)),
        definition: None,
        borrow: false,
    }))
}
