//! Explain a required associated type declared in a trait.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "associated_type" { return None; }
    let owner = node.parent()?.parent()?;
    if owner.kind() != "trait_item" { return None; }
    let name = src.norm(node.child_by_field_name("name")?);
    let trait_name = src.norm(owner.child_by_field_name("name")?);
    Some(ContractItem {
        group: "Associated type syntax".into(),
        text: format!("`type {name};` declares a named type that belongs to the trait {trait_name}; it does not create a value. The declaration leaves the concrete type unspecified, so each implementation must supply it using `type {name} = ...;` and meet any bounds written on the declaration. Code can refer to the selected type through a qualified path such as `<Concrete as {trait_name}>::{name}`."),
        span: src.span(node),
        type_info: None,
    })
}
