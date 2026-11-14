//! Explain `continue 'label` without assuming the loop kind.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "continue_expression" { return None; }
    let label = named_children(node).into_iter().find(|part| part.kind() == "label")?;
    let name = src.norm(label);
    Some((label, ReadingContent::Detail {
        label: "Continue a named loop".into(),
        text: format!("continue {name} skips the rest of the current iteration, including any enclosing inner loops, and resumes the enclosing loop marked {name}. A while loop checks its condition again; a for loop requests the next item; a loop starts its next iteration. This does not exit the named loop. The apostrophe is part of the label syntax, not a string or character literal. The label must name an enclosing loop in scope."),
        definition: None,
        borrow: false,
    }))
}
