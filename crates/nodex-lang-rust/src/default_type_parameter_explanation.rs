//! Explain written default type parameters on Rust type declarations.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "struct_item" | "enum_item" | "trait_item" | "type_item") {
        return Vec::new();
    }
    let Some(parameters) = declaration.child_by_field_name("type_parameters") else { return Vec::new() };
    named_children(parameters).into_iter().filter_map(|parameter| {
        if parameter.kind() != "type_parameter" { return None; }
        let name = parameter.child_by_field_name("name")?;
        let default = parameter.child_by_field_name("default_type")?;
        Some(ContractItem {
            group: "Default generic type".into(),
            text: format!("{} is a generic type parameter. The `= {}` specifies its default type: in a context where Rust applies defaults, omitting this argument uses {}. A caller can supply a different type explicitly; inference and context can also affect the choice where Rust permits inference. The chosen type must satisfy any bounds. This declaration does not create a value.", src.norm(name), src.norm(default), src.norm(default)),
            span: src.span(parameter),
            type_info: None,
        })
    }).collect()
}
