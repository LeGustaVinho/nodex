//! Explain tuple types containing multiple positional elements.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if node.kind() != "tuple_type" { return None; }
    let elements = named_children(node);
    if elements.len() < 2 { return None; }
    let types = elements.iter().enumerate().map(|(i, ty)| format!(".{i}: {}", src.norm(*ty))).collect::<Vec<_>>().join(", ");
    Some(format!("{} is a tuple type: one value groups {} elements in a fixed order. Each position has its own type ({types}), so the elements can have different types. Access them using zero-based positions such as value.0 and value.1. The number, order and types of elements are part of the tuple's type; declaring this type does not itself create a value.", src.norm(node), elements.len()))
}

/// Inspect only a symbol's own declaration, never local statements or nested items.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_variant" | "type_item" | "const_item" | "static_item" | "field_declaration") { return Vec::new(); }
    let mut stack = vec![declaration];
    let mut items = Vec::new();
    while let Some(node) = stack.pop() {
        if node != declaration && matches!(node.kind(), "block" | "declaration_list" | "function_item" | "function_signature_item" | "impl_item" | "trait_item" | "closure_expression" | "token_tree") { continue; }
        if let Some(text) = describe(src, node) {
            items.push(ContractItem { group: "Tuple type".into(), text, span: src.span(node), type_info: None });
        }
        stack.extend(named_children(node));
    }
    items
}

/// Collect explicit local tuple annotations, including nested types, without scanning the initializer.
pub(crate) fn local<'a>(src: Src<'_>, declaration: Node<'a>) -> Vec<(Node<'a>, ReadingContent)> {
    if declaration.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = declaration.child_by_field_name("type") else { return Vec::new(); };
    let mut stack = vec![ty];
    let mut items = Vec::new();
    while let Some(node) = stack.pop() {
        if let Some(text) = describe(src, node) {
            items.push((node, ReadingContent::Detail { label: "Tuple type".into(), text, definition: None, borrow: false }));
        }
        stack.extend(named_children(node));
    }
    items
}
