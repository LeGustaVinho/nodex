//! Explain a lifetime-parameterized associated type declaration in a trait.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "associated_type" || node.has_error() { return None; }
    let owner = node.parent()?.parent()?;
    if owner.kind() != "trait_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let parameters = node.child_by_field_name("type_parameters")?;
    let lifetime = named_children(parameters).into_iter().find(|part| part.kind() == "lifetime_parameter")?;
    let lifetime_name = lifetime.child_by_field_name("name")?;
    let trait_name = owner.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Lifetime-generic associated type".into(),
        text: format!("`type {}{};` declares a generic associated type in the trait {}. Each implementation supplies a type definition parameterized by the declared arguments: this describes a family of associated types, not a runtime value. The apostrophe in {} identifies a lifetime parameter; code selecting the associated type supplies a lifetime argument, and any bounds or where-clause requirements must hold. A lifetime can describe validity relationships, but this declaration alone does not prove that the resulting type is a reference or that it borrows the implementing value. A qualified use can look like `<Concrete as {}>::{}<{}>` when the declaration has this one lifetime parameter.", src.norm(name), src.norm(parameters), src.norm(trait_name), src.norm(lifetime_name), src.norm(trait_name), src.norm(name), src.norm(lifetime_name)),
        span: src.span(parameters), type_info: None,
    })
}
