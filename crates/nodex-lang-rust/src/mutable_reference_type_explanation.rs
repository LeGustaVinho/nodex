//! Explanation of an exclusive, mutable reference type.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

const TEXT: &str = "&mut T is an exclusive reference to a value of type T. It borrows the value rather than taking ownership of it, and permits mutation through this reference. While the borrow is active, other access to the borrowed place is restricted; Rust checks the lifetime and aliasing rules. Reborrowing through this reference is possible, but the original reference is restricted while the reborrow is active.";

fn mutable_reference(node: Node<'_>) -> bool {
    node.kind() == "reference_type" && children(node).iter().any(|part| part.kind() == "mutable_specifier")
}

fn references<'t>(root: Node<'t>, skip_body: bool) -> Vec<Node<'t>> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut pending = vec![root];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body { continue; }
        if mutable_reference(node) { result.push(node); }
        pending.extend(named_children(node));
    }
    result.sort_by_key(|node| node.start_byte());
    result
}

/// Explain written reference types in a symbol's declaration, not its body.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    references(node, true).into_iter().map(|ty| ContractItem {
        group: "Exclusive reference".into(), text: TEXT.into(), span: src.span(ty), type_info: None,
    }).collect()
}

/// Attach explanations for local type annotations to their binding's flow node.
pub(crate) fn local<'t>(node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    references(ty, false).into_iter().map(|site| {
        let highlight = children(site).into_iter().find(|part| part.kind() == "mutable_specifier").unwrap_or(site);
        (highlight, ReadingContent::Detail { label: "Exclusive reference".into(), text: TEXT.into(), definition: None, borrow: false })
    }).collect()
}
