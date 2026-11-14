//! Explain lifetime parameters declared on function signatures.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(parameters) = node.child_by_field_name("type_parameters") else { return Vec::new() };
    named_children(parameters).into_iter().filter_map(|param| {
        if param.kind() != "lifetime_parameter" { return None; }
        let lifetime = named_children(param).into_iter().find(|part| part.kind() == "lifetime")?;
        let name = src.norm(lifetime);
        Some(ContractItem {
            group: "Lifetime parameter".into(),
            text: format!("{name} declares a lifetime parameter for this function. It is a name for a region of validity used in the signature: occurrences of {name} express relationships that the compiler checks between references or other bounds. The function can be called with different suitable lifetimes; this declaration does not extend the lifetime of any value or determine how long a call runs. Bounds written on the parameter may restrict which lifetimes are valid."),
            span: src.span(lifetime),
            type_info: None,
        })
    }).collect()
}
