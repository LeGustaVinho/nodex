//! Explain written `String` type names without guessing their resolved definition.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn written_string(src: Src<'_>, node: Node<'_>) -> bool {
    if !matches!(node.kind(), "type_identifier" | "scoped_type_identifier") { return false; }
    if node.parent().is_some_and(|p| p.kind() == "scoped_type_identifier") { return false; }
    let spelling: String = src.text(node).chars().filter(|c| !c.is_whitespace()).collect();
    matches!(spelling.as_str(), "String" | "std::string::String" | "::std::string::String" | "alloc::string::String" | "::alloc::string::String")
}

fn description(src: Src<'_>, node: Node<'_>) -> String {
    format!("{} names a type. If it resolves to Rust's standard String, it is an owned, growable sequence of UTF-8 encoded text: the value manages its own storage and can be extended when mutable. Ownership can move to another binding or function; borrowing lets code access it without taking ownership. A different type can use the same written name, so its definition must be resolved before applying these properties.", src.norm(node))
}

/// Explain type occurrences in an item's signature, without visiting nested declarations or bodies.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let mut roots = Vec::new();
    if let Some(ty) = node.child_by_field_name("type") { roots.push(ty); }
    if let Some(ty) = node.child_by_field_name("return_type") { roots.push(ty); }
    if let Some(params) = node.child_by_field_name("parameters") {
        roots.extend(named_children(params).into_iter().filter_map(|p| p.child_by_field_name("type")));
    }
    let mut items = Vec::new();
    while let Some(current) = roots.pop() {
        if written_string(src, current) {
            items.push(ContractItem { group: "Text type".into(), text: description(src, current), span: src.span(current), type_info: None });
        } else {
            roots.extend(named_children(current));
        }
    }
    items
}

/// Explain an occurrence in a local type annotation in the flow.
pub(crate) fn local<'a>(src: Src<'_>, node: Node<'a>) -> Option<(Node<'a>, ReadingContent)> {
    if node.kind() != "let_declaration" { return None; }
    let mut pending = vec![node.child_by_field_name("type")?];
    while let Some(ty) = pending.pop() {
        if written_string(src, ty) {
            return Some((ty, ReadingContent::Detail { label: "Owned text type".into(), text: description(src, ty), definition: None, borrow: false }));
        }
        pending.extend(named_children(ty));
    }
    None
}
