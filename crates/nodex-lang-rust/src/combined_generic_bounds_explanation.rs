//! Explain multiple trait bounds on a generic type parameter.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" | "impl_item") { return Vec::new(); }
    let owner = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "impl_item");
    let mut result = Vec::new();
    for declaration in owner.into_iter().chain(std::iter::once(node)) {
        for list in named_children(declaration).into_iter().filter(|n| matches!(n.kind(), "type_parameters" | "where_clause")) {
            for predicate in named_children(list) {
                let Some(target) = predicate.child_by_field_name("name").or_else(|| predicate.child_by_field_name("left")) else { continue };
                let Some(bounds) = predicate.child_by_field_name("bounds") else { continue };
                if target.kind() == "lifetime" || !children(bounds).iter().any(|child| child.kind() == "+") { continue; }
                // `?Sized` relaxes a default requirement; it is not an additional requirement.
                // Leave such mixed bounds to a separate explanation rather than misstate them.
                if src.text(bounds).contains('?') { continue; }
                let requirements = src.norm(bounds);
                result.push(ContractItem {
                    group: "Combined generic bounds".into(),
                    text: format!("In {}{}, the colon introduces constraints on the type {}. The + joins requirements: the same type must satisfy every listed bound, not just one. Trait bounds let the code use the behavior those traits promise; a lifetime bound instead requires references inside the type to remain valid for the stated lifetime. These are compile-time requirements on the type, not values added together. The meaning of each named bound depends on name resolution.", src.norm(target), requirements, src.norm(target)),
                    span: src.span(bounds), type_info: None,
                });
            }
        }
    }
    result
}
