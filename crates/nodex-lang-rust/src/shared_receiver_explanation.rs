//! Beginner-facing explanation of a shared method receiver.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, function: Node<'_>) -> Option<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return None; }
    let parameter = named_children(function.child_by_field_name("parameters")?).into_iter()
        .find(|p| p.kind() == "self_parameter")?;
    let written = src.norm(parameter);
    if !written.starts_with('&') || children(parameter).iter().any(|child| child.kind() == "mutable_specifier") { return None; }
    Some(ContractItem {
        group: "Shared receiver".into(),
        text: "&self means this method receives a shared reference to the value it was called on. The caller keeps ownership; the method borrows that value for access rather than taking it by value. Ordinary fields cannot be changed through this shared reference, although types with interior mutability can permit controlled changes. The reference remains valid only for its borrow's lifetime; multiple shared references can coexist while Rust's borrowing rules are satisfied.".into(),
        span: src.span(parameter),
        type_info: None,
    })
}
