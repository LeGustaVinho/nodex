//! Explanations of immutable-access raw pointer types.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

const TEXT: &str = "`*const T` is a raw pointer to a T: it stores an address, and this pointer type does not permit writing through it directly. Unlike a Rust reference, a raw pointer can be null, dangling, or misaligned and does not itself keep its target alive or establish a borrow. Creating or copying one can be safe; dereferencing it requires unsafe code and the programmer must ensure the address is valid, aligned, initialized, and used according to Rust's aliasing rules. Casting it to `*mut T` does not by itself make mutation legal. `const` here describes access through this pointer, not an immutable allocation.";

fn is_const_pointer(node: Node<'_>) -> bool {
    node.kind() == "pointer_type" && children(node).iter().any(|part| part.kind() == "const")
}

fn occurrences<'t>(root: Node<'t>, skip_body: bool) -> Vec<Node<'t>> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut nodes = vec![root];
    let mut found = Vec::new();
    while let Some(node) = nodes.pop() {
        if Some(node) == body { continue; }
        if is_const_pointer(node) { found.push(node); }
        nodes.extend(named_children(node));
    }
    found.sort_by_key(|n| n.start_byte());
    found
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    occurrences(node, true).into_iter().map(|pointer| ContractItem {
        group: "Raw const pointer".into(), text: TEXT.into(), span: src.span(pointer), type_info: None,
    }).collect()
}

pub(crate) fn local<'t>(node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(ty, false).into_iter().map(|pointer| {
        let highlight = children(pointer).into_iter().find(|part| part.kind() == "const").unwrap_or(pointer);
        (highlight, ReadingContent::Detail { label: "Raw const pointer".into(), text: TEXT.into(), definition: None, borrow: false })
    }).collect()
}
