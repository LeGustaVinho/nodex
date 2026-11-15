//! Explain Rust's nested block-comment delimiters on the containing symbol.
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
            let written = src.text(node);
            if node.has_error() || !written.starts_with("/*") || !written.ends_with("*/") { continue; }
            let Some(relative) = written[2..].find("/*").map(|offset| offset + 2) else { continue; };
            let site = src.index.span(src.file, node.start_byte() + relative, node.start_byte() + relative + 2);
            result.push(ContractItem {
                group: "Nested block comment".into(),
                text: "Rust block comments can be nested. The inner /* starts another comment inside the outer /* ... */; its matching */ closes only that inner level. Text after that inner closing delimiter is still inside the outer comment until the outer */ closes it. Ordinary comment text is not executed. Documentation-comment forms can additionally contribute documentation; markers inside strings or line comments are not block-comment delimiters.".into(),
                span: site,
                type_info: None,
            });
            continue;
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    result
}
