//! Explain a `ref` parameter pattern whose declared argument is itself a reference.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, function: Node<'_>) -> Vec<ContractItem> {
    if function.kind() != "function_item" || function.has_error() { return Vec::new(); }
    let Some(parameters) = function.child_by_field_name("parameters") else { return Vec::new(); };
    let mut result = Vec::new();
    for parameter in named_children(parameters).into_iter().filter(|part| part.kind() == "parameter") {
        let (Some(pattern), Some(ty)) = (parameter.child_by_field_name("pattern"), parameter.child_by_field_name("type")) else { continue; };
        if pattern.kind() != "ref_pattern" || ty.kind() != "reference_type" || children(ty).iter().any(|part| part.kind() == "mutable_specifier") { continue; }
        let parts = children(pattern);
        if parts.iter().any(|part| matches!(part.kind(), "mut_pattern" | "mutable_specifier")) { continue; }
        let Some(keyword) = parts.iter().copied().find(|part| part.kind() == "ref") else { continue; };
        let names = named_children(pattern);
        let [name] = names.as_slice() else { continue; };
        if name.kind() != "identifier" { continue; }
        result.push(ContractItem {
            group: "Reference-binding parameter".into(),
            text: format!("The type annotation {} says that the value passed to this parameter is already a shared reference. The pattern `ref {}` then borrows that parameter value and binds {} to a shared reference to it, so inside the function {} is a reference to a reference (for example, `&&T` when the annotation is `&T`). `*{}` accesses the passed reference and `**{}` accesses the referenced value, subject to Rust's borrowing and access rules. `ref` changes how the local name binds the parameter; it does not clone the underlying value or extend its lifetime.", src.norm(ty), src.norm(*name), src.norm(*name), src.norm(*name), src.norm(*name), src.norm(*name)),
            span: src.span(keyword), type_info: None,
        });
    }
    result
}
