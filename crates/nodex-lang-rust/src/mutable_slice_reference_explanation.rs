//! Explain exclusive mutable references to dynamically sized slices.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

const EXPLANATION: &str = "&mut [T] is an exclusive mutable borrow of a contiguous sequence of T values stored elsewhere. The reference carries the number of elements; the slice type itself has no fixed length. The borrower may read or change elements but does not take ownership of the sequence or resize it through this slice. While this borrow is active, other access to the same data is restricted; it must not outlive the data. For example, &mut [i32] refers to signed 32-bit integer elements when i32 resolves to Rust's primitive type.";

fn mutable_slice(node: Node<'_>) -> bool {
    node.kind() == "reference_type"
        && children(node).iter().any(|child| child.kind() == "mutable_specifier")
        && node.child_by_field_name("type").is_some_and(|ty| ty.kind() == "array_type" && ty.child_by_field_name("length").is_none())
}

fn occurrences<'t>(root: Node<'t>, skip_body: bool) -> Vec<Node<'t>> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body { continue; }
        if mutable_slice(node) { found.push(node); }
        pending.extend(named_children(node));
    }
    found.sort_by_key(|node| node.start_byte());
    found
}

/// Explain written occurrences belonging to a symbol's declaration.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    occurrences(node, true).into_iter().map(|ty| ContractItem {
        group: "Mutable borrowed slice".into(), text: EXPLANATION.into(), span: src.span(ty), type_info: None,
    }).collect()
}

/// Explain an explicit local type on its flow node, highlighting `mut` so that
/// the slice and general annotation explanations can remain visible too.
pub(crate) fn local<'t>(_src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new() };
    occurrences(ty, false).into_iter().map(|site| {
        let highlight = children(site).into_iter().find(|child| child.kind() == "mutable_specifier").unwrap_or(site);
        (highlight, ReadingContent::Detail { label: "Mutable borrowed slice".into(), text: EXPLANATION.into(), definition: None, borrow: false })
    }).collect()
}
