//! Explain matching two shared reference layers in a let pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" || node.has_error() { return None; }
    let outer = node.child_by_field_name("pattern").filter(|n| n.kind() == "reference_pattern")?;
    let inner = named_children(outer).into_iter().find(|n| n.kind() == "reference_pattern")?;
    if [outer, inner].iter().any(|n| children(*n).iter().any(|c| c.kind() == "mutable_specifier")) { return None; }
    let binding = named_children(inner).into_iter().find(|n| n.kind() == "identifier")?;
    let value = node.child_by_field_name("value")?;
    let example = if src.norm(value) == "&&42" { " In this example, the expression `&&42` borrows the integer twice; `42` has a primitive integer type and is Copy, so its value is copied into the binding." } else { " The initializer supplies the value being matched; syntax alone does not establish its type or the resulting binding's value." };
    Some((outer, ReadingContent::Detail {
        label: "Match two shared reference layers".into(),
        text: format!("The pattern {} matches two nested shared references: its first `&` matches the outer reference, and its second `&` matches the inner reference. The name {} then matches the value behind both layers. These ampersands are pattern syntax that match existing reference layers; they do not themselves create borrows. In an expression, `&` instead creates a shared borrow.{example} Binding by value copies a Copy value; moving a non-Copy value out through shared references is generally not permitted. Rust checks the pattern against the actual type and ownership rules.", src.norm(outer), src.norm(binding)),
        borrow: false, definition: None,
    }))
}
