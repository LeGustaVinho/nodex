//! Explain the `u8` representation attribute on an enum.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "enum_item" { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut items = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "repr" { continue; }
        let Some(args) = meta.child_by_field_name("arguments") else { continue; };
        if !children(args).into_iter().any(|part| !part.is_extra() && src.text(part) == "u8") { continue; }
        items.push(ContractItem {
            group: "Enum u8 representation".into(),
            text: format!("{} specifies an 8-bit unsigned integer representation for this enum's discriminant, the tag that distinguishes variants. Discriminant values must fit the u8 range 0 through 255; explicitly assigned discriminants must also satisfy Rust's enum rules. This does not imply that every value of 0 through 255 is a valid variant. An enum whose variants carry data also stores that data and can require additional space or alignment, so the entire enum is not necessarily one byte. Other representation options on the same attribute and the compilation target can affect layout; the attribute alone is not a complete foreign-interface safety guarantee.", src.norm(attribute)),
            span: src.span(attribute), type_info: None,
        });
    }
    items.reverse();
    items
}
