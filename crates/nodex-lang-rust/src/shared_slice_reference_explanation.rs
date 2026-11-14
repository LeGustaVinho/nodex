//! Explain a shared reference to a slice without claiming ownership of its elements.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn shared_slice(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() != "reference_type" || children(node).iter().any(|child| child.kind() == "mutable_specifier") {
        return None;
    }
    let slice = node.child_by_field_name("type")?;
    (slice.kind() == "array_type" && slice.child_by_field_name("length").is_none()).then_some(slice)
}

fn description(src: Src<'_>, slice: Node<'_>) -> String {
    let element = slice.child_by_field_name("element").map(|n| src.norm(n)).unwrap_or_else(|| "the element type".into());
    format!("This is a shared reference to a slice of {element}: a borrowed view of zero or more contiguous elements stored elsewhere. The slice length can vary at runtime; the reference carries access to the elements and their length. It does not own the elements, and it cannot outlive the data it borrows. Shared access normally allows reading but not mutation through this reference; types with interior mutability can have additional rules.")
}

fn occurrences<'t>(root: Node<'t>, skip_body: bool) -> Vec<(Node<'t>, Node<'t>)> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body { continue; }
        if let Some(slice) = shared_slice(node) { found.push((node, slice)); }
        pending.extend(named_children(node));
    }
    found.sort_by_key(|(node, _)| node.start_byte());
    found
}

/// Explain written reference types in the selected symbol's declaration.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item") { return Vec::new(); }
    occurrences(node, true).into_iter().map(|(reference, slice)| ContractItem {
        group: "Shared slice reference".into(), text: description(src, slice), span: src.span(reference), type_info: None,
    }).collect()
}

/// Attach explicit local types to their binding without scanning initializers.
pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(ty, false).into_iter().map(|(reference, slice)| {
        let anchor = children(reference).into_iter().find(|child| child.kind() == "&").unwrap_or(reference);
        (anchor, ReadingContent::Detail { label: "Shared slice reference".into(), text: description(src, slice), definition: None, borrow: false })
    }).collect()
}
