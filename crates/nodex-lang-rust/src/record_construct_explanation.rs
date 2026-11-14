//! Explain record-style construction without requiring Rust syntax knowledge.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ReadingContent> {
    if node.kind() != "struct_expression" { return None; }
    // A record-shaped expression on the left of destructuring assignment is a destination,
    // not a construction. Check enclosing patterns as well as a direct assignment.
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        if ancestor.kind() == "assignment_expression" {
            if ancestor.child_by_field_name("left").is_some_and(|left| left.start_byte() <= node.start_byte() && left.end_byte() >= node.end_byte()) { return None; }
            break;
        }
        if matches!(ancestor.kind(), "block" | "function_item") { break; }
    }
    let name = node.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    let fields = named_children(body);
    if fields.is_empty() || fields.iter().any(|field| field.kind() != "field_initializer") { return None; }
    let mut descriptions = Vec::new();
    for field in fields {
        descriptions.push(format!("{} receives {}", src.norm(field.child_by_field_name("field")?), src.norm(field.child_by_field_name("value")?)));
    }
    Some(ReadingContent::Detail {
        label: "Construct a value with named fields".into(),
        text: format!("{} identifies the value being constructed; the braces supply its named fields. Each field name before : receives the expression after it: {}. Rust writes this construction directly, without a new keyword. The name may identify a struct, a named-field enum variant, or a union; the declaration determines which.", src.norm(name), descriptions.join("; ")),
        borrow: false,
        definition: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;
    #[test]
    fn explains_explicit_fields_only_when_constructing() {
        let text = "fn f() { let p = Point { x: 1.0, y: 2.0 }; Point { x: a, y: b } = p; let q = Point { x, y }; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "points.rs", index: &index };
        let mut pending = vec![tree.root_node()];
        let mut descriptions = Vec::new();
        while let Some(node) = pending.pop() {
            if let Some(ReadingContent::Detail { text, .. }) = explain(src, node) { descriptions.push(text); }
            pending.extend(named_children(node));
        }
        assert_eq!(descriptions.len(), 1);
        assert!(descriptions[0].contains("x receives 1.0"));
        assert!(descriptions[0].contains("y receives 2.0"));
        assert!(descriptions[0].contains("without a new keyword"));
    }
}
