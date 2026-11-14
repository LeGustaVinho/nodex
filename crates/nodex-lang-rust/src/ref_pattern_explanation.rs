//! Explain an explicit shared-reference binding in a Rust pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// `ref name` binds a shared reference; it is distinct from `ref mut` and `&pattern`.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "ref_pattern" { return None; }
    let parts = children(node);
    if parts.iter().any(|part| part.kind() == "mut_pattern" || part.kind() == "mutable_specifier") { return None; }
    let keyword = parts.iter().copied().find(|part| part.kind() == "ref")?;
    Some((keyword, ReadingContent::Detail {
        label: "Bind a shared reference".into(),
        text: format!("In the pattern {}, ref makes the name bind a shared reference to the matched part instead of moving that part into the binding. When the pattern matches, the underlying value remains where it was and can be read through the reference; the reference cannot be used to mutate it, except through types that explicitly allow interior mutation. The borrow must remain valid while used, and conflicting mutable access is restricted during the borrow. This is different from & in a pattern, which matches a reference.", src.norm(node)),
        definition: None,
        borrow: false,
    }))
}
