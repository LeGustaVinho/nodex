//! Explain a callable bound written as `F: FnMut(i32)`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item" | "type_item") { return Vec::new(); }
    let mut roots = Vec::new();
    if let Some(parameters) = declaration.child_by_field_name("type_parameters") { roots.push(parameters); }
    roots.extend(named_children(declaration).into_iter().filter(|node| node.kind() == "where_clause"));
    let mut output = Vec::new();
    while let Some(node) = roots.pop() {
        let target = match node.kind() {
            "type_parameter" => node.child_by_field_name("name"),
            "where_predicate" => node.child_by_field_name("left"),
            _ => None,
        };
        if let (Some(target), Some(bounds)) = (target, node.child_by_field_name("bounds")) {
            for bound in named_children(bounds) {
                if bound.kind() != "function_type" { continue; }
                let Some(trait_name) = bound.child_by_field_name("trait") else { continue; };
                let name = src.norm(trait_name).replace(' ', "");
                if !matches!(name.as_str(), "FnMut" | "std::ops::FnMut" | "core::ops::FnMut" | "::std::ops::FnMut" | "::core::ops::FnMut") { continue; }
                output.push(ContractItem {
                    group: "Mutable callable bound".into(),
                    text: format!("{}: {} requires the chosen type for {} to satisfy this callable bound. The parentheses specify the call's input types and any `->` specifies the result type. If {} resolves to Rust's standard FnMut trait, the callable can be invoked with mutable access to its state, so calls may change captured data; it can be called repeatedly when the borrow and ownership rules allow. It does not imply a call happens here or that every call actually mutates state. The written name alone does not prove which trait it resolves to.", src.norm(target), src.norm(bound), src.norm(target), src.norm(trait_name)),
                    span: src.span(bound), type_info: None,
                });
            }
            continue;
        }
        roots.extend(named_children(node).into_iter().rev());
    }
    output
}
