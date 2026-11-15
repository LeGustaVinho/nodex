//! Explain ordinary block comments on the symbol that contains them.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, owner: Node<'_>) -> Vec<ContractItem> {
    if !matches!(owner.kind(), "source_file" | "mod_item" | "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item" | "const_item" | "static_item" | "type_item") { return Vec::new(); }
    let mut pending = vec![owner];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if node != owner && matches!(node.kind(), "function_item" | "function_signature_item" | "mod_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item" | "const_item" | "static_item" | "type_item" | "token_tree") { continue; }
        if node.kind() == "block_comment" {
            let text = src.text(node);
            if node.has_error() || !text.starts_with("/*") || !text.ends_with("*/") || text.starts_with("/*!") || (text.starts_with("/**") && !text.starts_with("/***") && text != "/**/") { continue; }
            result.push(ContractItem {
                group: "Block comment".into(),
                text: "`/*` starts a block comment and its matching `*/` ends it; the text inside can span multiple lines and is not executed as Rust code. Rust allows another `/* ... */` inside: its closing delimiter closes the inner comment, and the outer comment continues until its own closing delimiter. Documentation comments such as `/** ... */` and `/*! ... */` have a separate documentation role. A delimiter-looking sequence inside a string is string content, not a comment.".into(),
                span: src.index.span(src.file, node.start_byte(), node.start_byte() + 2), type_info: None,
            });
            continue;
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    result
}
