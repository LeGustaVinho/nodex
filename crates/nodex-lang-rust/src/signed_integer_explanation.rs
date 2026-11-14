//! Beginner-facing descriptions of fixed-width signed integer types.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn explain_type(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "primitive_type" { return None; }
    let (bits, min, max) = match src.text(node) {
        "i8" => (8, "-128", "127"),
        "i16" => (16, "-32,768", "32,767"),
        "i32" => (32, "-2,147,483,648", "2,147,483,647"),
        "i64" => (64, "-9,223,372,036,854,775,808", "9,223,372,036,854,775,807"),
        _ => return None,
    };
    Some(ContractItem {
        group: "Signed integer type".into(),
        text: format!("{} is a signed {bits}-bit integer type. It represents whole numbers from {min} through {max}, inclusive; it can hold negative values, zero, and positive values. The number after i is the number of bits, not a decimal digit count.", src.text(node)),
        span: src.span(node),
        type_info: None,
    })
}

/// Describe type syntax on this declaration. Skip executable bodies, initializers,
/// and nested declarations: their symbols and flow have their own explanations.
pub(crate) fn explain(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "const_item" | "static_item" | "field_declaration" | "type_item" | "associated_type" | "struct_item" | "union_item" | "enum_variant") {
        return Vec::new();
    }
    fn visit(src: Src<'_>, node: Node<'_>, out: &mut Vec<ContractItem>) {
        if let Some(item) = explain_type(src, node) { out.push(item); return; }
        if matches!(node.kind(), "block" | "declaration_list" | "attribute_item" | "inner_attribute_item") { return; }
        let value = node.child_by_field_name("value");
        for child in named_children(node) {
            if Some(child) != value { visit(src, child, out); }
        }
    }
    let mut out = Vec::new();
    visit(src, declaration, &mut out);
    out
}

/// For `let name: Type`, return each signed primitive inside the written type.
/// This intentionally does not descend into the initializer or into nested bodies.
pub(crate) fn local_types<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, String)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new() };
    fn walk<'t>(src: Src<'_>, node: Node<'t>, out: &mut Vec<(Node<'t>, String)>) {
        if let Some(item) = explain_type(src, node) { out.push((node, item.text)); return; }
        for child in named_children(node) { walk(src, child, out); }
    }
    let mut out = Vec::new();
    walk(src, ty, &mut out);
    out
}
