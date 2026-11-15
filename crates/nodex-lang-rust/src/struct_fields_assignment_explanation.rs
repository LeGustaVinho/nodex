//! Explain named fields mapped to existing assignment places.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "assignment_expression" || node.has_error() { return None; }
    let mut left = node.child_by_field_name("left")?;
    while left.kind() == "parenthesized_expression" {
        let parts: Vec<_> = named_children(left).into_iter().filter(|part| !part.is_extra()).collect();
        let [inner] = parts.as_slice() else { return None; };
        left = *inner;
    }
    if left.kind() != "struct_expression" { return None; }
    let path = left.child_by_field_name("name")?;
    let fields = left.child_by_field_name("body")?;
    let fields: Vec<_> = named_children(fields).into_iter().filter(|field| !field.is_extra()).collect();
    if fields.is_empty() { return None; }
    let mut mappings = Vec::new();
    for field in fields {
        if field.kind() != "field_initializer" { return None; }
        let name = field.child_by_field_name("field")?;
        let place = field.child_by_field_name("value")?;
        let valid_place = matches!(place.kind(), "identifier" | "field_expression" | "index_expression")
            || (place.kind() == "unary_expression" && children(place).iter().any(|token| token.kind() == "*"));
        if !valid_place || src.text(place) == "_" { return None; }
        mappings.push(format!("field {} goes to {}", src.norm(name), src.norm(place)));
    }
    let right = node.child_by_field_name("right")?;
    let equals = children(node).into_iter().find(|token| token.kind() == "=")?;
    Some((equals, ReadingContent::Detail {
        label: "Assign named fields to existing places".into(),
        text: format!("This is a destructuring assignment, not construction of a new {} or a let declaration. First evaluate {} once; then take the corresponding fields from that value and write to existing places: {}. The writes occur from left to right; an initialized destination's previous value is dropped as required by assignment. The fields are moved or copied according to their types. Rust requires the right-hand shape and the destinations to be valid, and the destinations must be writable.", src.norm(path), src.norm(right), mappings.join("; ")),
        definition: None,
        borrow: false,
    }))
}
