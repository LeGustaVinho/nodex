//! Beginner-facing explanation of an unsafe block.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Describe the explicit safety scope without assuming which operations it contains.
pub(crate) fn explain<'t>(_src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "unsafe_block" { return None; }
    let keyword = children(node).into_iter().find(|part| part.kind() == "unsafe")?;
    Some((keyword, ReadingContent::Detail {
        label: "Unsafe block and safety obligations".into(),
        text: "unsafe { ... } permits operations that require extra safety guarantees, such as dereferencing a raw pointer or calling an unsafe function. The programmer must check each operation's safety requirements; the block itself does not prove them or make an operation safe. Rust still checks types and borrowing rules, and ordinary operations inside the block keep their usual behavior. The braces limit this permission to this block; like other blocks, its final expression can provide a value.".into(),
        definition: None,
        borrow: false,
    }))
}
