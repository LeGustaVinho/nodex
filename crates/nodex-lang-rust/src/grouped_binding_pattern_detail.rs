//! Explain parentheses that group a let binding without making a tuple.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::{blueprint_types::rust_grouped_pattern, syntax::Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return None; }
    let pattern = node.child_by_field_name("pattern")?;
    let mut inner = rust_grouped_pattern(pattern)?;
    while let Some(nested) = rust_grouped_pattern(inner) { inner = nested; }
    if inner.kind() != "identifier" { return None; }
    let binding = src.norm(inner);
    Some((pattern, ReadingContent::Detail {
        label: "Grouped binding pattern".into(),
        text: format!("The parentheses in `{}` group the binding pattern {binding}; without a comma they do not make a one-element tuple pattern. This binds {binding} to the entire value on the right, just as `let {binding} = ...` does. By contrast `( {binding}, )` includes a comma and destructures a one-element tuple. A by-value binding can copy or move depending on the value's type, which this syntax alone does not establish.", src.norm(pattern)),
        definition: None, borrow: false,
    }))
}
