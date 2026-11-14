//! Beginner-facing explanation of shared reference types.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn is_shared(node: Node<'_>) -> bool {
    node.kind() == "reference_type" && !children(node).iter().any(|child| child.kind() == "mutable_specifier")
}

fn explanation(src: Src<'_>, node: Node<'_>) -> String {
    let target = node.child_by_field_name("type").map(|ty| src.norm(ty)).unwrap_or_else(|| "T".into());
    format!("&{target} is a shared reference: it borrows a value of type {target} instead of taking ownership. Several shared references can coexist. The reference must remain valid for its lifetime; while it is used, ordinary mutation of the borrowed data is restricted, though types with interior mutability can permit controlled changes. A reference does not by itself copy the underlying value.")
}

fn occurrences<'t>(root: Node<'t>, body: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body || node.kind() == "token_tree" { continue; }
        if is_shared(node) { found.push(node); }
        pending.extend(named_children(node).into_iter().rev());
    }
    found
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    occurrences(node, node.child_by_field_name("body")).into_iter().map(|ty| ContractItem {
        group: "Shared reference type".into(), text: explanation(src, ty), span: src.span(ty), type_info: None,
    }).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(ty, None).into_iter().map(|reference| {
        let site = if reference == ty {
            children(node).into_iter().find(|child| child.kind() == ":").unwrap_or(reference)
        } else { reference };
        (site, ReadingContent::Detail { label: "Shared reference type".into(), text: explanation(src, reference), definition: None, borrow: false })
    }).collect()
}
