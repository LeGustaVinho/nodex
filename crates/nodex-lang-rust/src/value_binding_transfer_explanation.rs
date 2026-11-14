//! Explain a simple by-value binding without guessing whether its source is `Copy`.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Return a detail for `let name = source`, highlighting the source name.
/// A bare name can also resolve to an item (for example a constant), so the
/// explanation explicitly conditions the move/copy rules on local resolution.
pub(crate) fn explain<'t>(src: Src<'_>, declaration: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if declaration.kind() != "let_declaration" || declaration.child_by_field_name("alternative").is_some() {
        return None;
    }
    let target = declaration.child_by_field_name("pattern")?;
    let source = declaration.child_by_field_name("value")?;
    if target.kind() != "identifier" || source.kind() != "identifier" { return None; }
    let from = src.norm(source);
    let to = src.norm(target);
    Some((source, ReadingContent::Detail {
        label: "Bind a value from another name".into(),
        text: format!("Evaluate {from} and bind the resulting value to the new variable {to}. If {from} names a local variable, Rust copies the value when its type implements Copy. Otherwise, if moving is permitted, it moves ownership to {to}; the moved value cannot be used through {from} again unless that binding is reinitialized. A type may own resources that are released when its owner is dropped. The name alone does not establish whether this is a local variable or whether its type implements Copy, so this view cannot decide which case applies without more information."),
        definition: None,
        borrow: false,
    }))
}
