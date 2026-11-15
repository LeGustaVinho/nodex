//! Explain `panic!("message")` while keeping macro resolution explicit.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let path = node.child_by_field_name("macro")?;
    let macro_name = src.norm(path);
    if macro_name.rsplit("::").next()? != "panic" { return None; }
    let group = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    let tokens: Vec<_> = children(group).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() != 3 && tokens.len() != 4 { return None; }
    if tokens[0].kind() != "(" || tokens[1].kind() != "string_literal" || tokens.last()?.kind() != ")"
        || (tokens.len() == 4 && tokens[2].kind() != ",") { return None; }
    let message = tokens[1];
    if !src.text(message).starts_with('"') { return None; }
    Some((message, ReadingContent::Detail {
        label: "Possible panic".into(),
        text: format!("If {}! resolves to Rust's standard panic! macro, executing the expanded code starts a panic with the written message and this path does not continue normally. What happens next depends on the panic strategy and runtime context: with unwinding, the stack is unwound and applicable destructors run; an unwinding panic can sometimes be caught. With abort, the process terminates without unwinding that stack. The macro is expanded during compilation, but the panic starts only if the resulting code executes. A locally defined or imported macro can share this name and might even return normally, so spelling alone does not prove standard panic behavior.", macro_name),
        definition: None,
        borrow: false,
    }))
}
