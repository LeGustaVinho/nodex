//! Explain `include_str!` without assuming the macro's resolved identity.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let path = node.child_by_field_name("macro")?;
    let name: String = src.text(path).chars().filter(|character| !character.is_whitespace()).collect();
    if !matches!(name.as_str(), "include_str" | "std::include_str" | "::std::include_str" | "core::include_str" | "::core::include_str") { return None; }
    Some((path, "If this invocation resolves to Rust's standard `include_str!` macro, its argument supplies a path to a UTF-8 file read while compiling. A relative path is resolved relative to the source file containing the invocation, using the build platform's path conventions. The macro expands to the file contents as an `&'static str`; the running program uses the embedded text and does not reopen that file for this expression. An unreadable file or invalid UTF-8 causes compilation to fail. A macro with the same name may be shadowed or resolve differently, so the spelling alone does not prove which macro is called.".into()))
}

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let (site, text) = describe(src, node)?;
    Some((site, ReadingContent::Detail { label: "Include a text file at compile time".into(), text, definition: None, borrow: false }))
}

/// Const and static initializers can have no ordinary executable flow node.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "function_item" | "closure_expression" | "async_block") { continue; }
        if let Some((site, text)) = describe(src, part) {
            items.push(ContractItem { group: "Include a text file at compile time".into(), text, span: src.span(site), type_info: None });
            continue;
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
