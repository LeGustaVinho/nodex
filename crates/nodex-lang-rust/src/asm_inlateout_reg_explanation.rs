//! Explain a same-place `inlateout(reg)` operand in an `asm!` token tree.
//! Rust Reference: https://doc.rust-lang.org/reference/inline-assembly.html#operand-type
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(name) = node.child_by_field_name("macro") else { return Vec::new(); };
    if !matches!(src.norm(name).as_str(), "asm" | "core::arch::asm" | "std::arch::asm") { return Vec::new(); }
    let Some(tree) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    if tree.has_error() { return Vec::new(); }
    let tokens: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    let mut output = Vec::new();
    for window in tokens.windows(5) {
        let [before, kind, registers, place, after] = window else { continue; };
        if !matches!(before.kind(), "," | "(") || src.text(*kind) != "inlateout" || registers.kind() != "token_tree" || src.norm(*registers).replace(char::is_whitespace, "") != "(reg)" || place.kind() != "identifier" || !matches!(after.kind(), "," | ")") { continue; }
        output.push((*kind, ReadingContent::Detail {
            label: "Late input/output register".into(),
            text: format!("If this invocation resolves to Rust's standard `asm!` macro, `inlateout(reg) {0}` uses {0} as both input and output: the selected register starts with its value, and the register's final value is written back to that same mutable place after the assembly. `reg` is a register class; the compiler chooses a suitable register for the target rather than this syntax naming a specific one. Like `inout`, input and output use the same register, but `inlateout` allows the compiler to reuse a register also used by an input when permitted. The assembly must not overwrite the output before all potentially overlapping inputs have been read. Inline assembly requires the surrounding unsafe obligations to be upheld; the written macro name alone does not prove its identity or the assembly's correctness.", src.norm(*place)),
            definition: None,
            borrow: false,
        }));
    }
    output
}
