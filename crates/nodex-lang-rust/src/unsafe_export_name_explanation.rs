//! Explain the explicit linker symbol named by `unsafe(export_name = ...)`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn export_name<'t>(src: Src<'_>, item: Node<'t>) -> Option<Node<'t>> {
    if item.kind() != "attribute_item" || item.has_error() { return None; }
    let attr = named_children(item).into_iter().find(|part| part.kind() == "attribute")?;
    let name = named_children(attr).into_iter().find(|part| part.kind() == "identifier")?;
    if src.text(name) != "unsafe" { return None; }
    let args = attr.child_by_field_name("arguments")?;
    if args.kind() != "token_tree" || args.has_error() { return None; }
    let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    match parts.as_slice() {
        [open, key, equal, value, close] if open.kind() == "(" && key.kind() == "identifier"
            && src.text(*key) == "export_name" && equal.kind() == "="
            && matches!(value.kind(), "string_literal" | "raw_string_literal")
            && !value.has_error() && close.kind() == ")" => Some(*value),
        _ => None,
    }
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "static_item") { return Vec::new(); }
    let mut result = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if let Some(value) = export_name(src, attribute) {
            result.push(ContractItem {
                group: "Unsafe exported symbol name".into(),
                text: format!("This attribute gives the attached function or static the exported linker symbol name specified by {} (interpreted using Rust's string-literal rules), instead of the compiler's usual mangled symbol name. The Rust item name is unchanged. The unsafe wrapper marks a safety obligation: the author must ensure the global symbol does not collide with another symbol or violate linked code's assumptions. This spelling cannot prove those obligations are satisfied. Final visibility depends on the build and linker; the attribute does not choose a calling convention or make a function safe to call across an FFI boundary.", src.text(value)),
                span: src.span(value), type_info: None,
            });
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
