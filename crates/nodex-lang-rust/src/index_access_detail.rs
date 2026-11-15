//! Explain indexing by a non-range expression without guessing the operand type.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "index_expression" || node.has_error() { return None; }
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    let [base, index] = parts.as_slice() else { return None; };
    if index.kind() == "range_expression" { return None; }
    let bracket = children(node).into_iter().find(|part| part.kind() == "[")?;
    Some((bracket, ReadingContent::Detail {
        label: "Index a value".into(),
        text: format!("{}[{}] uses the value of {} as an index into {}. If the operand is an array, slice or standard Vec indexed by usize, positions start at zero and Rust checks that the index is less than the length; an out-of-bounds access panics. Other types can implement Index or IndexMut with different indexing behavior, so this syntax alone does not prove the operand type, output or bounds policy. A write through an index needs mutable access and, where applicable, IndexMut. Whether a read copies, borrows or can move a value depends on its type and context.", src.norm(*base), src.norm(*index), src.norm(*index), src.norm(*base)),
        definition: None,
        borrow: false,
    }))
}
