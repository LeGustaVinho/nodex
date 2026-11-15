//! Describe the standard matches! macro conditionally, without guessing name resolution.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let macro_name = node.child_by_field_name("macro")?;
    let name = src.norm(macro_name);
    if name.rsplit("::").next()? != "matches" { return None; }
    let bang = children(node).into_iter().find(|child| child.kind() == "!")?;
    let text = format!("{}! is a macro invocation. Its spelling alone does not prove which macro is called. If it resolves to Rust's standard matches! macro, the first expression is evaluated once and tested against the written pattern. The result is true when the pattern matches and any optional if guard passes, and false otherwise; the guard is checked only after a match. For example, Some(_) matches a present Option value if Some resolves to the standard Option variant; _ ignores the contained part. Pattern matching can move, copy, or borrow depending on the input and pattern. A custom macro with the same name can behave differently.", name);
    Some((src.span(bang), text))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Conditional pattern-test macro".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = declaration.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == "token_tree" { continue; }
        if let Some((span, text)) = describe(src, node) {
            items.push(ContractItem { group: "Conditional pattern-test macro".into(), text, span, type_info: None });
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    items
}
