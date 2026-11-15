//! Explain a required, argument-free trait function returning the implementor.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, method: Node<'_>) -> Option<ContractItem> {
    if method.kind() != "function_signature_item" { return None; }
    let owner = method.parent()?.parent()?;
    if owner.kind() != "trait_item" { return None; }
    let output = method.child_by_field_name("return_type")?;
    if src.text(output) != "Self" { return None; }
    let parameters = method.child_by_field_name("parameters")?;
    if !named_children(parameters).is_empty() { return None; }
    if children(method).into_iter().find(|part| part.kind() == "function_modifiers").is_some_and(|m| children(m).into_iter().any(|part| part.kind() == "async")) { return None; }
    let method_name = src.norm(method.child_by_field_name("name")?);
    let trait_name = src.norm(owner.child_by_field_name("name")?);
    Some(ContractItem {
        group: "Required trait function returning its implementor".into(),
        text: format!("Trait {trait_name} requires each implementation to provide the function {method_name}; the semicolon indicates that no default body is supplied. `Self` here means the concrete type implementing the trait, so an implementation for a type returns a value of that type. The function has no `self` receiver and is associated with the type rather than called on an existing value; a fully qualified call can use `<ConcreteType as {trait_name}>::{method_name}()`. The declaration alone does not tell us which type implements the trait or how the function constructs its result."),
        span: src.span(output), type_info: None,
    })
}
