//! Explanation for a function whose declared result is the never type.

use tree_sitter::Node;

/// Describe `-> !` only when the return type is explicitly the never type.
pub(crate) fn explain(result: Node<'_>) -> Option<&'static str> {
    (result.kind() == "never_type").then_some(
        "`!` is the never type: this function cannot return a value or finish normally. It may run forever, panic, or terminate the process. Execution after a call cannot be reached by a normal return. Unlike `()`, which is a value meaning no meaningful result, `!` has no values.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_an_explicit_never_return() {
        let tree = crate::parse("fn stop() -> ! { loop {} } fn done() -> () {} fn value() -> i32 { 1 }").unwrap();
        let root = tree.root_node();
        let mut cursor = root.walk();
        let results: Vec<_> = root.named_children(&mut cursor)
            .filter_map(|function| function.child_by_field_name("return_type"))
            .map(|result| explain(result).is_some())
            .collect();
        assert_eq!(results, [true, false, false]);
    }
}
