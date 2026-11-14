//! Explanation of an explicitly typed `self: Arc<Self>` receiver.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Explain the written receiver without assuming the name `Arc` resolves to std.
pub(crate) fn explain(src: Src<'_>, function: Node<'_>) -> Option<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return None; }
    let params = function.child_by_field_name("parameters")?;
    let parameter = named_children(params).into_iter().find(|p| {
        p.kind() == "parameter" && p.child_by_field_name("pattern").is_some_and(|pattern| pattern.kind() == "self")
    })?;
    let ty = parameter.child_by_field_name("type")?;
    if ty.kind() != "generic_type" { return None; }
    let type_name = ty.child_by_field_name("type")?;
    if src.norm(type_name) != "Arc" { return None; }
    let arguments = ty.child_by_field_name("type_arguments")?;
    let args = named_children(arguments);
    if args.len() != 1 || src.norm(args[0]) != "Self" { return None; }
    Some(ContractItem {
        group: "Typed receiver".into(),
        text: "`self: Arc<Self>` declares the method's receiver explicitly: the caller supplies a value of type `Arc<Self>`, and `Self` denotes the implementing type. The method takes that receiver by value. If `Arc` resolves to Rust's standard `std::sync::Arc`, the value is an atomically reference-counted shared ownership handle; passing it transfers that handle, while other cloned handles can keep the allocation alive. Cloning a handle increments its strong reference count, and dropping one decrements it. Shared ownership alone does not grant unrestricted mutation of the inner value. A different `Arc` name may denote a different type, so its behavior depends on name resolution.".into(),
        span: src.span(parameter),
        type_info: None,
    })
}
