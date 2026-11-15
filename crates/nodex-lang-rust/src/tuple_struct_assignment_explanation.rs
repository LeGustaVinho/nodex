//! Explain destructuring assignments using a tuple-like constructor path.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "assignment_expression" || node.has_error() { return None; }
    let left = node.child_by_field_name("left")?;
    if left.kind() != "call_expression" { return None; }
    let constructor = left.child_by_field_name("function")?;
    if !matches!(constructor.kind(), "identifier" | "scoped_identifier") { return None; }
    let arguments = left.child_by_field_name("arguments")?;
    let places = named_children(arguments);
    if places.len() != 2 || places.iter().any(|p| p.kind() != "identifier") { return None; }
    let right = node.child_by_field_name("right")?;
    Some((left, ReadingContent::Detail {
        label: "Assign tuple-struct fields".into(),
        text: format!("In `{}` the left side is a destructuring assignment, not a function call. If `{}` resolves to a tuple struct or an otherwise valid irrefutable tuple-like variant pattern, Rust evaluates `{}` and assigns its first and second fields to the existing places `{}` and `{}` respectively. This does not declare new variables; the targets must be assignable, and the value and pattern must satisfy Rust's type and irrefutability rules. The written name alone does not prove what it resolves to or that the assignment compiles.", src.norm(node), src.norm(constructor), src.norm(right), src.norm(places[0]), src.norm(places[1])),
        definition: None, borrow: false,
    }))
}
