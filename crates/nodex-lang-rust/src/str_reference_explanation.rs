//! Beginner-facing explanation of a shared reference to a UTF-8 string slice.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

const EXPLANATION: &str = "&str is a shared reference to a string slice: it lets code read UTF-8 text stored elsewhere without taking ownership of that text. The reference must not outlive the text it refers to. It points to a sequence of bytes, so byte positions need not correspond to characters and a slice must end at UTF-8 character boundaries. This is a borrowed view, rather than an owned, growable String.";

fn shared_str(src: Src<'_>, node: Node<'_>) -> bool {
    if node.kind() != "reference_type" || children(node).iter().any(|child| child.kind() == "mutable_specifier") { return false; }
    node.child_by_field_name("type").is_some_and(|ty| ty.kind() == "primitive_type" && src.text(ty) == "str")
}

fn occurrences<'t>(src: Src<'_>, root: Node<'t>, skip_body: bool) -> Vec<Node<'t>> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body { continue; }
        if shared_str(src, node) { found.push(node); }
        pending.extend(named_children(node));
    }
    found.sort_by_key(|node| node.start_byte());
    found
}

/// Explain explicit type annotations visible on the selected declaration.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item") { return Vec::new(); }
    occurrences(src, node, true).into_iter().map(|ty| ContractItem {
        group: "Borrowed string slice".into(), text: EXPLANATION.into(), span: src.span(ty), type_info: None,
    }).collect()
}

/// Explain the explicit type on a local variable in its Blueprint flow.
pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(src, ty, false).into_iter().map(|site| (children(site).into_iter().find(|child| child.kind() == "&").unwrap_or(site), ReadingContent::Detail {
        label: "Borrowed string slice".into(), text: EXPLANATION.into(), definition: None, borrow: false,
    })).collect()
}
