//! Explain a written enum with no variants.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "enum_item" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    if named_children(body).iter().any(|part| !part.is_extra()) { return None; }
    Some(ContractItem {
        group: "Enum with no variants".into(),
        text: format!("`enum {} {{}}` declares a distinct enum type with no alternatives, so the type has no valid values: it is uninhabited. Safe code cannot construct a value of this type, and unsafe code cannot make an invalid value valid. An empty `match` on a value of this type is exhaustive because there are no variants to handle. An expression of never type `!`, such as an infinite `loop`, can be used where this type is expected, but this enum remains a distinct named type, not the `!` type itself. Other attributes and the build configuration can affect whether the declaration is accepted; this description does not prove compilation succeeds.", src.norm(name)),
        span: src.span(body), type_info: None,
    })
}
