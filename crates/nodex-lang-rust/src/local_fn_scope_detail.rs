//! Explain named function items declared inside a function body.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Local function item".into(),
        text: format!("`fn {}` declares a named function item inside a block. Its name is available within the enclosing block's scope, and calling it executes its body; declaring it does not execute that body. A nested function item cannot capture local variables or parameters from the surrounding function: pass needed values as parameters, or use a closure when capturing is appropriate. Rust still checks name resolution and the function's signature at each call.", src.norm(name)),
        span: src.span(name), type_info: None,
    })
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" { return Vec::new(); }
    let mut result = Vec::new();
    if node.parent().is_some_and(|parent| parent.kind() == "block") { result.extend(describe(src, node)); }
    let Some(body) = node.child_by_field_name("body") else { return result; };
    let mut pending = vec![body];
    while let Some(part) = pending.pop() {
        if part.kind() == "function_item" { result.extend(describe(src, part)); continue; }
        if matches!(part.kind(), "mod_item" | "impl_item" | "trait_item" | "closure_expression" | "token_tree") { continue; }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
