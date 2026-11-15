//! Explain a call through a trait-qualified associated type path.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, call: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if call.kind() != "call_expression" || call.has_error() { return None; }
    let function = call.child_by_field_name("function")?;
    if function.kind() != "scoped_identifier" { return None; }
    let assoc_path = function.child_by_field_name("path")?;
    if !matches!(assoc_path.kind(), "scoped_identifier" | "scoped_type_identifier") { return None; }
    let bracket = assoc_path.child_by_field_name("path")?;
    if bracket.kind() != "bracketed_type" { return None; }
    let qualified = named_children(bracket).into_iter().find(|part| part.kind() == "qualified_type")?;
    let target = qualified.child_by_field_name("type")?;
    let tr = qualified.child_by_field_name("alias")?;
    let assoc = assoc_path.child_by_field_name("name")?;
    let method = function.child_by_field_name("name")?;
    Some((function, ReadingContent::Detail {
        label: "Call through a trait-associated type".into(),
        text: format!("In `{}`, `<{} as {}>` selects the implementation of trait {} for type {}. The following `::{}` selects its associated item, intended here as a type; `::{}` then names a callable associated item on that type, and the parentheses call it. If the called item is an instance method, its receiver must be supplied explicitly as an argument. The compiler still resolves the names and verifies that the trait implementation, associated type and call are valid; this spelling alone cannot establish them.", src.norm(function), src.norm(target), src.norm(tr), src.norm(tr), src.norm(target), src.norm(assoc), src.norm(method)),
        definition: None, borrow: false,
    }))
}
