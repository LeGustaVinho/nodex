//! Explain the `?Sized` bound on a type parameter or associated type.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "trait_item" | "impl_item" | "type_item" | "associated_type" | "struct_item" | "enum_item" | "union_item") {
        return Vec::new();
    }
    let body = declaration.child_by_field_name("body");
    let mut pending = vec![declaration];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body { continue; }
        if node.kind() == "removed_trait_bound" {
            let Some(trait_name) = named_children(node).into_iter().next() else { continue };
            let name = src.norm(trait_name);
            if name == "Sized" || name == "core::marker::Sized" || name == "std::marker::Sized" {
                result.push(ContractItem {
                    group: "Relaxed Sized bound".into(),
                    text: format!("{} uses ? to relax an implicit bound, not to ask whether the type is sized. If {} resolves to Rust's standard Sized trait, this permits the type parameter or associated type to be a dynamically sized type as well as an ordinary sized type. It does not require an unsized type. Such a value generally needs an appropriate pointer or reference when used where its size is not known at compile time. The written name alone does not prove name resolution; Rust restricts where ?Sized can be used.", src.norm(node), name),
                    span: src.span(node),
                    type_info: None,
                });
            }
            continue;
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    result
}
