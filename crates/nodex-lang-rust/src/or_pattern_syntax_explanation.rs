//! Explain `|` as an alternative in Rust patterns.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Return the surrounding control-flow construct and `|` token to attach the
/// explanation to a visible node without displacing existing pattern cards.
pub(crate) fn explain<'t>(src: Src<'_>, pattern: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if pattern.kind() != "or_pattern" || pattern.has_error() { return None; }
    if pattern.parent().is_some_and(|parent| parent.kind() == "or_pattern") { return None; }
    let pipe = children(pattern).into_iter().find(|part| part.kind() == "|" && !part.is_missing())?;
    let mut owner = pattern.parent()?;
    loop {
        if matches!(owner.kind(), "match_expression" | "if_expression" | "while_expression" | "for_expression" | "let_declaration") { break; }
        if matches!(owner.kind(), "function_item" | "closure_expression" | "source_file") { return None; }
        owner = owner.parent()?;
    }
    Some((owner, pipe, ReadingContent::Detail {
        label: "Alternative patterns".into(),
        text: format!("In {}, `|` means that the value can match any of the written alternative patterns. These are alternatives for the same position, not a bitwise operation on values. Every alternative must introduce the same names, with the same types and the same binding modes, so the selected arm or body can use those names consistently. A match guard, if present, is checked after a pattern matches; a failed guard continues matching. The applicable context still determines whether the whole pattern must be irrefutable.", src.norm(pattern)),
        definition: None,
        borrow: false,
    }))
}
