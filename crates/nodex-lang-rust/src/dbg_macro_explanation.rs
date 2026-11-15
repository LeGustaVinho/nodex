//! Explain a single-expression `dbg!` without assuming macro resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    let path: String = src.text(name).chars().filter(|ch| !ch.is_whitespace()).collect();
    if !matches!(path.as_str(), "dbg" | "std::dbg" | "::std::dbg") { return None; }
    let body = named_children(node).into_iter().find(|child| child.kind() == "token_tree")?;
    let parts: Vec<_> = children(body).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")" | "[" | "]" | "{" | "}")).collect();
    if parts.is_empty() { return None; }
    // An optional single trailing comma is accepted; other top-level commas
    // are left to the separate multiple-argument explanation.
    if parts.iter().enumerate().any(|(i, part)| part.kind() == "," && i + 1 != parts.len()) { return None; }
    Some((name, ReadingContent::Detail {
        label: "Debug a value with dbg!".into(),
        text: "If this invocation resolves to Rust's standard dbg! macro, its single expression is evaluated once. The macro writes the source location, expression and Debug-formatted value to standard error, then returns that same value to the surrounding code. Passing a value can move it unless it is Copy; passing a reference avoids moving the original value. The value must support Debug formatting. This diagnostic output is not limited to debug builds. Macro names can be shadowed, so the spelling alone does not prove this invocation resolves to the standard macro.".into(),
        definition: None,
        borrow: false,
    }))
}
