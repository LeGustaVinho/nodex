//! Explain a closure with no explicit parameters, such as `|| 42`.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "closure_expression" || node.has_error() { return None; }
    // Async closures return a future rather than the body's result directly.
    if children(node).iter().any(|child| child.kind() == "async") { return None; }
    let parameters = node.child_by_field_name("parameters")?;
    if named_children(parameters).into_iter().any(|child| !child.is_extra()) { return None; }
    let body = node.child_by_field_name("body")?;
    Some((body, ReadingContent::Detail {
        label: "Closure with no parameters".into(),
        text: format!("The empty pair of bars || declares an anonymous function value that takes no arguments. Creating it does not execute its body. Each time the closure is called, it evaluates {}; if the body completes normally, its value becomes the call's result. For example, a closure written || 42 returns 42 when called. The closure may also capture values from its surroundings, according to how its body uses them.", src.norm(body)),
        definition: None,
        borrow: false,
    }))
}
