//! Explain an optional trailing comma in a function parameter list.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") || node.has_error() { return None; }
    let params = node.child_by_field_name("parameters")?;
    let parts: Vec<_> = children(params).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() < 3 || parts.last()?.kind() != ")" { return None; }
    let comma = parts[parts.len() - 2];
    if comma.kind() != "," { return None; }
    Some(ContractItem {
        group: "Trailing comma in parameter list".into(),
        text: "The comma immediately before `)` is optional punctuation after the last parameter. In `fn f(x: i32,)`, the function still has one parameter, `x`, of type `i32`; the comma does not add another parameter or change the function signature or behavior. It can make adding, reordering, or formatting parameters more convenient. This is a parameter list, not a one-element tuple: a tuple's comma has a different syntactic role.".into(),
        span: src.span(comma), type_info: None,
    })
}
