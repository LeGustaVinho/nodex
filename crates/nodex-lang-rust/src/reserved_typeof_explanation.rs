//! Explain plain reserved `typeof` without treating strings or raw names as keywords.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

const TEXT: &str = "typeof is a reserved Rust keyword. In stable Rust it is not an available type-query operator and cannot be used as an ordinary unescaped identifier; code using it in that way is rejected by the Rust compiler even if a syntax parser recovers a tree. If the intent is an identifier literally named typeof, write r#typeof where raw identifiers are allowed. That raw name is an identifier, not a type-query operation. Depending on the goal, Rust can often infer a local variable's type without an explicit annotation.";

fn reserved(src: Src<'_>, node: Node<'_>) -> bool {
    matches!(node.kind(), "identifier" | "type_identifier" | "field_identifier" | "shorthand_field_identifier")
        && src.text(node) == "typeof"
        && !node.parent().is_some_and(|parent| matches!(parent.kind(), "lifetime" | "label" | "metavariable"))
}

/// Explain keyword occurrences in a declaration header on the owning symbol.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "trait_item" | "type_item" | "field_declaration" | "const_item" | "static_item") { return Vec::new(); }
    let body = declaration.child_by_field_name("body");
    let mut pending = vec![declaration];
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body || node.is_extra() || matches!(node.kind(), "token_tree" | "string_literal" | "raw_string_literal" | "char_literal") { continue; }
        if reserved(src, node) {
            items.push(ContractItem { group: "Reserved typeof keyword".into(), text: TEXT.into(), span: src.span(node), type_info: None });
        } else { pending.extend(named_children(node).into_iter().rev()); }
    }
    items
}

/// Explain a recovered plain identifier in a flow expression or local annotation.
pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    reserved(src, node).then(|| (node, ReadingContent::Detail { label: "Reserved typeof keyword".into(), text: TEXT.into(), definition: None, borrow: false }))
}
