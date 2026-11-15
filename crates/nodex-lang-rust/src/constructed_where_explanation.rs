//! Explain a where-clause bound on a constructed generic type.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let mut items = Vec::new();
    for clause in named_children(declaration).into_iter().filter(|part| part.kind() == "where_clause") {
        for predicate in named_children(clause).into_iter().filter(|part| part.kind() == "where_predicate") {
            let Some(target) = predicate.child_by_field_name("left") else { continue; };
            if target.kind() != "generic_type" || target.has_error() { continue; }
            let Some(bounds) = predicate.child_by_field_name("bounds") else { continue; };
            let written = src.norm(bounds);
            let requirements = written.trim_start_matches(':').trim();
            if requirements.is_empty() { continue; }
            items.push(ContractItem {
                group: "Constructed type requirement".into(),
                text: format!("The `where` clause requires the whole type `{}` to satisfy `{}` for this declaration to be usable; it places the bound on the constructed type, not directly on each type argument. For example, in `Vec<T>: Clone`, the requirement is that `Vec<T>` implements `Clone`. What that implies for `T` depends on the resolved types and trait implementations; the spelling alone does not prove them. If `Clone` is Rust's standard trait, it supports explicit duplication, but this clause itself neither calls `clone` nor creates a copy. The compiler checks the requirement along with any other bounds.", src.norm(target), requirements),
                span: src.span(predicate), type_info: None,
            });
        }
    }
    items
}
