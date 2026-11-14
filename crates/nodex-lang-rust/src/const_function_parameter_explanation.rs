//! Explain const generic parameters declared on function signatures.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(parameters) = node.child_by_field_name("type_parameters") else { return Vec::new() };
    named_children(parameters).into_iter().filter_map(|param| {
        if param.kind() != "const_parameter" { return None; }
        let name = param.child_by_field_name("name")?;
        let ty = param.child_by_field_name("type")?;
        Some(ContractItem {
            group: "Const generic parameter".into(),
            text: format!("`const {}: {}` declares a generic parameter whose argument is a compile-time value of type {}, rather than a type or an ordinary runtime argument. Each use of this function supplies a suitable constant value, explicitly (for example with `::<...>`) or by inference when possible; the function can use that value in contexts such as an array length. A constant generic parameter is part of the function's generic signature and is not a mutable variable.", src.norm(name), src.norm(ty), src.norm(ty)),
            span: src.span(param),
            type_info: None,
        })
    }).collect()
}
