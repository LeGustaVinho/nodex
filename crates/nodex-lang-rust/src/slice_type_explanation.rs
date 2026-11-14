//! Explain the unsized sequence type `[T]` in explicit type annotations.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn is_slice(node: Node<'_>) -> bool {
    node.kind() == "array_type" && node.child_by_field_name("element").is_some()
        && node.child_by_field_name("length").is_none()
}

fn occurrences(root: Node<'_>, skip_body: bool) -> Vec<Node<'_>> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut nodes = vec![root];
    let mut found = Vec::new();
    while let Some(node) = nodes.pop() {
        if Some(node) == body { continue; }
        if is_slice(node) { found.push(node); }
        nodes.extend(named_children(node).into_iter().rev());
    }
    found
}

fn description(src: Src<'_>, node: Node<'_>) -> String {
    let element = node.child_by_field_name("element").map(|element| src.norm(element)).unwrap_or_else(|| "T".into());
    format!("[{}] is a slice type: a contiguous sequence of elements of type {} whose length is not fixed in the type. Unlike [{}; N], it does not specify a number of elements. A slice is dynamically sized, so it is usually used behind a reference or pointer such as &[{}] or Box<[{}]>; those pointers carry the length needed to use the slice. The slice type itself does not say who owns the elements or whether they can be changed.", element, element, element, element, element)
}

/// Explain explicit occurrences on a symbol without walking into function bodies.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") {
        return Vec::new();
    }
    occurrences(node, true).into_iter().map(|slice| ContractItem {
        group: "Slice type".into(), text: description(src, slice), span: src.span(slice), type_info: None,
    }).collect()
}

/// Explain local type annotations, using the opening bracket as a distinct highlight.
pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(ty, false).into_iter().map(|slice| {
        let highlight = children(slice).into_iter().find(|child| child.kind() == "[").unwrap_or(slice);
        (highlight, ReadingContent::Detail { label: "Slice type".into(), text: description(src, slice), definition: None, borrow: false })
    }).collect()
}
