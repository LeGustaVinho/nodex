//! Explain the x86 AT&T syntax option in an `asm!` invocation.
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
        let [name, options] = segment else { continue; };
        if src.text(*name) != "options" || options.kind() != "token_tree" || !src.text(*options).starts_with('(') { continue; }
        for option in named_children(*options).into_iter().filter(|part| part.kind() == "identifier" && src.text(*part) == "att_syntax") {
            result.push((option, ReadingContent::Detail {
                label: "Use AT&T assembly syntax".into(),
                text: "If this invocation resolves to Rust's standard asm! macro, options(att_syntax) selects the assembler's AT&T syntax mode instead of the default Intel syntax mode on x86-family targets. Register operands are substituted with a leading % in this mode. This option is valid only for x86-family targets; the compiler checks target support. It changes how the assembly template is interpreted, not the CPU architecture or the safety obligations of inline assembly. The name asm! alone does not prove which macro is resolved.".into(),
                definition: None,
                borrow: false,
            }));
        }
    }
    result
}
