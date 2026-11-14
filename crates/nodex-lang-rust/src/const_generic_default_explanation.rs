//! Explain default values of const generic parameters on struct declarations.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "struct_item" { return Vec::new(); }
    let Some(params) = node.child_by_field_name("type_parameters") else { return Vec::new() };
    named_children(params).into_iter().filter_map(|param| {
        if param.kind() != "const_parameter" { return None; }
        let name = param.child_by_field_name("name")?;
        let ty = param.child_by_field_name("type")?;
        let value = param.child_by_field_name("value")?;
        if value.has_error() { return None; }
        Some(ContractItem {
            group: "Default const generic".into(),
            text: format!("`const {}: {} = {}` declares a generic parameter whose argument is a compile-time value of type {}. The `=` supplies the default {}; an explicit const argument selects a different value, and the default is used when the parameter is omitted in a context where defaults apply. The value is part of the instantiated type: different const arguments can produce different types. The initializer must satisfy Rust's compile-time and const-generic rules.", src.norm(name), src.norm(ty), src.norm(value), src.norm(ty), src.norm(value)),
            span: src.span(param),
            type_info: None,
        })
    }).collect()
}
