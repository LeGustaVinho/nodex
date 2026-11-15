//! Explain an unreachable! invocation without presuming macro resolution.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = src.norm(node.child_by_field_name("macro")?);
    if name.rsplit("::").next()? != "unreachable" { return None; }
    let bang = children(node).into_iter().find(|part| part.kind() == "!")?;
    Some((src.span(bang), format!("{}! is a macro invocation; the spelling alone does not establish which macro it resolves to. If it resolves to Rust's standard unreachable! macro, it asserts that this execution path should not be reached and triggers a panic if execution reaches it. The panic may unwind or abort according to the program's panic configuration. This does not prove that the path is impossible, and it is different from an unchecked operation that assumes unreachability. During required constant evaluation, reaching the panic prevents successful evaluation. A user-defined macro with the same name can behave differently.", name)))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Conditional unreachable assertion".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = declaration.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == "token_tree" { continue; }
        if let Some((span, text)) = describe(src, node) {
            items.push(ContractItem { group: "Conditional unreachable assertion".into(), text, span, type_info: None });
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    items
}
