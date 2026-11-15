//! Explain a higher-ranked lifetime binder before a borrowed where-clause type.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" | "impl_item") { return Vec::new(); }
    let mut result = Vec::new();
    for clause in named_children(declaration).into_iter().filter(|n| n.kind() == "where_clause") {
        for predicate in named_children(clause).into_iter().filter(|n| n.kind() == "where_predicate" && !n.has_error()) {
            let Some(binder) = predicate.child_by_field_name("left").filter(|n| n.kind() == "higher_ranked_trait_bound") else { continue; };
            let Some(params) = binder.child_by_field_name("type_parameters") else { continue; };
            let Some(target) = binder.child_by_field_name("type").filter(|n| n.kind() == "reference_type") else { continue; };
            let Some(lifetime) = named_children(target).into_iter().find(|n| n.kind() == "lifetime") else { continue; };
            if !named_children(params).iter().any(|p| p.kind() == "lifetime_parameter" && p.child_by_field_name("name").is_some_and(|n| src.text(n) == src.text(lifetime))) { continue; }
            let Some(bounds) = predicate.child_by_field_name("bounds") else { continue; };
            let requirements = src.norm(bounds);
            let requirements = requirements.trim_start_matches(':').trim();
            if requirements.is_empty() { continue; }
            result.push(ContractItem {
                group: "Higher-ranked borrowed type in where clause".into(),
                text: format!("In {}, `for{}` introduces lifetime names scoped to this where-clause predicate. For every applicable choice of {}, the borrowed reference type {} must satisfy {}. The requirement is on the reference, not automatically on the underlying type itself. It describes a generic constraint checked by Rust; it does not create or extend a borrow, require a reference to live forever, or prove which trait a written name resolves to. Other bounds and borrowing rules still apply.", src.norm(predicate), src.norm(params), src.norm(lifetime), src.norm(target), requirements),
                span: src.span(predicate), type_info: None,
            });
        }
    }
    result
}
