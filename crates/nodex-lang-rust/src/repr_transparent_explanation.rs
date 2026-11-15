//! Explain `#[repr(transparent)]` without claiming its layout constraints are met.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "struct_item" | "enum_item") { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if attribute.kind() == "attribute_item" {
            let written: String = src.text(attribute).chars().filter(|ch| !ch.is_whitespace()).collect();
            if written == "#[repr(transparent)]" {
                result.push(ContractItem {
                    group: "Transparent representation".into(),
                    text: "`#[repr(transparent)]` requests a layout and ABI that follow one field, which can be useful for a wrapper at a foreign-function boundary. For a valid transparent struct or single-variant enum, at most one field may have nonzero size or alignment greater than one; any other fields must have size zero and alignment one. Its layout and ABI match that field, or unit `()` when there is no such field. This syntax alone does not prove the declaration satisfies those restrictions or that an FFI use is safe; the compiler, target ABI, and surrounding interface still matter. It does not make the wrapper the same Rust type as its field.".into(),
                    span: src.span(attribute), type_info: None,
                });
            }
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
