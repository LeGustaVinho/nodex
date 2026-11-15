//! Explain a discarded register output in an apparent asm! invocation.
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
    let mut found = Vec::new();
    for group in parts.split(|part| part.kind() == ",") {
        let [direction, register, destination] = group else { continue; };
        if src.text(*direction) != "out" || register.kind() != "token_tree" || src.text(*destination) != "_" { continue; }
        let written_register: String = src.text(*register).chars().filter(|ch| !ch.is_whitespace()).collect();
        if written_register != "(reg)" { continue; }
        found.push((*destination, ReadingContent::Detail {
            label: "Discard assembly register output".into(),
            text: "If this resolves to Rust's standard `asm!`, `out(reg) _` declares an output register from the target's `reg` class and discards its final value instead of writing it to a Rust variable. The register's initial contents are undefined; it is not an input. The operand can tell the compiler that the assembly writes this register even though the result is unused. The assembly must meet the target and safety requirements; the spelling alone does not prove macro resolution or validate the assembly.".into(),
            definition: None, borrow: false,
        }));
    }
    found
}
