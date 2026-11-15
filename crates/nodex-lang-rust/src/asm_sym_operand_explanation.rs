//! Explain direct symbol operands in an assembly macro invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(name) = node.child_by_field_name("macro") else { return Vec::new(); };
    let path: String = src.text(name).chars().filter(|ch| !ch.is_whitespace()).collect();
    if !matches!(path.as_str(), "asm" | "core::arch::asm" | "::core::arch::asm" | "std::arch::asm" | "::std::arch::asm") { return Vec::new(); }
    let Some(tree) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    let parts: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() < 2 { return Vec::new(); }
    let mut result = Vec::new();
    for segment in parts[1..parts.len()-1].split(|part| part.kind() == ",").skip(1) {
        let start = if segment.len() >= 2 && segment[0].kind() == "identifier" && segment[1].kind() == "=" { 2 } else { 0 };
        let [keyword, target] = &segment[start..] else { continue; };
        if src.text(*keyword) != "sym" || target.kind() != "identifier" { continue; }
        result.push((*keyword, ReadingContent::Detail {
            label: "Assembly symbol operand".into(),
            text: format!("If this invocation resolves to Rust's standard `asm!` macro, `sym {}` supplies the linker symbol name for the referenced item for substitution into an assembly template placeholder. The path must resolve to a function or static item; its spelling alone does not establish which. The compiler supplies the item's mangled symbol name without relocation modifiers. The operand itself does not call a function or load a value: any instruction using the substituted symbol determines that behavior. Assembly syntax and correctness depend on the target, and using inline assembly requires an unsafe context. The macro spelling alone does not prove resolution.", src.text(*target)),
            definition: None, borrow: false,
        }));
    }
    result
}
