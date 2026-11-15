//! Explain the distinguishing comma of a one-element tuple binding.
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "let_declaration" || node.has_error() || node.child_by_field_name("value").is_none() { return None; }
    let pattern = node.child_by_field_name("pattern")?;
    if pattern.kind() != "tuple_pattern" { return None; }
    let elements = named_children(pattern);
    if elements.len() != 1 || elements[0].kind() == "remaining_field_pattern" || src.text(elements[0]) == ".." { return None; }
    let commas: Vec<_> = children(pattern).into_iter().filter(|child| child.kind() == ",").collect();
    if commas.len() != 1 { return None; }
    Some((commas[0], format!("In the pattern {}, the comma makes this a one-element tuple pattern. It matches a tuple with one element and applies {} to that element; for `(a,)`, the name a receives the element. Without the comma, `(a)` only groups the pattern a and does not require a tuple. In `let (a,): (i32,) = (1,);`, the comma likewise marks a one-element tuple in the type and in the value expression: the annotation specifies a tuple containing one signed 32-bit integer, and the right side constructs that tuple. The element is copied or moved when bound by value depending on its type; the compiler checks the pattern, type and value agree.", src.norm(pattern), src.norm(elements[0]))))
}
