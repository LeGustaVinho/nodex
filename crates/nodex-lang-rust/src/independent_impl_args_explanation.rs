//! Connect separate argument-position `impl Trait` parameters.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return None; }
    let parameters = node.child_by_field_name("parameters")?;
    let written: Vec<_> = named_children(parameters).into_iter().filter_map(|parameter| {
        let ty = parameter.child_by_field_name("type")?;
        (ty.kind() == "abstract_type").then_some(ty)
    }).collect();
    if written.len() < 2 { return None; }
    let descriptions = written.iter().map(|ty| format!("`{}`", src.norm(*ty))).collect::<Vec<_>>().join(", ");
    Some(ContractItem {
        group: "Independent impl Trait arguments".into(),
        text: format!("The argument-position types {descriptions} each introduce a separate anonymous generic type parameter. The caller may supply different concrete types for the arguments, as long as each type satisfies its own written trait bounds; the two types may also be the same. Repeating the same trait name does not require the arguments' types to match. To require the same type, a named parameter can be shared, for example `fn f<T: Trait>(a: T, b: T)`. This description concerns the written bounds; the exact trait identities and actual argument types depend on resolution and the call."),
        span: src.span(parameters), type_info: None,
    })
}
