//! Explain the written `include_bytes!("...")` form without assuming macro identity.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn details<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let path = node.child_by_field_name("macro")?;
    let written_path = src.norm(path);
    if written_path.rsplit("::").next()? != "include_bytes" { return None; }
    let tree = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    let parts: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")" | "[" | "]" | "{" | "}")).collect();
    let literal = match parts.as_slice() {
        [literal] => *literal,
        [literal, comma] if comma.kind() == "," => *literal,
        _ => return None,
    };
    let text = src.text(literal);
    if !(literal.kind() == "string_literal" && text.starts_with('"')
        || literal.kind() == "raw_string_literal" && text.starts_with('r')) { return None; }
    let highlight = if path.kind() == "scoped_identifier" { path.child_by_field_name("name").unwrap_or(path) } else { path };
    Some((highlight, format!("If `{written_path}!` resolves to Rust's standard include_bytes! macro, {} names a file whose contents are read during compilation and embedded as bytes in the compiled program. A relative path is interpreted relative to the source file containing this invocation, using the compilation platform's path syntax. The resulting value is a shared static reference to a byte array (`&'static [u8; N]`), where N is the file's byte length; it does not decode the file as text. Including those bytes does not read the file when the program later runs. The file must be available to the compiler, and the written name alone does not prove which macro it resolves to.", src.norm(literal))))
}

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let (span, text) = details(src, node)?;
    Some((span, ReadingContent::Detail { label: "Embed file bytes at compile time".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
        if let Some((span, text)) = details(src, part) {
            items.push(ContractItem { group: "Embed file bytes at compile time".into(), text, span: src.span(span), type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
