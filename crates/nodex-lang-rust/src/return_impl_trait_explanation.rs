//! Explain return-position `impl Trait` without requiring semantic name resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return None; }
    let result = node.child_by_field_name("return_type")?;
    if result.kind() != "abstract_type" { return None; }
    let written = src.norm(result);
    Some(ContractItem {
        group: "Opaque return type".into(),
        text: format!("The `->` introduces the return type. In `{written}`, `impl` means the function returns one concrete type selected by its implementation, while promising that this type satisfies the written trait bounds. Callers can use that promised behavior without the signature naming the concrete type. For a particular instantiation, all return paths must produce the same concrete type; this syntax alone does not imply a heap allocation or dynamic dispatch. The meaning of the bound names depends on name resolution."),
        span: src.span(result),
        type_info: None,
    })
}
