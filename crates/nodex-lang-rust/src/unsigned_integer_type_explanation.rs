//! Explanations for Rust's fixed-width unsigned integer type names.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Explain occurrences in the declaration visible on this symbol, not function bodies or initializers.
pub(crate) fn explain(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "union_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    let mut pending = vec![declaration];
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if node != declaration && (Some(node) == declaration.child_by_field_name("value") ||
            (matches!(declaration.kind(), "function_item" | "function_signature_item") && Some(node) == declaration.child_by_field_name("body"))) { continue; }
        if node.kind() == "primitive_type" {
            if let Some(text) = description(src.text(node)) {
                items.push(ContractItem { group: "Unsigned integer type".into(), text, span: src.span(node), type_info: None });
            }
            continue;
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    items
}

fn description(name: &str) -> Option<String> {
    let (bits, max) = match name {
        "u8" => (8, "255"), "u16" => (16, "65,535"),
        "u32" => (32, "4,294,967,295"), "u64" => (64, "18,446,744,073,709,551,615"),
        _ => return None,
    };
    Some(format!("When `{name}` refers to Rust's built-in type, it is an unsigned {bits}-bit integer: it stores whole numbers from 0 through {max}, inclusive. The `u` means unsigned (no negative values), and {bits} is the number of bits. Arithmetic does not represent fractions, and overflow behavior depends on the operation and build settings. A user-defined type can shadow this name, so the spelling alone does not prove its meaning."))
}

/// Explain each unsigned type occurrence in an explicit local `let` annotation.
pub(crate) fn local<'t>(src: Src<'_>, declaration: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if declaration.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = declaration.child_by_field_name("type") else { return Vec::new() };
    let mut pending = vec![ty];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == "primitive_type" {
            if let Some(text) = description(src.text(node)) {
                result.push((node, ReadingContent::Detail { label: "Unsigned integer type".into(), text, definition: None, borrow: false }));
            }
        } else {
            pending.extend(named_children(node).into_iter().rev());
        }
    }
    result
}
