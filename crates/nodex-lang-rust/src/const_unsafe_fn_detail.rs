//! Explain the combination of const-evaluation eligibility and an unsafe call.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" || node.has_error() { return None; }
    let modifiers = children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let parts = children(modifiers);
    let constant = parts.iter().copied().find(|part| part.kind() == "const")?;
    let unsafe_keyword = parts.iter().copied().find(|part| part.kind() == "unsafe")?;
    if constant.start_byte() > unsafe_keyword.start_byte() { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Const unsafe function".into(),
        text: format!("const unsafe fn {} combines two separate properties. const makes this function eligible for evaluation in a constant context when its body and arguments satisfy constant-evaluation rules; a call outside such a context may run at runtime. unsafe means a caller must use an unsafe context and uphold the function's documented safety requirements, including when calling it in a constant context. The modifiers do not reveal the specific requirements, and an unsafe block does not prove they are met. Rust still checks types and const-evaluation rules, and the function is not executed merely by being declared.", src.norm(name)),
        span: src.index.span(src.file, constant.start_byte(), unsafe_keyword.end_byte()), type_info: None,
    })
}
