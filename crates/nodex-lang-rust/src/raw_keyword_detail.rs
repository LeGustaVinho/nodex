//! Explain raw identifiers appearing in executable Rust syntax.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if !matches!(node.kind(), "identifier" | "type_identifier" | "field_identifier" | "shorthand_field_identifier" | "shorthand_field_initializer") { return None; }
    if node.parent().is_some_and(|parent| matches!(parent.kind(), "lifetime" | "label" | "metavariable")) { return None; }
    let spelling = src.text(node);
    let name = spelling.strip_prefix("r#")?;
    if name.is_empty() || matches!(name, "_" | "crate" | "self" | "Self" | "super") { return None; }
    Some((node, ReadingContent::Detail {
        label: "Use a raw identifier".into(),
        text: format!("{spelling} is an identifier spelled with Rust's raw prefix r#. The name is {name}; the prefix is syntax, not part of that name. It lets an eligible keyword be used as a name and can also be used on ordinary names. It does not turn the name into a keyword operation, and this spelling alone does not determine which declaration it resolves to. The special names r#self, r#Self, r#super, r#crate and r#_ are reserved rather than usable raw identifiers."),
        definition: None, borrow: false,
    }))
}
