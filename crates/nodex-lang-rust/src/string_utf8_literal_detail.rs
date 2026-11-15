//! Explain ordinary UTF-8 string literals in flow and constant contracts.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "string_literal" || node.has_error() || !src.text(node).starts_with('"') { return None; }
    let opening = src.index.span(src.file, node.start_byte(), node.start_byte() + 1);
    Some((opening, "An ordinary quoted Rust string literal produces `&'static str`: a shared reference to UTF-8 text stored for the program's lifetime. The quotes delimit the source literal and are not part of the text; escapes such as `\\n` are interpreted when constructing its contents. UTF-8 encodes characters using one or more bytes, so byte length can differ from character count; for example, `é` uses two UTF-8 bytes. This is a borrowed string slice, not an owned, growable `String`; code can create an owned String when needed. This form does not automatically append a C NUL terminator.".into()))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "UTF-8 string literal".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if matches!(part.kind(), "token_tree" | "macro_invocation") { continue; }
        if let Some((span, text)) = describe(src, part) { items.push(ContractItem { group: "UTF-8 string literal".into(), text, span, type_info: None }); }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
