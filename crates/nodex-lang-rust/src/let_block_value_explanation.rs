//! Explain a block used to initialize a `let` binding.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

use crate::syntax::{block_statements, statement_tail, unparen};

pub(crate) fn explain<'t>(src: Src<'_>, declaration: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if declaration.kind() != "let_declaration" || declaration.has_error() { return None; }
    let block = unparen(declaration.child_by_field_name("value")?);
    if block.kind() != "block" { return None; }
    let equals = children(declaration).into_iter().find(|child| child.kind() == "=")?;
    let tail = block_statements(block).next_back().and_then(statement_tail);
    let ending = if let Some(expression) = tail {
        format!("If execution reaches the final expression {} without an ending semicolon, its result becomes the value of the block.", src.norm(expression))
    } else {
        "If execution reaches the end without a final expression, the block produces the unit value (). A semicolon after an expression discards that expression's value.".into()
    };
    Some((tail.unwrap_or(block), equals, ReadingContent::Detail {
        label: "Initialize from a block".into(),
        text: format!("Run the statements inside the braces in order when this declaration is evaluated. {ending} The `let` pattern then binds the block's result. A semicolon after the closing brace ends the `let` statement; it does not discard a final expression inside the block."),
        definition: None,
        borrow: false,
    }))
}
