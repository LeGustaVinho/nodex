//! Explain an explicit `Sized` bound without assuming that the name resolves to std.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" | "impl_item") { return Vec::new(); }
    let mut roots = Vec::new();
    if let Some(params) = declaration.child_by_field_name("type_parameters") { roots.push(params); }
    roots.extend(named_children(declaration).into_iter().filter(|node| node.kind() == "where_clause"));
    let mut result = Vec::new();
    while let Some(node) = roots.pop() {
        let target = match node.kind() {
            "type_parameter" => node.child_by_field_name("name"),
            "where_predicate" => node.child_by_field_name("left").filter(|left| left.kind() != "lifetime"),
            _ => None,
        };
        if let (Some(target), Some(bounds)) = (target, node.child_by_field_name("bounds")) {
            for bound in named_children(bounds) {
                // `?Sized` relaxes a bound and is not an affirmative requirement.
                if bound.kind() == "removed_trait_bound" { continue; }
                let spelling = src.norm(bound).replace(' ', "");
                if !matches!(spelling.as_str(), "Sized" | "std::marker::Sized" | "core::marker::Sized" | "::std::marker::Sized" | "::core::marker::Sized") { continue; }
                result.push(ContractItem {
                    group: "Sized type requirement".into(),
                    text: format!("{}: {} requires {} to implement the named trait. If {} resolves to Rust's standard Sized trait, the type's size is known at compile time. Type parameters normally have that standard requirement implicitly unless relaxed with ?Sized; an explicit bound states it here. Unsized types such as slices can still be used behind sized references or pointers. The spelling alone does not prove which trait a name resolves to.", src.norm(target), src.norm(bound), src.norm(target), src.norm(bound)),
                    span: src.span(bound), type_info: None,
                });
            }
            continue;
        }
        roots.extend(named_children(node).into_iter().rev());
    }
    result
}
