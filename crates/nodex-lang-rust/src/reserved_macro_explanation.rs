//! Explain the reserved `macro` keyword without confusing it with macro_rules!.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn visit(src: Src<'_>, node: Node<'_>, items: &mut Vec<ContractItem>) {
    // Macro bodies and inputs contain token trees, not necessarily Rust code.
    if matches!(node.kind(), "mod_item" | "macro_invocation" | "macro_definition" | "token_tree" | "line_comment" | "block_comment" | "string_literal" | "raw_string_literal" | "char_literal") { return; }
    if matches!(node.kind(), "identifier" | "type_identifier" | "field_identifier" | "macro") && src.text(node) == "macro" {
        items.push(ContractItem {
            group: "Reserved word macro".into(),
            text: "`macro` is a reserved Rust keyword, so it cannot be used as an ordinary unescaped identifier; `r#macro` can name an identifier. Experimental declarative macros 2.0 use `macro` in a definition and currently require the unstable `decl_macro` feature on a supporting nightly compiler. Stable declarative macros are written with `macro_rules!`, a distinct spelling and form. This occurrence alone does not establish the compiler version, enabled features, or whether the surrounding code compiles.".into(),
            span: src.span(node), type_info: None,
        });
        return;
    }
    for child in named_children(node) { visit(src, child, items); }
}

/// Attach the explanation to the containing module, including malformed syntax.
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
