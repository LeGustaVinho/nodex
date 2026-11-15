//! Explain a wildcard used as an entire function parameter pattern.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, function: Node<'_>) -> Vec<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") || function.has_error() { return Vec::new(); }
    let Some(parameters) = function.child_by_field_name("parameters") else { return Vec::new(); };
    let mut items = Vec::new();
    for parameter in named_children(parameters) {
        let Some(pattern) = parameter.child_by_field_name("pattern") else { continue; };
        if pattern.kind() != "_" { continue; }
        let Some(ty) = parameter.child_by_field_name("type") else { continue; };
        items.push(ContractItem {
            group: "Ignored function parameter".into(),
            text: format!("The caller still supplies an argument of type `{}` and its expression is evaluated as usual. The `_` parameter pattern accepts that value without giving it a name, so the function body cannot refer to it through this parameter. This wildcard does not bind, copy, move, or borrow the value by itself; normal argument passing and ownership rules still apply. A name such as `_value` is different: it creates a binding even though it starts with an underscore.", src.norm(ty)),
            span: src.span(pattern), type_info: None,
        });
    }
    items
}
