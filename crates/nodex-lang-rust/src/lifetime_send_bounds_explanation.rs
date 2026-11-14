//! Explain a type required to satisfy both an outlives and a Send bound.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" | "impl_item") { return Vec::new(); }
    let mut roots = Vec::new();
    if let Some(parameters) = declaration.child_by_field_name("type_parameters") { roots.push(parameters); }
    roots.extend(named_children(declaration).into_iter().filter(|n| n.kind() == "where_clause"));
    let mut result = Vec::new();
    for root in roots {
        for predicate in named_children(root) {
            let Some(target) = predicate.child_by_field_name("name").or_else(|| predicate.child_by_field_name("left")) else { continue };
            if target.kind() == "lifetime" { continue; }
            let Some(bounds) = predicate.child_by_field_name("bounds") else { continue };
            let parts = named_children(bounds);
            let Some(lifetime) = parts.iter().find(|n| n.kind() == "lifetime") else { continue };
            let Some(send) = parts.iter().find(|n| src.norm(**n) == "Send") else { continue };
            let ty = src.norm(target);
            let lt = src.norm(*lifetime);
            result.push(ContractItem {
                group: "Lifetime and Send requirements".into(),
                text: format!("The + combines requirements on {ty}: it must satisfy both {lt} and Send. The {ty}: {lt} requirement constrains any borrowed data inside the type to remain valid for at least {lt}; a particular value may still be dropped earlier. If Send resolves to Rust's standard Send trait, values of {ty} may be transferred between threads when the other rules permit it. The spelling Send alone does not establish which trait it names; neither bound starts a thread or extends a value's life."),
                span: src.span(bounds),
                type_info: None,
            });
            let _ = send;
        }
    }
    result
}
