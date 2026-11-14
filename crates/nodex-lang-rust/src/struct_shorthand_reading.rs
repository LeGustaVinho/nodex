//! Explanation of struct field initialization shorthand.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ReadingContent> {
    if node.kind() != "struct_expression" { return None; }
    // A struct-shaped expression can also be a destructuring assignment target.
    let mut child = node;
    while let Some(parent) = child.parent() {
        if parent.kind() == "assignment_expression" {
            if parent.child_by_field_name("left").is_some_and(|left| left.start_byte() <= node.start_byte() && left.end_byte() >= node.end_byte()) {
                return None;
            }
            break;
        }
        if !matches!(parent.kind(), "parenthesized_expression" | "tuple_expression" | "array_expression" | "field_initializer" | "field_initializer_list") { break; }
        child = parent;
    }
    let body = node.child_by_field_name("body")?;
    let fields: Vec<_> = named_children(body).into_iter()
        .filter(|field| field.kind() == "shorthand_field_initializer")
        .map(|field| src.norm(field))
        .collect();
    if fields.is_empty() { return None; }
    let examples = fields.iter().map(|name| format!("{name}: {name}")).collect::<Vec<_>>().join(", ");
    Some(ReadingContent::Detail {
        label: "Initialize fields from namesakes".into(),
        text: format!("The fields {} use shorthand: each name is both a field and the local variable or value used to initialize it. The same fields could be written {examples}. This creates a value with those fields; depending on the field's type, taking each value can copy it or move it.", fields.join(", ")),
        definition: None,
        borrow: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_shorthand_and_excludes_explicit_fields_and_assignment() {
        let text = "fn f() { let p = Point { x, y }; let q = Point { x: 1, y: 2 }; Point { x, y } = p; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let mut nodes = vec![tree.root_node()];
        let mut found = Vec::new();
        while let Some(node) = nodes.pop() {
            if let Some(ReadingContent::Detail { text, .. }) = explain(src, node) { found.push(text); }
            nodes.extend(named_children(node));
        }
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("x: x, y: y"));
    }
}
