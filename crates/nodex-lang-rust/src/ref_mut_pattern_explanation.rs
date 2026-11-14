//! Explain explicit mutable-reference bindings in Rust patterns.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Recognize `ref mut` where it creates a binding, including field shorthand.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if !matches!(node.kind(), "ref_pattern" | "field_pattern") { return None; }
    let parts = children(node);
    let keyword = parts.iter().copied().find(|part| part.kind() == "ref")?;
    let mutable = if node.kind() == "ref_pattern" {
        named_children(node).into_iter().any(|part| part.kind() == "mut_pattern")
    } else {
        parts.iter().any(|part| part.kind() == "mutable_specifier")
    };
    if !mutable { return None; }
    Some((keyword, ReadingContent::Detail {
        label: "Bind with an exclusive reference".into(),
        text: format!("In the pattern {}, `ref mut` binds the matched part through an exclusive mutable reference rather than moving that part into the new name. The name holds a reference; dereference it with `*` to access or change the matched value. While this borrow is active, other conflicting access is restricted. The reference cannot outlive the value it borrows. This `mut` is part of the reference-binding mode, not merely permission to reassign the local name.", src.norm(node)),
        definition: None,
        borrow: false,
    }))
}
