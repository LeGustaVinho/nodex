//! Explain a unit-success Result-shaped return without assuming type identity.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") || node.has_error() { return None; }
    let result = node.child_by_field_name("return_type")?;
    if result.kind() != "generic_type" { return None; }
    let name = result.child_by_field_name("type")?;
    if !matches!(src.norm(name).as_str(), "Result" | "std::result::Result" | "core::result::Result") { return None; }
    let args = result.child_by_field_name("type_arguments")?;
    let arguments: Vec<_> = named_children(args).into_iter().filter(|part| !part.is_extra()).collect();
    let [success, error] = arguments.as_slice() else { return None; };
    if success.kind() != "unit_type" { return None; }
    let error = src.norm(*error);
    Some(ContractItem {
        group: "Result with unit success".into(),
        text: format!("`-> {}` is the function's return type. If `Result` resolves to Rust's standard result type, a normal return is either `Ok(())`, representing success with no additional meaningful value, or `Err(error)`, carrying a value of type {error}. `()` is the unit type and has one value, also written `()`. Returning an `Err` is an ordinary return of a value; this signature does not automatically throw or propagate it, and it does not prove what the function body will actually do. The written names alone do not establish which `Result` or {error} type is meant.", src.norm(result)),
        span: src.span(result), type_info: None,
    })
}
