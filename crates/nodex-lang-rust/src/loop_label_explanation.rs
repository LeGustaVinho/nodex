//! Explain labels that identify loops for control-flow jumps.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "loop_expression" || node.has_error() { return None; }
    let label = named_children(node).into_iter().find(|part| part.kind() == "label")?;
    let written = src.norm(label);
    let name = written.trim_end_matches(':');
    if !name.starts_with('\'') || name.len() < 2 { return None; }
    Some((label, ReadingContent::Detail {
        label: "Name this loop".into(),
        text: format!("`{name}:` labels this loop. A `break {name}` inside nested loops exits this labeled loop, and `continue {name}` starts its next iteration. An unlabeled break or continue normally targets the innermost loop. The apostrophe is part of label syntax here; this name is not a lifetime or a runtime value. A label does not change when the loop runs by itself."),
        definition: None,
        borrow: false,
    }))
}
