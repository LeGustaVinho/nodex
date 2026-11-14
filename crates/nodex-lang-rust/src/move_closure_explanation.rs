//! Explain the `move` capture mode of a closure.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(_src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "closure_expression" { return None; }
    if children(node).iter().any(|part| part.kind() == "async") { return None; }
    let keyword = children(node).into_iter().find(|part| part.kind() == "move")?;
    Some((keyword, ReadingContent::Detail {
        label: "Capture closure environment by value".into(),
        text: "move makes the closure capture the outer values it actually uses by value when the closure is created; the parameter between | characters is supplied later when the closure is called. A captured value implementing Copy is copied, while a non-Copy value is moved and the original place may no longer be usable. If the captured value is itself a reference, the closure captures that reference, not ownership of the data it points to; the reference must still be valid for its use. move alone does not prove a closure can be called only once: that depends on what its body does with the captures.".into(),
        definition: None,
        borrow: false,
    }))
}
