//! Explain reserved `virtual` in ordinary syntax, including recovered invalid code.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// The module symbol remains visible even when a declaration is malformed.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut stack = vec![container];
    let mut items = Vec::new();
    while let Some(part) = stack.pop() {
        // Macro input and attributes contain token syntax, not necessarily Rust identifiers.
        // Lifetimes and labels have different lexical rules from identifier positions.
        if matches!(part.kind(), "mod_item" | "macro_definition" | "macro_invocation" | "token_tree" | "attribute_item" | "inner_attribute_item" | "lifetime" | "label" | "string_literal" | "raw_string_literal" | "char_literal" | "line_comment" | "block_comment") { continue; }
        if matches!(part.kind(), "identifier" | "type_identifier" | "field_identifier" | "shorthand_field_identifier") && src.text(part) == "virtual" {
            items.push(ContractItem {
                group: "Reserved keyword virtual".into(),
                text: "`virtual` is a reserved Rust keyword: it does not enable virtual dispatch or act as a C#-style method modifier. It cannot be used as an ordinary bare identifier. Where a name is needed, `r#virtual` is a raw identifier named `virtual`; it does not turn the word into a modifier. Rust can use trait objects for dynamic dispatch when appropriate. A recovering parser may still show parts of invalid code, so this explanation does not validate the surrounding program.".into(),
                span: src.span(part), type_info: None,
            });
            continue;
        }
        stack.extend(named_children(part).into_iter().rev());
    }
    items
}
