//! Explain how an unmangled name participates in linking.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "static_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    let written_name = src.norm(name);
    // `r#` permits a keyword to be written as an identifier; it is not part of its name.
    let symbol = written_name.strip_prefix("r#").unwrap_or(&written_name);
    let raw_note = if written_name != symbol {
        format!(" In {written_name}, `r#` only permits a keyword as an identifier; the identifier's name is {symbol}.")
    } else { String::new() };
    let mut result = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if attribute.kind() == "attribute_item" {
            let written: String = src.text(attribute).chars().filter(|ch| !ch.is_whitespace()).collect();
            if written == "#[unsafe(no_mangle)]" {
                result.push(ContractItem {
                    group: "Unmangled symbol and linking".into(),
                    text: format!("For this item, `#[unsafe(no_mangle)]` requests the unmangled symbol name {symbol}.{raw_note} A linker uses symbol names to connect references and definitions across object files and libraries; removing Rust's usual mangling also removes the uniqueness it normally provides. The author must ensure that the name and linked definitions do not violate the relevant safety requirements. Final symbol retention and export depend on the artifact and linker configuration. This attribute alone does not select a calling convention, establish ABI compatibility, or prove that linking is safe."),
                    span: src.span(attribute), type_info: None,
                });
            }
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
