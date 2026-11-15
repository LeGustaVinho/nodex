//! Explain omitted commas after match arms with block bodies.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "match_expression" || node.has_error() { return Vec::new(); }
    let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
    let Some(anchor) = children(node).into_iter().find(|part| part.kind() == "match") else { return Vec::new(); };
    let arms: Vec<_> = named_children(body).into_iter().filter(|part| part.kind() == "match_arm").collect();
    arms.windows(2).filter_map(|pair| {
        let arm = pair[0];
        let next = pair[1];
        let value = arm.child_by_field_name("value")?;
        if value.kind() != "block" { return None; }
        let intervening = src.text.get(value.end_byte()..next.start_byte())?;
        if intervening.contains(',') { return None; }
        let close = children(value).into_iter().rev().find(|part| part.kind() == "}")?;
        Some((anchor, close, ReadingContent::Detail {
            label: "Match arm without a comma".into(),
            text: "This match arm ends in a block `{ ... }`, so Rust permits the comma between its closing brace and the next arm to be omitted. The next pattern still begins a separate arm; the match selects the first arm whose pattern and any guard succeed. A comma is also allowed after the block. For an expression without a block used as a non-final arm, a separating comma is required; the final arm may omit its trailing comma. Other expressions with blocks have their own grammar. The braces delimit the arm's expression and do not make the next arm part of that block.".into(),
            definition: None, borrow: false,
        }))
    }).collect()
}
