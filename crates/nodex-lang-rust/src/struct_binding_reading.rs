//! Explanation of named-field destructuring in a `let` declaration.

use tree_sitter::Node;

/// Describe the syntax only where the declaration has an actual initializer.
pub(crate) fn explanation(declaration: Node<'_>) -> Option<&'static str> {
    if declaration.kind() != "let_declaration"
        || declaration.child_by_field_name("value").is_none()
        || declaration.child_by_field_name("pattern")?.kind() != "struct_pattern"
    {
        return None;
    }
    Some("Evaluate the expression after =, then take the named fields from that value into local variables. For example, Point { x, y } binds x to the x field and y to the y field; each name is shorthand for x: x or y: y. The field values are copied or moved according to their types; a reference pattern can borrow them instead.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(source: &str) -> tree_sitter::Tree {
        crate::parse(source).expect("parse Rust")
    }

    fn find_let(node: Node<'_>) -> Option<Node<'_>> {
        if node.kind() == "let_declaration" {
            return Some(node);
        }
        let mut cursor = node.walk();
        node.named_children(&mut cursor).find_map(find_let)
    }

    #[test]
    fn explains_struct_destructuring_without_claiming_all_patterns_are_structs() {
        let tree = declaration("fn f(p: Point) { let Point { x, y } = p; }");
        let text = explanation(find_let(tree.root_node()).unwrap()).unwrap();
        assert!(text.contains("x field"));
        assert!(text.contains("copied or moved"));
        let tree = declaration("fn f(pair: (i32, i32)) { let (x, y) = pair; }");
        assert!(explanation(find_let(tree.root_node()).unwrap()).is_none());
    }
}
