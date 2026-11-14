//! Explain fixed-size array types in declarations and local annotations.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn description(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if node.kind() != "array_type" { return None; }
    let element = node.child_by_field_name("element")?;
    let length = node.child_by_field_name("length")?;
    Some(format!("{} is a fixed-size array type: its elements have type {} and its length is the constant expression {}. The semicolon separates the element type from the length; it is not a statement terminator here. The length is part of the type, so arrays of different lengths have different types. Elements are stored contiguously and positions start at index 0. This type annotation describes values; it does not itself create an array.", src.norm(node), src.norm(element), src.norm(length)))
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_variant" | "type_item" | "const_item" | "static_item" | "field_declaration") { return Vec::new(); }
    let mut pending = vec![declaration];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if node != declaration && matches!(node.kind(), "block" | "declaration_list" | "function_item" | "function_signature_item" | "impl_item" | "trait_item" | "closure_expression" | "token_tree") { continue; }
        if let Some(text) = description(src, node) {
            result.push(ContractItem { group: "Fixed-size array type".into(), text, span: src.span(node), type_info: None });
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    result
}

/// Return each array annotation on a let without scanning its initializer.
pub(crate) fn local<'a>(src: Src<'_>, node: Node<'a>) -> Vec<(Node<'a>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new() };
    let mut pending = vec![ty];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if let Some(text) = description(src, part) {
            // The generic explicit-type explanation highlights the whole direct type.
            // Use the colon to preserve both facts; nested array types have their own span.
            let highlight = if part == ty { children(node).into_iter().find(|child| child.kind() == ":").unwrap_or(part) } else { part };
            result.push((highlight, ReadingContent::Detail { label: "Fixed-size array type".into(), text, definition: None, borrow: false }));
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
