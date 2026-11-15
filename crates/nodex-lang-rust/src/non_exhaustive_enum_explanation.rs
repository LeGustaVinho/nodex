//! Explain an enum marked `#[non_exhaustive]`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "enum_item" { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if attribute.kind() == "attribute_item" {
            let written: String = src.text(attribute).chars().filter(|ch| !ch.is_whitespace()).collect();
            if written == "#[non_exhaustive]" {
                result.push(ContractItem {
                    group: "Enum may gain variants".into(),
                    text: "`#[non_exhaustive]` marks this enum so its defining crate can add variants in a future version without downstream code assuming today's list is complete. In a different crate, a match must allow for other variants, usually with a wildcard `_` arm, even when it names all currently visible variants. Inside the defining crate the usual exhaustiveness rules apply to the variants known there. This attribute does not add a variant now or change the enum's Rust visibility; it changes what downstream code may assume.".into(),
                    span: src.span(attribute), type_info: None,
                });
            }
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
