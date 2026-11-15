//! Explain line documentation comments that describe their enclosing Rust module.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Look only at direct, leading comments of this file or inline module. Inner
/// docs describe the enclosing module, not its next item or nested modules.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => node,
        "mod_item" => match node.child_by_field_name("body") { Some(body) => body, None => return Vec::new() },
        _ => return Vec::new(),
    };
    let mut result = Vec::new();
    for child in children(container) {
        match child.kind() {
            "{" | "}" | "inner_attribute_item" => continue,
            "line_comment" if src.text(child).starts_with("//!") => {
                let prefix = src.index.span(src.file, child.start_byte(), child.start_byte() + 3);
                result.push(ContractItem {
                    group: "Inner documentation comment".into(),
                    text: "//! starts a line of documentation for the enclosing module; in a crate-root file it documents the crate. Documentation tools such as rustdoc can include this text in the generated API documentation. The ! makes this an inner doc comment: unlike ///, it describes the containing module rather than the item that follows. It is documentation, not an instruction executed by the program.".into(),
                    span: prefix,
                    type_info: None,
                });
            }
            "line_comment" if !src.text(child).starts_with("///") => continue,
            "block_comment" if !src.text(child).starts_with("/**") => continue,
            _ => break,
        }
    }
    result
}
