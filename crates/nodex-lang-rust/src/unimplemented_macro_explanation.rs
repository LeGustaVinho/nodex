//! Explain the empty `unimplemented!()` placeholder without assuming resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    let path: String = src.text(name).chars().filter(|ch| !ch.is_whitespace()).collect();
    if !matches!(path.as_str(), "unimplemented" | "std::unimplemented" | "::std::unimplemented" | "core::unimplemented" | "::core::unimplemented") { return None; }
    let body = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    let parts: Vec<_> = children(body).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() != 2 || parts[0].kind() != "(" || parts[1].kind() != ")" { return None; }
    Some((name, ReadingContent::Detail {
        label: "Placeholder that panics when reached".into(),
        text: "If this resolves to Rust's standard unimplemented! macro, it marks functionality that has not yet been implemented. When execution reaches this call, it panics instead of returning a normal value. Depending on the program's panic strategy and context, that panic may unwind or abort; it is not a harmless no-op. The spelling alone cannot prove which macro is called because names may be shadowed.".into(),
        definition: None,
        borrow: false,
    }))
}
