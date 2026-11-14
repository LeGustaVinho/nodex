//! Beginner-facing explanation of a local binding that hides an earlier binding.

use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Recognize straightforward `let name` shadowing using lexical block order.
/// Only report cases established by syntax; semantic resolution is not assumed.
pub(crate) fn explain(src: Src<'_>, declaration: Node<'_>) -> Option<ReadingContent> {
    if declaration.kind() != "let_declaration" {
        return None;
    }
    let name = declaration.child_by_field_name("pattern")?;
    if name.kind() != "identifier" {
        return None;
    }
    let spelling = src.text(name);
    let mut current = declaration;
    while let Some(parent) = current.parent() {
        if parent.kind() == "block" {
            let mut cursor = parent.walk();
            for previous in parent.named_children(&mut cursor).take_while(|n| n.start_byte() < current.start_byte()) {
                if previous.kind() != "let_declaration" {
                    continue;
                }
                if let Some(pattern) = previous.child_by_field_name("pattern")
                    && pattern.kind() == "identifier"
                    && src.text(pattern) == spelling
                {
                    return Some(ReadingContent::Detail {
                        borrow: false,
                        definition: None,
                        label: "A new variable hides an earlier one".into(),
                        text: format!("This `let` creates a new variable named {spelling}; it does not change the earlier variable. The expression on the right is evaluated first, so an occurrence of {spelling} there still refers to the earlier variable. After this declaration, the name refers to the new variable until its scope ends. The new variable can have a different type or mutability."),
                    });
                }
            }
        }
        // Nested functions cannot capture local variables from their enclosing function.
        if matches!(parent.kind(), "function_item" | "function_signature_item") {
            break;
        }
        current = parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    fn flags(text: &str) -> Vec<bool> {
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "shadow.rs", index: &index };
        let mut nodes = vec![tree.root_node()];
        let mut flags = Vec::new();
        while let Some(node) = nodes.pop() {
            if node.kind() == "let_declaration" {
                flags.push(explain(src, node).is_some());
            }
            let mut cursor = node.walk();
            let children: Vec<_> = node.named_children(&mut cursor).collect();
            nodes.extend(children.into_iter().rev());
        }
        flags
    }

    #[test]
    fn recognizes_sequential_and_nested_shadowing_only() {
        assert_eq!(flags("fn f() { let x = 1; let x = x + 1; { let x = 3; } let y = 4; }"), vec![false, true, true, false]);
        assert_eq!(flags("fn f() { { let x = 1; } let x = 2; fn g() { let x = 3; } }"), vec![false, false, false]);
    }
}
