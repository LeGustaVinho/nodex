//! Explain the raw identifier spellings reserved by Rust.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut stack = vec![container];
    let mut found = Vec::new();
    while let Some(part) = stack.pop() {
        if matches!(part.kind(), "mod_item" | "macro_definition" | "macro_invocation" | "token_tree" | "attribute_item" | "inner_attribute_item" | "lifetime" | "label" | "string_literal" | "raw_string_literal" | "char_literal" | "line_comment" | "block_comment") { continue; }
        if part.child_count() == 0 && matches!(src.text(part), "r#self" | "r#Self" | "r#super" | "r#crate" | "r#_") {
            let spelling = src.text(part);
            found.push(ContractItem {
                group: "Reserved raw identifier".into(),
                text: format!("`{spelling}` is reserved and cannot be used as a raw identifier. The `r#` prefix allows many keywords as names, but Rust excludes `self`, `Self`, `super`, `crate`, and `_` from raw identifiers. Choose a different name if a name was intended; `_` on its own has context-dependent meanings including wildcard patterns and inferred placeholders. A syntax parser may recover invalid code; this explanation does not make the program valid."),
                span: src.span(part), type_info: None,
            });
            continue;
        }
        stack.extend(children(part).into_iter().rev());
    }
    found
}
