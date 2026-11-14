//! Explain a braced constant expression used as an explicit generic argument.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Recognize a braced argument in a turbofish. A braced block in this position is
/// a const argument; no claim is made about which generic definition it resolves to.
pub(crate) fn explain<'a>(src: Src<'_>, node: Node<'a>) -> Vec<(Node<'a>, ReadingContent)> {
    if node.kind() != "generic_function" { return Vec::new(); }
    let Some(args) = node.child_by_field_name("type_arguments") else { return Vec::new() };
    named_children(args).into_iter().filter_map(|arg| {
        if arg.kind() != "block" { return None; }
        let brace = children(arg).into_iter().find(|part| part.kind() == "{")?;
        Some((brace, ReadingContent::Detail {
            label: "Computed const generic argument".into(),
            text: format!("The ::<...> syntax supplies explicit generic arguments. {} is a braced constant expression used as a const generic argument: Rust checks and evaluates it at compile time, and the resulting value is used when instantiating the item. The braces group the expression as one argument; they do not run a block each time the function is called. The compiler must be able to evaluate it under the applicable const-generic rules. The resolved declaration determines which parameter receives it.", src.norm(arg)),
            definition: None,
            borrow: false,
        }))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn recognizes_braced_argument_but_not_call_body_or_literal_argument() {
        let text = "fn f() { foo::<{ 2 + 2 }>(); foo::<42>(); { 2 + 2 }; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let mut todo = vec![tree.root_node()];
        let mut found = Vec::new();
        while let Some(node) = todo.pop() {
            found.extend(explain(src, node));
            todo.extend(named_children(node));
        }
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(src.text(found[0].0), "{");
        assert!(matches!(&found[0].1, ReadingContent::Detail { text, .. } if text.contains("{ 2 + 2 }") && text.contains("compile time")));
    }
}
