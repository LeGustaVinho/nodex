//! Explain `cfg!(target_os = "linux")` without assuming macro resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    if src.norm(name) != "cfg" { return None; }
    let tree = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    if !src.text(tree).starts_with('(') { return None; }
    let parts: Vec<_> = children(tree).into_iter().filter(|part|
        !part.is_extra() && !matches!(part.kind(), "(" | ")")).collect();
    let [key, equal, value] = parts.as_slice() else { return None; };
    if src.text(*key) != "target_os" || equal.kind() != "=" || src.text(*value) != "\"linux\"" { return None; }
    Some((*key, ReadingContent::Detail {
        label: "Check the compilation target".into(),
        text: "If `cfg!` resolves to Rust's standard configuration macro, `cfg!(target_os = \"linux\")` expands at compile time to the boolean `true` when the compilation target's `target_os` is `linux`, and `false` otherwise. This checks the configured target, which can differ from the machine running the compiler; it is not a runtime operating-system query. Unlike `#[cfg(...)]` on an item, `cfg!` supplies a boolean expression and does not itself remove surrounding branches from type checking. The written macro name alone does not prove which macro it resolves to.".into(),
        definition: None,
        borrow: false,
    }))
}
