//! Explain the `&mut value` expression without treating raw pointers as references.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Return the `mut` keyword as the highlight, so the detail can coexist with
/// the enclosing binding and other explanations on the operand.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "reference_expression" || node.has_error() { return None; }
    let parts = children(node);
    if parts.iter().any(|part| part.kind() == "raw") { return None; }
    let keyword = parts.iter().copied().find(|part| part.kind() == "mutable_specifier")?;
    let value = node.child_by_field_name("value")?;
    Some((keyword, ReadingContent::Detail {
        label: "Borrow with exclusive mutable access".into(),
        text: format!("`&mut {}` creates a mutable reference to the selected value; it borrows the value rather than transferring ownership. The reference permits changes through it. While this exclusive borrow is active, other accesses to the same data are restricted; a shorter reborrow can temporarily use it. The reference must remain valid for its use, and the original owner remains responsible for the value.", src.norm(value)),
        definition: None,
        borrow: false,
    }))
}
