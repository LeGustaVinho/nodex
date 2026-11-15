//! Explain `#[unsafe(naked)]` on a function with a body.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" || node.has_error() { return Vec::new(); }
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {
                let compact: String = src.text(part).chars().filter(|ch| !ch.is_whitespace()).collect();
                if compact == "#[unsafe(naked)]" {
                    result.push(ContractItem {
                        group: "Unsafe naked function".into(),
                        text: "`#[unsafe(naked)]` marks the following function as naked: the compiler emits no usual function prologue or epilogue, so it does not automatically generate the entry and exit sequences normally used for a call. The body must consist of one `naked_asm!` invocation, and the function cannot use the Rust ABI. The assembly author must uphold the declared signature and calling convention, handle stack and register obligations, and return correctly or diverge instead of falling through the end. The `unsafe(...)` wrapper acknowledges these safety obligations; seeing the attribute does not prove that the assembly satisfies them. Availability and accepted ABIs depend on the compiler and target.".into(),
                        span: src.span(part), type_info: None,
                    });
                }
            }
            "line_comment" if src.text(part).starts_with("//!") => break,
            "block_comment" if src.text(part).starts_with("/*!") => break,
            "line_comment" | "block_comment" => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    result
}
