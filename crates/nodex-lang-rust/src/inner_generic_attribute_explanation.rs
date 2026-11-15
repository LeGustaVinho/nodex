//! Explain direct inner attributes on their enclosing scope.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::{kit::truncate, syntax::{named_children, Src}};
use tree_sitter::Node;

fn text(src: Src<'_>, attr: Node<'_>) -> String {
    format!("{} is an inner attribute: `#![...]` applies metadata or configuration to the enclosing scope, not to the next item. The meaning depends on the attribute and compiler or tool processing it; for example, an inner lint attribute can configure diagnostics in that scope. Its placement and accepted forms are checked by Rust. This declaration is not an instruction executed as program code. In contrast, an outer `#[...]` attribute applies to the item that follows it.", truncate(&src.norm(attr), 160))
}

fn scope(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "source_file" => Some(node),
        "mod_item" | "trait_item" | "impl_item" | "function_item" => node.child_by_field_name("body"),
        _ => None,
    }
}

/// Each fact belongs to the item whose body directly contains the attribute.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let Some(container) = scope(node) else { return Vec::new() };
    named_children(container).into_iter().filter(|attr| attr.kind() == "inner_attribute_item" && !attr.has_error()).map(|attr| ContractItem {
        group: "Inner attribute".into(), text: text(src, attr), span: src.span(attr), type_info: None,
    }).collect()
}

/// Detail for an inner attribute directly in a block used inside a flow.
pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "block" { return Vec::new(); }
    let parts = named_children(node);
    let anchor = parts.iter().copied().find(|part| part.kind() != "inner_attribute_item" && !part.is_extra()).unwrap_or(node);
    parts.into_iter().filter(|attr| attr.kind() == "inner_attribute_item" && !attr.has_error()).map(|attr| (anchor, attr, ReadingContent::Detail {
        label: "Attribute on the enclosing block".into(), text: text(src, attr), definition: None, borrow: false,
    })).collect()
}
