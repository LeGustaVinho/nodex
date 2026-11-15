//! Explain the reserved word `override` when written as an identifier.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn visit(src: Src<'_>, node: Node<'_>, items: &mut Vec<ContractItem>) {
    // Macro input is a token stream, not necessarily Rust identifiers or code.
    if matches!(node.kind(), "mod_item" | "macro_invocation" | "macro_definition" | "token_tree" | "line_comment" | "block_comment" | "string_literal" | "raw_string_literal" | "char_literal") { return; }
    if matches!(node.kind(), "identifier" | "type_identifier" | "field_identifier") && src.text(node) == "override" {
        items.push(ContractItem {
            group: "Reserved word override".into(),
            text: "`override` is reserved in Rust and has no ordinary language meaning. It cannot be used as an unescaped identifier, so code using it this way is rejected by the Rust compiler. If the intent is an identifier named `override`, write `r#override`; this raw identifier does not declare or perform method overriding. The parser alone does not establish whether the surrounding program compiles.".into(),
            span: src.span(node), type_info: None,
        });
        return;
    }
    for child in named_children(node) { visit(src, child, items); }
}

/// Attach diagnostics to the containing module so invalid syntax remains visible.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut items = Vec::new();
    for child in named_children(container) { visit(src, child, &mut items); }
    items
}
