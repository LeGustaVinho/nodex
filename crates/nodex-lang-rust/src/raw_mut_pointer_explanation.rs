//! Explanations of mutable raw-pointer types in written type annotations.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

const TEXT: &str = "*mut T is a raw pointer whose pointee type is T. The mut means that this pointer can be used for writes when the operation is valid; it does not itself make the pointed-to memory writable or create an exclusive borrow. Unlike a reference, a raw pointer can be null or dangling and does not guarantee that its target remains alive. Creating or copying one is safe, but dereferencing it requires unsafe code and the programmer must ensure the pointer is valid, aligned, and used under Rust's aliasing rules. The pointer does not own or automatically free the target.";

fn occurrences<'t>(root: Node<'t>, skip_body: bool) -> Vec<Node<'t>> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body { continue; }
        if node.kind() == "pointer_type" && named_children(node).iter().any(|child| child.kind() == "mutable_specifier") {
            found.push(node);
        }
        pending.extend(named_children(node));
    }
    found.sort_by_key(|node| node.start_byte());
    found
}

/// Types appearing in a declaration's own header, including nested types.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") {
        return Vec::new();
    }
    occurrences(node, true).into_iter().map(|ty| ContractItem {
        group: "Mutable raw pointer".into(), text: TEXT.into(), span: src.span(ty), type_info: None,
    }).collect()
}

/// Types written on a local binding; highlight `mut` so existing annotation
/// and nested-type explanations can coexist at distinct spans.
pub(crate) fn local<'t>(node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(ty, false).into_iter().filter_map(|site| {
        let keyword = named_children(site).into_iter().find(|child| child.kind() == "mutable_specifier")?;
        Some((keyword, ReadingContent::Detail { label: "Mutable raw pointer".into(), text: TEXT.into(), definition: None, borrow: false }))
    }).collect()
}
