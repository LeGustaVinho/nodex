//! Explain a tuple pattern in the header of a `for` loop.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "for_expression" || node.has_error() { return None; }
    let pattern = node.child_by_field_name("pattern")?;
    if pattern.kind() != "tuple_pattern" { return None; }
    let iterable = node.child_by_field_name("value")?;
    Some((pattern, ReadingContent::Detail {
        label: "Split each item in a for loop".into(),
        text: format!("Evaluate {} once and obtain an iterator using IntoIterator. On each iteration, ask it for the next item; when it is exhausted, leave the loop. The pattern {} splits each item by tuple position: in (a, b), a binds the first element and b the second. The pattern must match every item, and the names it introduces are available inside the loop body. Bindings copy or move values according to their types, unless a reference pattern borrows them; whether the collection itself is moved or borrowed depends on the iterator expression and its IntoIterator implementation.", src.norm(iterable), src.norm(pattern)),
        definition: None,
        borrow: false,
    }))
}
