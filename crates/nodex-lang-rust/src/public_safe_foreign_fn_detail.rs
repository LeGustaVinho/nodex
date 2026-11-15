//! Explain public visibility on explicitly safe foreign declarations.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_signature_item" { return None; }
    // Reuse the existing safe-keyword recovery, which reads the original source
    // when the parser blanks `safe` for grammar compatibility.
    crate::foreign_safe_fn_explanation::contract(src, node)?;
    let visibility = children(node).into_iter().find(|part| part.kind() == "visibility_modifier")?;
    if src.norm(visibility) != "pub" { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Public safe foreign function".into(),
        text: format!("`pub safe fn {}` declares a public foreign function; it provides a Rust-facing declaration, not the external implementation. `pub` permits access from other modules or crates only through an accessible path (including any containing modules or a public re-export). `safe` promises that safe Rust can call it without an unsafe block; the author of the enclosing unsafe extern block must ensure the signature, ABI and safety promise match the external implementation. Public visibility and `safe` do not prove that the symbol exists, links, or is actually safe.", src.norm(name)),
        span: src.span(visibility), type_info: None,
    })
}
