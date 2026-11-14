//! Explain lifetime bounds on types, not lifetime-to-lifetime bounds.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Describe bounds written on this declaration's own type parameters or where-clause.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" | "impl_item") {
        return Vec::new();
    }
    let mut roots = Vec::new();
    if let Some(parameters) = declaration.child_by_field_name("type_parameters") { roots.push(parameters); }
    roots.extend(named_children(declaration).into_iter().filter(|child| child.kind() == "where_clause"));
    let mut result = Vec::new();
    let mut pending = roots;
    while let Some(node) = pending.pop() {
        let target = match node.kind() {
            "type_parameter" => node.child_by_field_name("name"),
            "where_predicate" => node.child_by_field_name("left").filter(|left| left.kind() != "lifetime"),
            _ => None,
        };
        if let (Some(target), Some(bounds)) = (target, node.child_by_field_name("bounds")) {
            for lifetime in named_children(bounds).into_iter().filter(|bound| bound.kind() == "lifetime") {
                let ty = src.norm(target);
                let lt = src.norm(lifetime);
                result.push(ContractItem {
                    group: "Type lifetime requirement".into(),
                    text: format!("{ty}: {lt} requires the type {ty} to outlive {lt}: any references contained in a value of {ty} must be valid for at least {lt}. This is a constraint on the type and its borrowed data, not a promise that a particular value stays alive that long; the value can be dropped sooner. A type with no non-static borrowed data may satisfy this requirement too."),
                    span: src.span(lifetime), type_info: None,
                });
            }
            // Bounds were handled above; do not treat nested syntax as another predicate.
            continue;
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    result
}
