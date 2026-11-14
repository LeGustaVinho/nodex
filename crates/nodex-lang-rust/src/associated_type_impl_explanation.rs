//! Explain the definition of an associated type in a trait implementation.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "type_item" { return None; }
    let implementation = node.parent()?.parent()?;
    if implementation.kind() != "impl_item" { return None; }
    let trait_name = implementation.child_by_field_name("trait")?;
    let implementer = implementation.child_by_field_name("type")?;
    let name = node.child_by_field_name("name")?;
    let chosen = node.child_by_field_name("type")?;
    Some(ContractItem {
        group: "Associated type definition".into(),
        text: format!(
            "In this implementation of {} for {}, `type {} = {}` selects {} as the type associated with {} for this implementation. Code using the trait's associated type for this implementer refers to that selected type; this declaration chooses a type, not a value or a new distinct type. Other implementations may select different types, subject to the trait's requirements.",
            src.norm(trait_name), src.norm(implementer), src.norm(name), src.norm(chosen),
            src.norm(chosen), src.norm(name),
        ),
        span: src.span(node),
        type_info: None,
    })
}
