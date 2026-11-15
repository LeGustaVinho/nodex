//! Explain caller-location propagation on explicitly attributed functions.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn is_track_caller(src: Src<'_>, item: Node<'_>) -> bool {
    if item.kind() != "attribute_item" || item.has_error() { return false; }
    let Some(attr) = named_children(item).into_iter().find(|part| part.kind() == "attribute") else { return false; };
    let parts: Vec<_> = children(attr).into_iter().filter(|part| !part.is_extra()).collect();
    matches!(parts.as_slice(), [name] if name.kind() == "identifier" && src.text(*name) == "track_caller")
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let mut result = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if is_track_caller(src, attribute) {
            result.push(ContractItem {
                group: "Tracked caller location".into(),
                text: "#[track_caller] allows code in this function to obtain a hint of the caller's source location (file, line and column), for example through std::panic::Location::caller(); it can make panic diagnostics point to the call site. A chain of tracked functions propagates the location of the call made from the nearest untracked function; it does not always identify the immediate caller. This is a hint, not a guaranteed stack trace: a function pointer or virtual call may lose the actual caller location and report the tracked function's definition instead. On a trait method declaration the attribute applies to implementations. Rust restricts where this attribute is valid, including requiring the Rust ABI; its presence alone does not verify the program compiles.".into(),
                span: src.span(attribute), type_info: None,
            });
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
