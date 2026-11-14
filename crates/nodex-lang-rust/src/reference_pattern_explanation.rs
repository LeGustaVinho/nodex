//! Explain explicit reference patterns without treating them as borrow expressions.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "reference_pattern" || node.has_error() { return None; }
    let parts = children(node);
    if parts.iter().any(|part| part.kind() == "mutable_specifier") { return None; }
    let ampersand = parts.iter().copied().find(|part| part.kind() == "&")?;
    let inner = named_children(node).into_iter().find(|part| part.kind() != "mutable_specifier" && !part.is_extra())?;
    Some((ampersand, ReadingContent::Detail {
        label: "Match through a reference".into(),
        text: format!("In the pattern {}, `&` matches a shared reference layer already present in the value and matches the referenced value against {}. It does not create a new reference. In contrast, `ref x` binds x as a reference to the matched part, and `&value` in an expression creates a borrow. Bindings inside the pattern may copy or move according to their types and Rust's ownership rules; moving a non-Copy value out of a borrowed place is not allowed. The type checker verifies that the pattern fits the value.", src.norm(node), src.norm(inner)),
        definition: None,
        borrow: false,
    }))
}
