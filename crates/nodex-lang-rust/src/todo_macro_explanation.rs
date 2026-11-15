//! Explain a no-argument `todo!()` invocation without assuming macro identity.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    if src.norm(node.child_by_field_name("macro")?) != "todo" { return None; }
    let tree = named_children(node).into_iter().find(|child| child.kind() == "token_tree")?;
    if tree.has_error() { return None; }
    let tokens: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    if !matches!(tokens.as_slice(), [left, right] if left.kind() == "(" && right.kind() == ")") { return None; }
    let bang = children(node).into_iter().find(|part| part.kind() == "!")?;
    Some((bang, ReadingContent::Detail {
        label: "Placeholder macro invocation".into(),
        text: "`todo!()` invokes a macro with no arguments; the `!` marks macro syntax and the call is expanded during compilation. If `todo!` resolves to Rust's standard macro, it marks unfinished code and panics when execution reaches it instead of returning a value. Its presence does not by itself prevent compilation, even if the call is reachable. Panic behavior may unwind or abort according to the program configuration. A different macro can have the same name, so the spelling alone does not prove this behavior.".into(),
        definition: None,
        borrow: false,
    }))
}
