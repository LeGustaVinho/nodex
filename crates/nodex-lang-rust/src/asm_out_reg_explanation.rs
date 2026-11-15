//! Explain a simple `out(reg) name` operand in an apparent `asm!` invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(path) = node.child_by_field_name("macro") else { return Vec::new(); };
    if src.norm(path).rsplit("::").next() != Some("asm") { return Vec::new(); }
    let Some(tree) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    let parts: Vec<_> = children(tree).into_iter().filter(|part|
        !part.is_extra() && !matches!(part.kind(), "(" | ")" | "[" | "]" | "{" | "}")).collect();
    let mut result = Vec::new();
    for group in parts.split(|part| part.kind() == ",") {
        let [direction, register, destination] = group else { continue; };
        if src.text(*direction) != "out" || register.kind() != "token_tree" || destination.kind() != "identifier" || src.text(*destination) == "_" { continue; }
        let written_register: String = src.text(*register).chars().filter(|ch| !ch.is_whitespace()).collect();
        if written_register != "(reg)" { continue; }
        result.push((*direction, ReadingContent::Detail {
            label: "Assembly register output".into(),
            text: format!("If this invocation resolves to Rust's standard `asm!` macro, `out(reg) {}` declares an output operand: the compiler selects a register from the target's `reg` class, and when the assembly finishes normally its register contents are written to the destination {}. The register's initial value is undefined; this is not an input containing the previous value of {}. The destination is a place that may initially be uninitialized. The assembly and its operands must uphold the target and safety requirements; the written macro name and operand alone do not prove resolution, validity, or correct assembly behavior.", src.norm(*destination), src.norm(*destination), src.norm(*destination)),
            definition: None,
            borrow: false,
        }));
    }
    result
}
