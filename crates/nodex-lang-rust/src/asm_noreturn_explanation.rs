//! Explain the `noreturn` option in a function-scope `asm!` invocation.
//! Rust Reference: https://doc.rust-lang.org/reference/inline-assembly.html#asmoptionssupported-optionsnoreturn
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
    for pair in tokens.windows(2) {
        if src.text(pair[0]) != "options" || pair[1].kind() != "token_tree" { continue; }
        let options: Vec<_> = children(pair[1]).into_iter().filter(|part| !part.is_extra()).collect();
        for triple in options.windows(3) {
            if !matches!(triple[0].kind(), "(" | ",") || src.text(triple[1]) != "noreturn" || !matches!(triple[2].kind(), ")" | ",") { continue; }
            output.push((triple[1], ReadingContent::Detail {
                label: "Assembly does not fall through".into(),
                text: "If this invocation resolves to Rust's standard `asm!` macro, `options(noreturn)` promises that execution will not fall through the end of the assembly code; doing so is undefined behavior, so the programmer must uphold this unsafe obligation. The assembly may still jump to a `label` block. If a label block completes with unit, the asm expression can return `()` through it; otherwise its type is `!` (never). Local variables in scope are not dropped before the assembly begins. The macro spelling alone does not establish its identity or prove that these safety requirements are met.".into(),
                definition: None,
                borrow: false,
            }));
        }
    }
    output
}
