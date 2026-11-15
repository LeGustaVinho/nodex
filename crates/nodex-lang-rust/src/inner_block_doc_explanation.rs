//! Explain `/*! ... */` documentation on the enclosing item.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Inner documentation belongs to its own enclosing item, not the next item.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" | "function_item" | "trait_item" | "impl_item" | "foreign_mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut items = Vec::new();
    for child in children(container) {
        let text = src.text(child);
        match child.kind() {
            "{" | "inner_attribute_item" => continue,
            "line_comment" if !text.starts_with("///") => continue,
            "block_comment" if text.starts_with("/*!") && text.ends_with("*/") => {
                items.push(ContractItem {
                    group: "Inner block documentation".into(),
                    text: "/*! ... */ is an inner documentation comment: it documents the enclosing crate, module or item, rather than the item following the comment. The opening /*! and closing */ delimit a block that can span multiple lines. Rust documentation tools can render its text, commonly as Markdown; the comment is not an instruction executed by the program.".into(),
                    span: src.span(child),
                    type_info: None,
                });
            }
            "block_comment" if !text.starts_with("/**") => continue,
            _ => break,
        }
    }
    items
}
