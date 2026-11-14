//! Explain qualified, named-field construction without guessing name resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// A qualified record can name a variant, but syntax alone does not prove that it does.
/// Return the path separately so its highlighted span can coexist with the general
/// explanation of the complete construction.
pub(crate) fn explain<'a>(src: Src<'_>, node: Node<'a>) -> Option<(Node<'a>, ReadingContent)> {
    if node.kind() != "struct_expression" { return None; }
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        if ancestor.kind() == "assignment_expression" {
            if ancestor.child_by_field_name("left").is_some_and(|left| left.start_byte() <= node.start_byte() && node.end_byte() <= left.end_byte()) { return None; }
            break;
        }
        if matches!(ancestor.kind(), "block" | "function_item") { break; }
    }
    let path = node.child_by_field_name("name")?;
    if !matches!(path.kind(), "scoped_identifier" | "scoped_type_identifier") { return None; }
    let owner = path.child_by_field_name("path")?;
    let variant = path.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    if !named_children(body).into_iter().any(|field| matches!(field.kind(), "field_initializer" | "shorthand_field_initializer")) { return None; }
    let owner = src.norm(owner);
    let variant_node = variant;
    let variant = src.norm(variant);
    Some((variant_node, ReadingContent::Detail {
        label: "Possible named enum variant".into(),
        text: format!("If {owner} resolves to an enum and {variant} is one of its named-field variants, {owner}::{variant} selects that variant and the braces provide its data. The value is a {owner} whose active variant is {variant}; field names identify the data, and each expression after a colon supplies that field's value. This syntax can also identify another kind of record, so the actual meaning depends on name resolution."),
        definition: None,
        borrow: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn qualified_construction_is_conditional_and_assignment_is_excluded() {
        let text = "fn f() { let s = Shape::Circle { radius: 2.0 }; Shape::Circle { radius: r } = s; let p = Point { x: 1 }; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let mut pending = vec![tree.root_node()];
        let mut found = Vec::new();
        while let Some(node) = pending.pop() {
            if let Some((path, ReadingContent::Detail { text, .. })) = explain(src, node) { found.push((src.norm(path), text)); }
            pending.extend(named_children(node));
        }
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "Circle");
        assert!(found[0].1.contains("If Shape resolves to an enum"));
        assert!(found[0].1.contains("active variant is Circle"));
        assert!(found[0].1.contains("depends on name resolution"));
    }
}
