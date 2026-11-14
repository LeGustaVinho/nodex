//! Explain `_` written in a type or generic-argument position.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn placeholders<'a>(src: Src<'_>, root: Node<'a>) -> Vec<Node<'a>> {
    let mut todo = vec![root];
    let mut result = Vec::new();
    while let Some(node) = todo.pop() {
        if node.kind() == "type_identifier" && src.text(node) == "_" { result.push(node); continue; }
        if matches!(node.kind(), "block" | "token_tree") { continue; }
        todo.extend(named_children(node).into_iter().rev());
    }
    result
}

fn explanation(generic: bool) -> &'static str {
    if generic {
        "The underscore asks the compiler to infer this generic argument from the surrounding context. Depending on the generic parameter it can stand for a type or an inferred constant, where Rust permits that form. It is not a variable named `_`; compilation fails if the compiler cannot determine a unique valid argument."
    } else {
        "The underscore asks the compiler to infer this type from the initializer and other constraints in this context. It is not a variable named `_`, nor a type whose value can be anything at runtime. Rust must determine a valid type at compile time, and does not allow this placeholder in every type position."
    }
}

fn detail(generic: bool) -> ReadingContent {
    ReadingContent::Detail { label: "Infer a type or generic argument".into(), text: explanation(generic).into(), definition: None, borrow: false }
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    placeholders(src, node).into_iter().map(|p| {
        let generic = p.parent().is_some_and(|n| n.kind() == "type_arguments");
        ContractItem { group: "Inferred type or argument".into(), text: explanation(generic).into(), span: src.span(p), type_info: None }
    }).collect()
}

pub(crate) fn local<'a>(src: Src<'_>, node: Node<'a>) -> Vec<(Node<'a>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new() };
    placeholders(src, ty).into_iter().map(|p| {
        let generic = p.parent().is_some_and(|n| n.kind() == "type_arguments");
        // A direct `let x: _` already has an annotation detail on `_`.
        let highlight = if p == ty { children(node).into_iter().find(|c| c.kind() == ":").unwrap_or(p) } else { p };
        (highlight, detail(generic))
    }).collect()
}

/// In a turbofish, `_` is an argument even when the generic definition is unavailable.
pub(crate) fn generic_arguments<'a>(src: Src<'_>, node: Node<'a>) -> Vec<(Node<'a>, ReadingContent)> {
    if node.kind() != "generic_function" { return Vec::new(); }
    let Some(args) = node.child_by_field_name("type_arguments") else { return Vec::new() };
    placeholders(src, args).into_iter().map(|p| (p, detail(true))).collect()
}
