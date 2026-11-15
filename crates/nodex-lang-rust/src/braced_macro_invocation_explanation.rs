//! Explain a macro invocation whose token input is enclosed in braces.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    let tree = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    let opening = children(tree).into_iter().find(|part| part.kind() == "{")?;
    if !src.text(tree).starts_with('{') { return None; }
    Some((opening, ReadingContent::Detail {
        label: "Invoke a macro with braces".into(),
        text: format!("{}! invokes a macro; the `!` distinguishes the invocation from an ordinary function call. The braces delimit the tokens supplied to the macro, not necessarily a runtime code block or evaluated function arguments. During compilation the macro expands those tokens into Rust syntax appropriate for its use. The meaning and any eventual effects depend on the macro's definition and expansion; the written tokens alone do not establish them.", src.norm(name)),
        definition: None,
        borrow: false,
    }))
}
