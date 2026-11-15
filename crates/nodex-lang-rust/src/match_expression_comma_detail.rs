//! Explain a comma following a non-final expression arm of a match.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(_src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "match_expression" || node.has_error() { return Vec::new(); }
    let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
    let Some(anchor) = children(node).into_iter().find(|part| part.kind() == "match") else { return Vec::new(); };
    let arms: Vec<_> = named_children(body).into_iter().filter(|n| n.kind() == "match_arm").collect();
    arms.windows(2).filter_map(|pair| {
        let arm = pair[0];
        let value = arm.child_by_field_name("value")?;
        // These are unambiguously expressions without a block. Leave other
        // kinds to generic explanations rather than guessing their grammar.
        if !matches!(value.kind(), "integer_literal" | "float_literal" | "boolean_literal" | "char_literal" | "string_literal" | "raw_string_literal" | "identifier" | "call_expression" | "binary_expression" | "unit_expression") { return None; }
        let comma = children(arm).into_iter().find(|part| part.kind() == "," && !part.is_missing())?;
        Some((anchor, comma, ReadingContent::Detail {
            label: "Comma between match arms".into(),
            text: "This comma separates the preceding match arm from the next one. When a non-final arm's result is an expression without a block, its separating comma is required; the final arm may omit its trailing comma. An arm whose result is an expression with a block may omit the separator after that block. These separators mark syntax, not extra values or execution: Rust selects the first matching arm whose guard, if any, succeeds, and only that arm's result expression runs.".into(),
            borrow: false, definition: None,
        }))
    }).collect()
}
