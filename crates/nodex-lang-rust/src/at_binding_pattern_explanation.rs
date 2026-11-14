//! Explain the `name @ pattern` binding pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "captured_pattern" || node.has_error() { return None; }
    let at = children(node).into_iter().find(|part| part.kind() == "@")?;
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    let [name, nested] = parts.as_slice() else { return None; };
    Some((at, ReadingContent::Detail {
        label: "Bind the value matched by a pattern".into(),
        text: format!("In {}, the pattern after @ checks the same value while the binding before @ gives the entire matched value a name. On a successful match, {} becomes available in the applicable scope; {} can also bind names inside the value. The binding mode and value's type determine whether a binding borrows, copies or moves; Rust checks that uses of the whole value and its parts are compatible.", src.norm(node), src.norm(*name), src.norm(*nested)),
        definition: None,
        borrow: false,
    }))
}
