//! Explain simple split `inout(reg) input => output` asm operands.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(name) = node.child_by_field_name("macro") else { return Vec::new(); };
    let path: String = src.text(name).chars().filter(|ch| !ch.is_whitespace()).collect();
    if !matches!(path.as_str(), "asm" | "core::arch::asm" | "::core::arch::asm" | "std::arch::asm" | "::std::arch::asm") { return Vec::new(); }
    let Some(body) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    let parts: Vec<_> = children(body).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")" | "[" | "]" | "{" | "}")).collect();
    let mut result = Vec::new();
    for segment in parts.split(|part| part.kind() == ",") {
        let [keyword, register, input, arrow, output] = segment else { continue; };
        if src.text(*keyword) != "inout" || register.kind() != "token_tree" || src.norm(*register) != "(reg)"
            || input.kind() != "identifier" || arrow.kind() != "=>" || output.kind() != "identifier" { continue; }
        result.push((*keyword, ReadingContent::Detail {
            label: "Read and write one assembly register".into(),
            text: format!("If this invocation resolves to Rust's standard asm! macro, inout(reg) {} => {} is a split input/output operand. Before the assembly instructions execute, {} provides the initial value in a compiler-selected register from the reg class. When the assembly completes normally, the resulting value in that same register is written to {}. The => separates the input expression from the output place; it is not a Rust function call. The input and output must satisfy the target's register and type constraints. Inline assembly requires appropriate unsafe handling, and the author must ensure the instructions and declared operands are correct. Macro spelling alone does not prove which asm! macro is resolved.", src.norm(*input), src.norm(*output), src.norm(*input), src.norm(*output)),
            definition: None,
            borrow: false,
        }));
    }
    result
}
