//! Explain a written negative trait implementation without treating it as positive.
//! Rust Unstable Book: https://doc.rust-lang.org/unstable-book/language-features/negative-impls.html
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn item(src: Src<'_>, implementation: Node<'_>) -> Option<ContractItem> {
    if implementation.kind() != "impl_item" || implementation.has_error() { return None; }
    let negative = children(implementation).into_iter().find(|part| part.kind() == "!")?;
    let trait_name = implementation.child_by_field_name("trait")?;
    let target = implementation.child_by_field_name("type")?;
    Some(ContractItem {
        group: "Negative trait implementation".into(),
        text: format!("`impl !{} for {}` declares that the written target type does not implement the named trait; `!` reverses the meaning of a positive trait implementation. Negative implementations cannot contain associated items or overlap positive implementations, and must satisfy coherence and orphan rules. For an auto trait, a valid negative impl also opts out of automatic implementation. This is an unstable `negative_impls` feature requiring a supporting compiler and feature configuration. The written names must be resolved and checked by Rust; this display does not establish that the declaration compiles or that the feature is enabled.", src.norm(trait_name), src.norm(target)),
        span: src.span(negative), type_info: None,
    })
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "impl_item" { return item(src, node).into_iter().collect(); }
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        "struct_item" | "enum_item" | "union_item" => node.parent(),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let target_name = if matches!(node.kind(), "struct_item" | "enum_item" | "union_item") {
        node.child_by_field_name("name").map(|name| src.norm(name))
    } else { None };
    named_children(container).into_iter().filter_map(|implementation| {
        if let Some(target_name) = &target_name {
            let target = implementation.child_by_field_name("type")?;
            if src.norm(target) != *target_name { return None; }
            if implementation.child_by_field_name("type_parameters").is_some_and(|params| named_children(params).iter().any(|param| param.child_by_field_name("name").is_some_and(|name| src.norm(name) == *target_name))) { return None; }
        }
        item(src, implementation)
    }).collect()
}
