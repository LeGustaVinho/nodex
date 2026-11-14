//! Explain a value-bearing break to a lexically enclosing labeled block.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "break_expression" || node.has_error() { return None; }
    let parts = named_children(node);
    let label = parts.iter().copied().find(|part| part.kind() == "label")?;
    let keyword = children(node).into_iter().find(|part| part.kind() == "break")?;
    let value = parts.iter().copied().find(|part| part.kind() != "label" && !part.is_extra())?;
    let wanted = src.norm(label);
    let mut current = node.parent();
    while let Some(parent) = current {
        // A break cannot escape a function, closure, async block, or const block.
        if matches!(parent.kind(), "function_item" | "closure_expression" | "async_block" | "const_block") { return None; }
        let destination = named_children(parent).into_iter().find(|part| part.kind() == "label" && src.norm(*part) == wanted);
        if destination.is_some() {
            // A loop with the same label shadows an outer labeled block.
            if parent.kind() != "block" { return None; }
            return Some((keyword, ReadingContent::Detail {
                label: "Leave a labeled block with a value".into(),
                text: format!("{wanted}: names this block; it does not repeat like a loop. Evaluate {}, then `break {wanted}` immediately leaves the named enclosing block, skipping its remaining statements. The evaluated value becomes the value of that block expression, so its surrounding expression can use it. Without this break, the block's final expression supplies its value (or () if it has none). The apostrophe marks a label, not a string or lifetime. Rust checks that the block's possible result values have compatible types.", src.norm(value)),
                definition: None,
                borrow: false,
            }));
        }
        current = parent.parent();
    }
    None
}
