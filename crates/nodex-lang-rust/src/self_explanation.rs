//! Explanations for the method receiver and current-module path `self`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn receiver(src: Src<'_>, function: Node<'_>) -> Option<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return None; }
    let parameter = named_children(function.child_by_field_name("parameters")?)
        .into_iter().find(|p| p.kind() == "self_parameter")?;
    let written = src.norm(parameter);
    let mode = if written.starts_with('&') {
        if children(parameter).iter().any(|child| child.kind() == "mutable_specifier") {
            "&mut self borrows that value exclusively and permits changes while the borrow lasts. Other access is restricted while this borrow is active; the caller retains ownership."
        } else {
            "&self borrows that value for shared access while the borrow lasts."
        }
    } else if written == "mut self" {
        "mut self receives the value by value and permits changes to the local receiver binding; the value is moved or copied according to its type."
    } else if written == "self" {
        "self receives the value by value. If its type implements Copy, the method receives a copy and the caller retains the original. Otherwise the value is moved into the method, and the caller cannot use that moved value afterward."
    } else {
        return None;
    };
    Some(ContractItem { group: "Receiver".into(), text: format!("self names the value on which this method is called, like this in C#. {mode}"), span: src.span(parameter), type_info: None })
}
