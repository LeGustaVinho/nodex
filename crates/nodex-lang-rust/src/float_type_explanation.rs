//! Beginner-facing explanations for written floating-point primitive types.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn description(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if node.kind() != "primitive_type" { return None; }
    let (name, digits) = match src.text(node) {
        "f32" => ("f32", "about 6–9"),
        "f64" => ("f64", "about 15–17"),
        _ => return None,
    };
    Some(format!("{name} is a {}-bit floating-point type for approximate real-number calculations. It has {digits} significant decimal digits of precision, not a fixed number of digits after the decimal point. Many decimal fractions cannot be represented exactly, so arithmetic can round; it also supports positive and negative infinity and NaN. Rust does not implicitly convert between f32 and f64.", if name == "f32" { 32 } else { 64 }))
}

/// Collect occurrences in a declaration's own signature and fields, excluding bodies
/// whose statements or nested items belong to a separate flow or symbol.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_variant" | "type_item" | "const_item" | "static_item" | "field_declaration") { return Vec::new(); }
    let mut pending = vec![declaration];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if node != declaration && matches!(node.kind(), "block" | "declaration_list" | "function_item" | "function_signature_item" | "impl_item" | "trait_item" | "closure_expression") { continue; }
        if let Some(text) = description(src, node) {
            result.push(ContractItem { group: "Floating-point type".into(), text, span: src.span(node), type_info: None });
        }
        pending.extend(named_children(node));
    }
    result
}

/// A local type annotation is displayed as part of its flow.
pub(crate) fn local<'a>(src: Src<'_>, node: Node<'a>) -> Option<(Node<'a>, ReadingContent)> {
    let text = description(src, node)?;
    let declaration = std::iter::successors(node.parent(), |parent| parent.parent())
        .find(|ancestor| matches!(ancestor.kind(), "let_declaration" | "function_item" | "closure_expression" | "const_item" | "static_item"))
        .filter(|ancestor| ancestor.kind() == "let_declaration")?;
    // The existing annotation detail highlights the whole direct type. Use the
    // declaration in that case so both details survive FlowGraph deduplication.
    let highlight = if declaration.child_by_field_name("type") == Some(node) { declaration } else { node };
    Some((highlight, ReadingContent::Detail { label: "Floating-point type".into(), text, definition: None, borrow: false }))
}
