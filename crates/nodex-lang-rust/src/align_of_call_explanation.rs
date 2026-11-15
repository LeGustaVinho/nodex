//! Explain a standard alignment query without assuming path resolution.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "call_expression" || node.has_error() { return None; }
    let function = node.child_by_field_name("function")?;
    if function.kind() != "generic_function" { return None; }
    let path = function.child_by_field_name("function")?;
    let name: String = src.text(path).chars().filter(|ch| !ch.is_whitespace()).collect();
    if !matches!(name.as_str(), "std::mem::align_of" | "::std::mem::align_of" | "core::mem::align_of" | "::core::mem::align_of") { return None; }
    let generic = function.child_by_field_name("type_arguments")?;
    let args = named_children(generic);
    if args.len() != 1 { return None; }
    let call_args = node.child_by_field_name("arguments")?;
    if !named_children(call_args).is_empty() { return None; }
    Some((path, format!("If this path resolves to the standard `align_of` function, it returns the minimum alignment of type {} in bytes as a `usize`: an address holding a value of that type must be a multiple of this alignment. The result is a positive power of two, depends on the type and compilation target, and can be evaluated at compile time. It reports a type's alignment, not its size or the address of a particular value. The written path alone does not prove which function is resolved.", src.norm(args[0]))))
}

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let (site, text) = describe(src, node)?;
    Some((site, ReadingContent::Detail { label: "Query a type's alignment".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "function_item" | "closure_expression" | "async_block") { continue; }
        if let Some((site, text)) = describe(src, part) {
            result.push(ContractItem { group: "Query a type's alignment".into(), text, span: src.span(site), type_info: None });
            continue;
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
