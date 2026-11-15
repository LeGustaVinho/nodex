//! Explain ABI clobbers in an asm-shaped macro invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(path) = node.child_by_field_name("macro") else { return Vec::new(); };
    let macro_name = src.norm(path);
    if macro_name.rsplit("::").next() != Some("asm") { return Vec::new(); }
    let Some(group) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    let parts: Vec<_> = children(group).into_iter().filter(|part| !part.is_extra()).collect();
    let mut result = Vec::new();
    for pair in parts.windows(2) {
        if src.text(pair[0]) != "clobber_abi" || pair[1].kind() != "token_tree" { continue; }
        let arguments: Vec<_> = children(pair[1]).into_iter().filter(|part| !part.is_extra()).collect();
        if arguments.first().map(|n| n.kind()) != Some("(") || arguments.last().map(|n| n.kind()) != Some(")") { continue; }
        if !arguments.iter().any(|part| part.kind() == "string_literal" && src.text(*part) == "\"C\"") { continue; }
        result.push((pair[0], ReadingContent::Detail {
            label: "Assembly ABI register clobbers".into(),
            text: format!("If {}! resolves to Rust's standard `asm!`, `clobber_abi(\"C\")` tells the compiler that registers not fully preserved by the target's C calling convention may be overwritten by this assembly. Rust adds implicit late output clobbers for those registers, except where an explicit register output takes precedence; the exact set depends on the target. This is useful when the assembly calls a C-ABI function. It does not itself make the call, save registers, verify the assembly or satisfy the other calling-convention and safety requirements. When using `clobber_abi`, outputs must name explicit registers; multiple ABIs contribute the union of their clobbers. The spelling alone does not prove macro resolution or target support.", macro_name),
            definition: None, borrow: false,
        }));
    }
    result
}
