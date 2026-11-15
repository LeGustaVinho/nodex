//! Explain `?` without guessing the operand type from spelling.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;
use crate::reading::Family;

/// `family` is the reader's resolved operation evidence, not a syntactic guess.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>, family: Option<Family>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "try_expression" || node.has_error() { return None; }
    let operand = named_children(node).into_iter().find(|part| !part.is_extra())?;
    let question = children(node).into_iter().find(|part| part.kind() == "?")?;
    let behavior = match family {
        Some(Family::Result) => "The analysis identifies Result behavior here: Ok(value) continues with value; Err(error) leaves the nearest compatible propagation boundary. In a Result-returning context the error can be converted using From when required and permitted.",
        Some(Family::Option) => "The analysis identifies Option behavior here: Some(value) continues with value; None leaves the nearest compatible propagation boundary with absence.",
        _ => "The operand's type is not established here. If it is the standard Result, Ok(value) continues with value and Err(error) propagates; a compatible Result context can convert the error using From. If it is the standard Option, Some(value) continues and None propagates. Other supported try types can behave differently.",
    };
    Some((operand, question, ReadingContent::Detail {
        label: "Continue or propagate with ?".into(),
        text: format!("Evaluate {} first, then ? either continues with its successful value or propagates a residual early. {} The destination is the nearest compatible boundary, such as a function, closure, async block or try block, rather than necessarily the outermost function. Result and Option have different residuals; they do not automatically convert into each other. ? is not an exception handler and does not catch panics.", src.norm(operand), behavior),
        definition: None,
        borrow: false,
    }))
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
        let src = Src { text, file: "lib.rs", index: &index };
        let offset = text.find("value?").unwrap();
        let start = tree.root_node().descendant_for_byte_range(offset, offset + 6).unwrap();
        let node = std::iter::successors(Some(start), |node| node.parent()).find(|node| node.kind() == "try_expression").unwrap();
        let describe = |family| match explain(src, node, family).unwrap().2 { ReadingContent::Detail { text, .. } => text, _ => unreachable!() };
        assert!(describe(Some(Family::Result)).contains("analysis identifies Result"));
        assert!(describe(Some(Family::Option)).contains("analysis identifies Option"));
        assert!(describe(None).contains("type is not established"));
        assert!(describe(None).contains("If it is the standard Result"));
    }
}
