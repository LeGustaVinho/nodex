//! Explain creation of a shared reference in an expression.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Explain only `&value`, not mutable borrows or raw-pointer creation.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "reference_expression" || node.has_error() { return None; }
    let parts = children(node);
    if parts.iter().any(|part| matches!(part.kind(), "mutable_specifier" | "raw")) { return None; }
    let ampersand = parts.iter().find(|part| part.kind() == "&").copied()?;
    let value = node.child_by_field_name("value")?;
    Some((ampersand, ReadingContent::Detail {
        label: "Create a shared borrow".into(),
        text: format!("`&{}` creates a shared reference to the value instead of taking ownership of it. The owner keeps the value, and code can read it through the reference while that borrow is valid. Rust checks that the reference cannot outlive the data and restricts conflicting mutation during the borrow. Some types allow controlled changes through shared references (interior mutability).", src.norm(value)),
        definition: None,
        borrow: false,
    }))
}
