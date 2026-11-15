//! Explain `?` without guessing the operand type from spelling.
use crate::reading::Family;
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// `family` is the reader's resolved operation evidence, not a syntactic guess.
pub(crate) fn explain(src: Src<'_>, operand: Node<'_>, family: Option<Family>) -> ReadingContent {
    let behavior = match family {
        Some(Family::Result) => {
            "The analysis identifies Result behavior here. Ok(value) yields value. Err(error) propagates, with conversion through From when the receiving Result context requires and supports it."
        }
        Some(Family::Option) => {
            "The analysis identifies Option behavior here. Some(value) yields value. None propagates absence."
        }
        _ => {
            "The operand's type is not established by syntax alone. If it is the standard Result, Ok(value) yields value and Err(error) propagates. A compatible Result context can convert the error using From. If it is the standard Option, Some(value) yields value and None propagates. Other supported try types can behave differently."
        }
    };
    ReadingContent::Detail {
        label: "Continue or propagate with ?".into(),
        text: format!(
            "Evaluate {} once. The ? operator either yields the successful value of the expression or propagates a residual early. \
             {behavior} Propagation exits the innermost enclosing function, closure, async block, or try block. That context must accept the residual. \
             Result and Option have different residuals and require an explicit conversion between them. The ? operator does not catch panics.",
            src.norm(operand),
        ),
        definition: None,
        borrow: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn resolved_family_changes_explanation_but_unknown_does_not_guess() {
        let text = "fn f() { let x = value?; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src {
            text,
            file: "lib.rs",
            index: &index,
        };
        let offset = text.find("value?").unwrap();
        let start = tree
            .root_node()
            .descendant_for_byte_range(offset, offset + 6)
            .unwrap();
        let node = std::iter::successors(Some(start), |node| node.parent())
            .find(|node| node.kind() == "try_expression")
            .unwrap();
        let operand = node.named_child(0).unwrap();
        let describe = |family| match explain(src, operand, family) {
            ReadingContent::Detail { text, .. } => text,
            _ => unreachable!(),
        };
        assert!(describe(Some(Family::Result)).contains("analysis identifies Result"));
        assert!(describe(Some(Family::Option)).contains("analysis identifies Option"));
        assert!(describe(None).contains("type is not established"));
        assert!(describe(None).contains("If it is the standard Result"));
    }
}
