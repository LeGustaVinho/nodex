//! Explain the never type wherever it is explicitly written in a declaration.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

const TEXT: &str = "! is the never type: it has no values. An expression of this type cannot finish normally with a value; it can, for example, loop forever, panic, or terminate the process. Because it never produces a value, it can be used where another type is expected. Unlike (), which has one value and can return normally, ! has no values. Some uses of ! outside return types depend on Rust language support and compilation context.";

/// Search only this declaration's written type information, not nested bodies.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "associated_type" | "const_item" | "static_item") {
        return Vec::new();
    }
    let body = node.child_by_field_name("body");
    let value = node.child_by_field_name("value");
    let mut pending = vec![node];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if Some(part) == body || Some(part) == value { continue; }
        if part.kind() == "never_type" {
            result.push(ContractItem { group: "Never type".into(), text: TEXT.into(), span: src.span(part), type_info: None });
        } else {
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    result
}

/// Explain explicit local annotations without inspecting their initializer.
pub(crate) fn local<'t>(node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new() };
    let mut pending = vec![ty];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if part.kind() == "never_type" {
            // The complete type is already used as the span for an existing explanation.
            let site = if part == ty { children(node).into_iter().find(|child| child.kind() == ":").unwrap_or(part) } else { part };
            result.push((site, ReadingContent::Detail { label: "Never type".into(), text: TEXT.into(), definition: None, borrow: false }));
        } else {
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    result
}
