//! Explain an inline `Clone` bound on a named generic function parameter.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, function: Node<'_>) -> Vec<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(generics) = function.child_by_field_name("type_parameters") else { return Vec::new(); };
    let mut output = Vec::new();
    for parameter in named_children(generics) {
        if parameter.kind() != "type_parameter" { continue; }
        let Some(name) = parameter.child_by_field_name("name") else { continue; };
        let Some(bounds) = parameter.child_by_field_name("bounds") else { continue; };
        let mut todo = named_children(bounds);
        while let Some(bound) = todo.pop() {
            if bound.kind() == "type_identifier" && src.norm(bound) == "Clone" {
                let name = src.norm(name);
                output.push(ContractItem {
                    group: "Generic Clone bound".into(),
                    text: format!("In `<{name}: Clone>`, {name} is a named type parameter: a caller uses the function with a concrete type, and each occurrence of {name} in this signature refers to that same chosen type. The colon requires that type to satisfy the trait named Clone. If Clone resolves to Rust's standard Clone trait, values of {name} can be explicitly duplicated by calling clone(); cloning is not automatic and its cost depends on the implementation. The spelling alone does not prove which trait Clone refers to."),
                    span: src.span(bound), type_info: None,
                });
            }
            // Do not infer bounds from nested generic arguments such as Trait<Clone>.
            if matches!(bound.kind(), "trait_bounds" | "higher_ranked_trait_bound") { todo.extend(named_children(bound)); }
        }
    }
    output
}
