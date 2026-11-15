//! Explain a `Pin<&mut Self>` method receiver without assuming resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, function: Node<'_>) -> Option<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return None; }
    let parameters = function.child_by_field_name("parameters")?;
    let receiver = named_children(parameters).into_iter().find(|parameter| {
        parameter.kind() == "parameter" && parameter.child_by_field_name("pattern").is_some_and(|pattern| pattern.kind() == "self")
    })?;
    let ty = receiver.child_by_field_name("type")?;
    if ty.kind() != "generic_type" { return None; }
    let constructor = ty.child_by_field_name("type")?;
    if !matches!(src.norm(constructor).as_str(), "Pin" | "std::pin::Pin" | "core::pin::Pin") { return None; }
    let args = named_children(ty.child_by_field_name("type_arguments")?);
    if args.len() != 1 || args[0].kind() != "reference_type" || !children(args[0]).iter().any(|part| part.kind() == "mutable_specifier") { return None; }
    let target = args[0].child_by_field_name("type")?;
    if src.norm(target) != "Self" { return None; }
    Some(ContractItem {
        group: "Pinned mutable receiver".into(),
        text: format!("`self: {}` declares an explicit method receiver: `Self` is the implementing type and `&mut Self` is an exclusive mutable reference to it. The receiver value is passed to the method by value; this does not transfer ownership of the underlying `Self`. If `{}` resolves to Rust's standard `Pin`, it wraps that reference with pinning guarantees: for a value that is not `Unpin`, safe code cannot move the pinned value out of its location before it is dropped. For `Unpin` values, pinning imposes no additional movement restriction. Pinning does not itself allocate on the heap or automatically make arbitrary mutation safe. The written name alone does not prove which type `Pin` denotes; Rust checks the receiver type and method call.", src.norm(ty), src.norm(constructor)),
        span: src.span(receiver), type_info: None,
    })
}
