//! Explain `option_env!("NAME")` without assuming macro name resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    let written = src.norm(name);
    if written.split("::").last()? != "option_env" { return None; }
    let bang = children(node).into_iter().find(|part| part.kind() == "!" && !part.is_missing())?;
    let tree = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    if tree.has_error() { return None; }
    let parts: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    let [open, literal, close] = parts.as_slice() else { return None; };
    if open.kind() != "(" || close.kind() != ")" || literal.kind() != "string_literal" { return None; }
    Some((bang, ReadingContent::Detail {
        label: "Optional compile-time environment variable".into(),
        text: format!("{}!({}) is a macro invocation. If it resolves to Rust's standard option_env! macro, the compiler looks up the environment variable named by this string while compiling and expands to Some(value) when it is set, or None when it is absent. The value has type Option<&'static str>; a missing variable does not itself cause a compile error. This is not a lookup when the program runs: the value reflects the build environment and can change after a rebuild. The written name alone does not prove which macro was resolved.", written, src.norm(*literal)),
        definition: None,
        borrow: false,
    }))
}
