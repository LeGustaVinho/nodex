//! Explain the `unsafe(no_mangle)` outer attribute without claiming link safety.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "static_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if attribute.kind() == "attribute_item" {
            let written: String = src.text(attribute).chars().filter(|ch| !ch.is_whitespace()).collect();
            if written == "#[unsafe(no_mangle)]" {
                result.push(ContractItem {
                    group: "Unsafe unmangled symbol".into(),
                    text: format!("`#[unsafe(no_mangle)]` asks the compiler to use the item's name {} as its symbol name without Rust's usual name mangling, and makes that symbol publicly exported from the produced object or library. The `unsafe(...)` wrapper marks an attribute with safety obligations: the author must ensure that this global symbol name does not collide with another symbol or violate the assumptions of linked code. The spelling alone cannot prove those obligations are satisfied. Final visibility depends on the build and linker; this attribute does not choose a calling convention or make a function safe to call across an FFI boundary.", src.text(name).strip_prefix("r#").unwrap_or(src.text(name))),
                    span: src.span(attribute), type_info: None,
                });
            }
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
