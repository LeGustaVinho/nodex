//! Explain a method receiver explicitly written as `self: Box<Self>`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, function: Node<'_>) -> Option<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return None; }
    let parameters = function.child_by_field_name("parameters")?;
    let receiver = named_children(parameters).into_iter().find(|parameter| {
        parameter.kind() == "parameter"
            && parameter.child_by_field_name("pattern").is_some_and(|pattern| pattern.kind() == "self")
    })?;
    let ty = receiver.child_by_field_name("type")?;
    if ty.kind() != "generic_type" { return None; }
    let constructor = ty.child_by_field_name("type")?;
    let written_constructor = src.norm(constructor);
    if !matches!(written_constructor.as_str(), "Box" | "std::boxed::Box" | "alloc::boxed::Box") { return None; }
    let args = named_children(ty.child_by_field_name("type_arguments")?);
    if args.len() != 1 || src.norm(args[0]) != "Self" { return None; }
    Some(ContractItem {
        group: "Boxed receiver".into(),
        text: format!("`self: {}` declares the method's receiver explicitly: the method takes the receiver value by value rather than borrowing it merely because it is called as a method. `Self` means the implementing type. If `{}` resolves to Rust's standard Box, the receiver owns a heap-allocated value of that type; passing the Box moves that owning handle into the method, so the caller cannot use that moved Box afterward. The spelling alone cannot prove which Box type is in scope.", src.norm(ty), written_constructor),
        span: src.span(receiver),
        type_info: None,
    })
}
