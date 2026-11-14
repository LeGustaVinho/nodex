//! Explain ordinary Rust block expressions and the role of a final semicolon.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children};
use tree_sitter::Node;

/// Identify ordinary expression blocks, leaving function/control/closure bodies to their own descriptions.
pub(crate) fn explain<'t>(block: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if block.kind() != "block" || block.has_error() { return None; }
    let owner = block.parent()?;
    if matches!(owner.kind(), "function_item" | "function_signature_item" | "for_expression" | "while_expression" | "loop_expression" | "if_expression" | "else_clause" | "closure_expression" | "async_block" | "unsafe_block" | "const_block" | "try_block" | "match_arm" | "gen_block") { return None; }
    if owner.kind() == "let_declaration" && owner.child_by_field_name("alternative") == Some(block) { return None; }
    let opening = children(block).into_iter().find(|token| token.kind() == "{")?;
    let first = named_children(block).into_iter().find(|node| !node.is_extra())?;
    Some((first, opening, ReadingContent::Detail {
        label: "Block as a value".into(),
        text: "The braces group statements in a scope. They run in order; if execution reaches a final expression without a semicolon, its value becomes the value of the whole block. If there is no final expression, or it ends in a semicolon, the block yields () (unit). Names declared inside the block are local to its scope. A statement that leaves the block early can prevent later statements and the final expression from running.".into(),
        definition: None, borrow: false,
    }))
}
