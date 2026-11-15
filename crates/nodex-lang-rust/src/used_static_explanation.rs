//! Explain retention requested by the bare `#[used]` static attribute.
//! Rust Reference: https://doc.rust-lang.org/reference/abi.html#the-used-attribute
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "static_item" { return Vec::new(); }
    let mut output = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            let compact: String = src.text(part).chars().filter(|ch| !ch.is_whitespace()).collect();
            if compact == "#[used]" {
                output.push(ContractItem {
                    group: "Retain static in object output".into(),
                    text: "`#[used]` tells the compiler to retain this static in the output object file or library object even if no other item in the crate references it. This affects compilation output, not a runtime access. It does not guarantee that the static survives into the final executable or library: the linker may still remove it. The attribute applies to static items; it does not itself export a symbol or select a linker section.".into(),
                    span: src.span(part), type_info: None,
                });
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    output.reverse();
    output
}
