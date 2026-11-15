//! Explain a type representation attribute requesting 16-byte alignment.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "struct_item" | "union_item" | "enum_item") { return Vec::new(); }
    let mut result = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            let compact: String = src.text(part).chars().filter(|ch| !ch.is_whitespace()).collect();
            if compact == "#[repr(align(16))]" {
                result.push(ContractItem {
                    group: "Minimum alignment".into(),
                    text: "`#[repr(align(16))]` requests that the following type have an alignment of at least 16 bytes. A valid address for a value of that type must satisfy its resulting alignment; the actual alignment can be larger if the type already requires it. The compiler may add padding, and the size of the type is a multiple of its alignment. This attribute does not by itself give the type C layout or fix the order of its fields; it affects representation rather than evaluating code at runtime.".into(),
                    span: src.span(part), type_info: None,
                });
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    result.reverse();
    result
}
