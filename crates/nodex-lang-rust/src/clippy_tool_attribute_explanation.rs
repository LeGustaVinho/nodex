//! Explain the Clippy minimum-supported-Rust-version tool attribute.
//! Clippy configuration: https://doc.rust-lang.org/clippy/configuration.html
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_item" | "enum_variant" | "trait_item" | "type_item" | "associated_type" | "field_declaration" | "const_item" | "static_item" | "mod_item" | "macro_definition") { return Vec::new(); }
    let mut result = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            if let Some(attr) = named_children(part).into_iter().find(|child| child.kind() == "attribute") {
                if let (Some(path), Some(value)) = (named_children(attr).into_iter().next(), attr.child_by_field_name("value")) {
                    if src.norm(path) == "clippy::msrv" && value.kind() == "string_literal" {
                        result.push(ContractItem {
                            group: "Clippy minimum Rust version".into(),
                            text: format!("`clippy::msrv` is an attribute for the Clippy lint tool. Its written value {} specifies the minimum supported Rust version (MSRV) for Clippy's analysis in this scope. Lints that use MSRV may suppress a suggestion or choose one compatible with that version. The attribute does not install or select a compiler, prove the code compiles on that version, or itself run Clippy. Toolchain support and whether the value is accepted must be checked separately.", src.text(value)),
                            span: src.span(part), type_info: None,
                        });
                    }
                }
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    result.reverse();
    result
}
