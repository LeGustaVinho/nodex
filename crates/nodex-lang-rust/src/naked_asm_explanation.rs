//! Explain naked assembly conditionally on macro resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let path = node.child_by_field_name("macro")?;
    if src.norm(path).rsplit("::").next()? != "naked_asm" { return None; }
    let tokens = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    let open = children(tokens).into_iter().find(|part| !part.is_extra())?;
    if open.kind() != "(" { return None; }
    let template = named_children(tokens).into_iter().find(|part| !part.is_extra())?;
    if !matches!(template.kind(), "string_literal" | "raw_string_literal") || template.has_error() { return None; }
    Some((open, ReadingContent::Detail {
        label: "Naked function assembly".into(),
        text: format!("If {} resolves to Rust's standard `core::arch::naked_asm!`, {} is an assembly template used to write the entire machine-level body of a naked function. It is allowed only in a naked function, whose body must consist of one such invocation; the compiler emits no function prologue or epilogue. The author must honor the function signature and calling convention and ensure the code returns or diverges rather than falling through. Only `sym` and `const` operands and the `att_syntax` and `raw` options are supported; target and compiler support also matter. The spelling alone does not prove macro resolution or that the assembly is valid or safe.", src.norm(path), src.norm(template)),
        definition: None, borrow: false,
    }))
}
