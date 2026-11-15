//! Explain a macro invocation using a square-bracket token group.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    let tree = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    if tree.has_error() { return None; }
    let parts = children(tree);
    let open = parts.iter().copied().find(|part| part.kind() == "[" && !part.is_missing())?;
    if parts.first()? != &open || !parts.iter().any(|part| part.kind() == "]" && !part.is_missing()) { return None; }
    Some((open, ReadingContent::Detail {
        label: "Macro invocation with brackets".into(),
        text: format!("{}! invokes a macro; `!` marks the invocation, and `[` and `]` delimit the group of input tokens written here. The compiler expands the macro during compilation according to the resolved macro and its rules. The tokens inside the brackets are not necessarily an array or ordinary function arguments, and the written name alone does not establish what code the macro produces. Other macros may use parentheses or braces, but the accepted input and meaning depend on the macro.", src.norm(name)),
        definition: None,
        borrow: false,
    }))
}
