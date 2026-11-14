//! Explain named generic type parameters used by a function argument.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, function: Node<'_>) -> Vec<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(generics) = function.child_by_field_name("type_parameters") else { return Vec::new() };
    let Some(parameters) = function.child_by_field_name("parameters") else { return Vec::new() };
    let mut result = Vec::new();
    for generic in named_children(generics).into_iter().filter(|p| p.kind() == "type_parameter") {
        let Some(name) = generic.child_by_field_name("name") else { continue };
        let type_name = src.norm(name);
        let used_by = named_children(parameters).into_iter().find_map(|parameter| {
            let ty = parameter.child_by_field_name("type")?;
            if ty.kind() != "type_identifier" || src.norm(ty) != type_name { return None; }
            parameter.child_by_field_name("pattern")
        });
        let Some(parameter) = used_by else { continue };
        let name_of_argument = src.norm(parameter);
        result.push(ContractItem {
            group: "Named generic type parameter".into(),
            text: format!("In this function, <{type_name}> declares {type_name} as a placeholder for a concrete type. The parameter {name_of_argument}: {type_name} accepts a value of that type; each use of the same {type_name} in this function's signature refers to the same chosen type for that call. The caller may supply a type explicitly or the compiler may infer it from the arguments and context. Any declared trait or lifetime bounds must be satisfied. Rust checks the function using its declared requirements, so an unconstrained {type_name} does not automatically support operations such as addition."),
            span: src.span(generic),
            type_info: None,
        });
    }
    result
}
