//! Explain reserved `priv` in ordinary syntax, including recovered invalid code.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut pending = vec![container];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        // Raw token input, attributes, literals, labels and lifetimes have their
        // own lexical contexts; do not diagnose them as ordinary identifiers.
        if matches!(part.kind(), "mod_item" | "macro_definition" | "macro_invocation" | "token_tree" | "attribute_item" | "inner_attribute_item" | "lifetime" | "label" | "string_literal" | "raw_string_literal" | "char_literal" | "line_comment" | "block_comment") { continue; }
        if matches!(part.kind(), "identifier" | "type_identifier" | "field_identifier" | "shorthand_field_identifier") && src.text(part) == "priv" {
            result.push(ContractItem {
                group: "Reserved keyword priv".into(),
                text: "`priv` is a reserved Rust keyword, not a visibility modifier like C# `private`, and it cannot be used as an ordinary bare identifier. An ordinary module item with no visibility modifier is private to its declaring module and descendants, subject to ancestor visibility; other items, such as enum variants and trait items, have their own visibility rules. Where a name is needed, `r#priv` is a raw identifier named `priv`, not a visibility modifier. A recovering parser may still show parts of invalid code; this explanation does not validate the surrounding program.".into(),
                span: src.span(part), type_info: None,
            });
            continue;
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
