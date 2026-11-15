//! Explain a direct target architecture conditional-compilation attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_item" | "enum_variant" | "trait_item" | "impl_item" | "type_item" | "associated_type" | "field_declaration" | "const_item" | "static_item" | "mod_item" | "macro_definition") { return Vec::new(); }
    let mut result = Vec::new();
    let mut prev = node.prev_named_sibling();
    while let Some(attribute) = prev {
        if attribute.kind() == "attribute_item" {
            let compact: String = src.text(attribute).chars().filter(|ch| !ch.is_whitespace()).collect();
            if compact == "#[cfg(target_arch=\"x86_64\")]" {
                result.push(ContractItem {
                    group: "Target architecture condition".into(),
                    text: "`#[cfg(target_arch = \"x86_64\")]` conditionally includes the following item when the compilation target's architecture configuration is `x86_64`; if the condition is false, Rust excludes that item from the build. The target can differ from the machine running the compiler, and this condition says nothing by itself about the operating system. This is a compile-time configuration test, not a runtime check. The written attribute alone does not tell us which target is being built or whether the condition is currently true.".into(),
                    span: src.span(attribute), type_info: None,
                });
            }
        } else if !attribute.is_extra() { break; }
        prev = attribute.prev_named_sibling();
    }
    result.reverse();
    result
}
