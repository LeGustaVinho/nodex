//! Explain invocation of a macro through a qualified Rust path.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::{kit::truncate, syntax::{children, named_children, Src}};
use tree_sitter::Node;

fn details<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let path = node.child_by_field_name("macro")?;
    if path.kind() != "scoped_identifier" { return None; }
    let qualifier = path.child_by_field_name("path")?;
    let name = path.child_by_field_name("name")?;
    let marker = children(node).into_iter().find(|part| part.kind() == "!" && !part.is_missing())?;
    let tokens = named_children(node).into_iter().find(|part| part.kind() == "token_tree" && !part.has_error())?;
    Some((marker, format!("In {}!, the :: path selects the macro named {} through {}. Which macro is selected depends on Rust's path resolution, visibility and any imports or re-exports; this spelling alone does not establish its definition. The ! marks a macro invocation, and {} contains its input tokens. Rust expands the macro during compilation; it may generate code that runs later. The written input is not automatically a function argument evaluated once: the macro determines how its tokens are used, and may omit or repeat generated evaluation.", src.norm(path), src.norm(name), src.norm(qualifier), truncate(&src.norm(tokens), 80))))
}

/// Explain a qualified macro invocation within an executable flow.
pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let (marker, text) = details(src, node)?;
    Some((marker, ReadingContent::Detail { label: "Invoke a macro through a path".into(), text, definition: None, borrow: false }))
}

/// Direct module-level invocations have no ordinary flow node, so show them on
/// their containing module. Do not scan nested declarations or macro tokens.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter_map(|part| {
        let invocation = if part.kind() == "macro_invocation" { Some(part) }
            else if part.kind() == "expression_statement" { named_children(part).into_iter().find(|child| child.kind() == "macro_invocation") }
            else { None }?;
        let (marker, text) = details(src, invocation)?;
        Some(ContractItem { group: "Qualified macro invocation".into(), text, span: src.span(marker), type_info: None })
    }).collect()
}
