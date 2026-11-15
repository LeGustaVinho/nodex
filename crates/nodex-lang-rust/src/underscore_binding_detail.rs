//! Explain an underscore-prefixed local name in a simple `let` pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" || node.has_error() { return None; }
    let pattern = node.child_by_field_name("pattern")?;
    if pattern.kind() != "identifier" { return None; }
    let name = src.text(pattern);
    if !name.starts_with('_') || name.len() == 1 { return None; }
    let value = node.child_by_field_name("value")?;
    Some((pattern, ReadingContent::Detail {
        label: "Underscore-prefixed local binding".into(),
        text: format!("If `{name}` resolves as a new local binding, `let {name} = {}` evaluates the initializer and binds its result to the name `{name}`, which can still be used later. Starting the name with `_` normally suppresses the `unused_variables` diagnostic for that binding; it does not assert the variable is unused, change the initializer's effects, or disable unrelated lints. Unlike a standalone `_` wildcard pattern, a named binding can hold a value until its usual drop point; whether the value is moved or copied depends on its type and pattern. Rust name resolution and other compiler checks still apply.", src.norm(value)),
        borrow: false, definition: None,
    }))
}
