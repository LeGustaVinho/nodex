//! Explain the inline assembly example without resolving the macro ourselves.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let path = node.child_by_field_name("macro")?;
    let name: String = src.text(path).chars().filter(|c| !c.is_whitespace()).collect();
    if !matches!(name.as_str(), "core::arch::asm" | "::core::arch::asm") { return None; }
    let tokens = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    let text = src.text(tokens).trim();
    let argument = text.strip_prefix('(')?.strip_suffix(')')?.trim();
    let argument = argument.strip_suffix(',').unwrap_or(argument).trim();
    if argument != "\"nop\"" { return None; }
    Some((path, ReadingContent::Detail {
        label: "Inline assembly no-operation instruction".into(),
        text: "If this path resolves to Rust's standard `core::arch::asm!` macro, it embeds the assembly template `nop` into the surrounding function. On targets where `nop` is a valid no-operation instruction it performs no normal computation; the exact instruction and assembly syntax depend on the target architecture, and other targets may reject it. Inline assembly requires an unsafe context: the programmer must ensure that the instructions obey Rust's assembly rules and the target's requirements. The spelling alone does not prove which macro the path resolves to.".into(),
        definition: None, borrow: false,
    }))
}
