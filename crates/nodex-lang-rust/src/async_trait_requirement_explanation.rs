//! Explain async methods required by a trait declaration.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, method: Node<'_>) -> Option<ContractItem> {
    if method.kind() != "function_signature_item" { return None; }
    let owner = method.parent()?.parent()?;
    if owner.kind() != "trait_item" { return None; }
    let modifiers = children(method).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let keyword = children(modifiers).into_iter().find(|part| part.kind() == "async")?;
    let method_name = src.norm(method.child_by_field_name("name")?);
    let trait_name = src.norm(owner.child_by_field_name("name")?);
    let output = method.child_by_field_name("return_type").map(|ty| src.norm(ty)).unwrap_or_else(|| "()".into());
    Some(ContractItem {
        group: "Required async trait method".into(),
        text: format!("Trait {trait_name} requires an implementation of the asynchronous method {method_name}; the semicolon means this declaration supplies no default body. Calling the implemented method returns a future, a value representing work that progresses when polled, for example through `.await` in a running async context. Calling alone does not schedule a background task. The written return type {output} is the future's output on completion; when omitted, that output is `()`. The future may borrow the method's arguments, including a `&self` receiver, so their required lifetimes must be respected. The concrete future type is determined by the implementation and is not resolved from this declaration alone."),
        span: src.span(keyword), type_info: None,
    })
}
