//! Explain a non-ASCII identifier in a simple local binding.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" || node.has_error() { return None; }
    let pattern = node.child_by_field_name("pattern")?;
    if pattern.kind() != "identifier" { return None; }
    let name = src.norm(pattern);
    if name.is_ascii() { return None; }
    let value = node.child_by_field_name("value")?;
    Some((pattern, ReadingContent::Detail {
        label: "Unicode local identifier".into(),
        text: format!("`let {} = {};` introduces a local binding named `{}` and initializes it with the value of `{}`. Rust identifiers can contain permitted Unicode characters: `{}` here is a name, not a Rust keyword or operation, and a later use of the same name refers to this binding within its scope. The identifier must obey Rust's Unicode identifier rules; names are normalized to NFC for comparison. The initializer determines the value and its type is inferred when possible. Without `mut` the binding cannot normally be reassigned; ownership and borrowing rules still apply. The name's spelling by itself does not determine the value or meaning of the program.", name, src.norm(value), name, src.norm(value), name),
        definition: None, borrow: false,
    }))
}
