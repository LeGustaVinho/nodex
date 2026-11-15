//! Explain an ordinary character literal as one Unicode scalar value.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if node.kind() != "char_literal" || node.has_error() || !src.text(node).starts_with('\'') { return None; }
    Some(format!("`{}` is a Rust `char` literal: it represents one Unicode scalar value, stored as a four-byte `char`. Single quotes delimit the character; escapes can represent a scalar without writing it directly. A Unicode scalar excludes surrogate code points. One visible character can contain multiple scalars, so `char` is not necessarily one user-perceived character. A byte literal has a `b` prefix and represents a `u8` instead; a string literal uses double quotes and can contain many scalars.", src.text(node)))
}

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let text = describe(src, node)?;
    Some((node, ReadingContent::Detail { label: "Unicode scalar character literal".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "function_item" | "closure_expression" | "async_block") { continue; }
        if let Some(text) = describe(src, part) { result.push(ContractItem { group: "Unicode scalar character literal".into(), text, span: src.span(part), type_info: None }); continue; }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
