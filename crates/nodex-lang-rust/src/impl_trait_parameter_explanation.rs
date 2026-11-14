//! Explain `impl Trait` in function parameter types.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Report only occurrences in parameter types; return-position `impl Trait` has different rules.
pub(crate) fn contract(src: Src<'_>, function: Node<'_>) -> Vec<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(parameters) = function.child_by_field_name("parameters") else { return Vec::new(); };
    let mut items = Vec::new();
    for parameter in named_children(parameters) {
        let Some(ty) = parameter.child_by_field_name("type") else { continue; };
        let mut stack = vec![ty];
        while let Some(node) = stack.pop() {
            if node.kind() == "abstract_type" {
                let text = format!("In this parameter, `{}` means the caller supplies a concrete type that satisfies the listed trait requirements. This is shorthand for an anonymous generic type parameter, not a trait object: the concrete type is selected for the call and does not need to be written in the function declaration. Separate `impl Trait` parameters are independent; use a named generic parameter if they must have the same type. The exact traits depend on name resolution.", src.norm(node));
                items.push(ContractItem { group: "Generic parameter type".into(), text, span: src.span(node), type_info: None });
            }
            stack.extend(named_children(node).into_iter().rev());
        }
    }
    items
}
