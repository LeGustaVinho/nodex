//! Explain label operands in an inline assembly invocation.
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
        let (keyword, block) = match segment {
            [keyword, block] => (*keyword, *block),
            [name, equals, keyword, block] if name.kind() == "identifier" && equals.kind() == "=" => (*keyword, *block),
            _ => continue,
        };
        if src.text(keyword) != "label" || block.kind() != "token_tree" || !src.text(block).starts_with('{') { continue; }
        result.push((keyword, ReadingContent::Detail {
            label: "Assembly branch to Rust block".into(),
            text: "If this invocation resolves to Rust's standard asm! macro, label { ... } supplies a Rust block as an assembly operand. A corresponding positional or named placeholder in the assembly template is replaced with the block's address, so assembly instructions can branch to it; the block runs only if control reaches it. The block must produce () or never return. When it finishes normally, the asm! expression returns. A label block creates a new safety context: unsafe operations inside need their own unsafe block, even when the asm! invocation is inside one. With options(noreturn), the assembly must not fall through, but it may branch to a label whose block returns (). Target-specific branch restrictions still apply. The spelling alone does not prove which macro is resolved or whether a branch occurs.".into(),
            definition: None,
            borrow: false,
        }));
    }
    result
}
