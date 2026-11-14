//! Explain a method body supplied by a trait declaration.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, method: Node<'_>) -> Option<ContractItem> {
    if method.kind() != "function_item" { return None; }
    let body = method.child_by_field_name("body")?;
    let owner = method.parent()?.parent()?;
    if owner.kind() != "trait_item" { return None; }
    let name = method.child_by_field_name("name")?;
    let trait_name = owner.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Default trait method".into(),
        text: format!("The body in braces supplies a default implementation of {} for trait {}. A type implementing the trait may use this body without defining the method again, or provide its own implementation to override it. The body runs when the method is called, not when the trait is declared.", src.norm(name), src.norm(trait_name)),
        span: src.span(body),
        type_info: None,
    })
}
