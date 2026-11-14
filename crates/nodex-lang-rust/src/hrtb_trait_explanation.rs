//! Explain higher-ranked trait bounds such as `for<'a> Fn(&'a T)`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Inspect a declaration's own header, never its body or nested declarations.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "trait_item" | "impl_item" | "type_item" | "associated_type") {
        return Vec::new();
    }
    let body = declaration.child_by_field_name("body");
    let mut pending = vec![declaration];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body { continue; }
        if node.kind() == "higher_ranked_trait_bound" {
            if let (Some(params), Some(bound)) = (node.child_by_field_name("type_parameters"), node.child_by_field_name("type")) {
                let names = src.norm(params);
                let bound_text = src.norm(bound);
                result.push(ContractItem {
                    group: "Higher-ranked trait bound".into(),
                    text: format!("In {}, for{} introduces lifetime names scoped to the following bound {}. The bound must hold for every applicable choice of those lifetimes, rather than one fixed lifetime chosen in advance. For example, if Fn is the standard callable trait, for<'a> Fn(&'a T) requires a callable that accepts an appropriate borrow regardless of how long that borrow lasts. This does not require the borrow to be 'static or keep any value alive forever; actual calls and any other bounds still have to satisfy Rust's borrowing rules. The spelling Fn alone does not prove which trait it resolves to.", src.norm(node), names, bound_text),
                    span: src.span(node),
                    type_info: None,
                });
            }
            // Bound is handled as a unit: a nested function-pointer binder is a separate syntax.
            continue;
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    result
}
