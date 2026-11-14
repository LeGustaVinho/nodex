//! Beginner-facing explanation of tuple destructuring in `let` statements.
use tree_sitter::Node;

/// Returns the tuple pattern when a let statement actually destructures a tuple.
/// Parenthesized single patterns, `(x)`, are grouping rather than tuple patterns.
pub(crate) fn tuple_let_pattern(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() != "let_declaration" || node.child_by_field_name("value").is_none() {
        return None;
    }
    let pattern = node.child_by_field_name("pattern")?;
    if pattern.kind() != "tuple_pattern" {
        return None;
    }
    let mut cursor = pattern.walk();
    let has_comma = pattern.children(&mut cursor).any(|child| child.kind() == ",");
    // The empty tuple is also a tuple, despite having no comma.
    let mut cursor = pattern.walk();
    let empty = pattern.named_children(&mut cursor).next().is_none();
    (has_comma || empty).then_some(pattern)
}

pub(crate) const TITLE: &str = "Split a tuple into names";
pub(crate) const EXPLANATION: &str = "First evaluate the expression on the right. The parentheses on the left describe a tuple pattern: each position matches the corresponding position of that tuple. For example, in let (a, b) = pair, a receives the first item (pair.0) and b the second (pair.1). The pattern must match the tuple shape; `..` can stand for omitted positions. Values bound by name may be copied or moved depending on their types; reference patterns can instead borrow.";

#[cfg(test)]
mod tests {
    use super::*;
    fn finds(source: &str) -> bool {
        let tree = crate::parse(source).unwrap();
        let mut todo = vec![tree.root_node()];
        while let Some(node) = todo.pop() {
            if tuple_let_pattern(node).is_some() { return true; }
            let mut cursor = node.walk();
            todo.extend(node.named_children(&mut cursor));
        }
        false
    }
    #[test]
    fn recognizes_tuple_let_but_not_grouped_or_other_patterns() {
        assert!(finds("fn f(pair: (i32, i32)) { let (a, b) = pair; }"));
        assert!(finds("fn f(pair: (i32,)) { let (a,) = pair; }"));
        assert!(finds("fn f() { let () = (); }"));
        assert!(!finds("fn f(x: i32) { let (a) = x; }"));
        assert!(!finds("fn f(x: i32) { let a = x; }"));
    }
}
