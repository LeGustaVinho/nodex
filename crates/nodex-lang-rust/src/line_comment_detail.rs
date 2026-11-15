//! Explain ordinary line comments on their containing symbol.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, owner: Node<'_>) -> Vec<ContractItem> {
    if !matches!(owner.kind(), "source_file" | "mod_item" | "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item" | "const_item" | "static_item" | "type_item") { return Vec::new(); }
    let mut pending = vec![owner];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if node != owner && matches!(node.kind(), "function_item" | "function_signature_item" | "mod_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item" | "const_item" | "static_item" | "type_item" | "token_tree") { continue; }
        if node.kind() == "line_comment" {
            let text = src.text(node);
            if !text.starts_with("//") || text.starts_with("//!") || (text.starts_with("///") && !text.starts_with("////")) { continue; }
            let span = src.index.span(src.file, node.start_byte(), node.start_byte() + 2);
            result.push(ContractItem {
                group: "Line comment".into(),
                text: "`//` begins an ordinary line comment: the text after it is commentary for readers until the end of the line, rather than Rust code executed by the program. The next line is parsed normally. Documentation comments `///` and `//!` have different roles and can contribute generated documentation. A `//` inside a string literal is string content, not a comment delimiter.".into(),
                span, type_info: None,
            });
            continue;
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    result
}
